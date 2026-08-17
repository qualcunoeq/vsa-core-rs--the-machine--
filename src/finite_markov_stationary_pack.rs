//! Bounded exact stationary distributions for finite row-stochastic chains.
//!
//! This is deliberately separate from `finite_markov_pack`: the historical
//! pack's two-state contract remains reproducible, while this extension owns
//! the stronger exact linear-system boundary for at most four states.

use crate::probability_pack::Rational;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAX_STATES: usize = 4;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StationaryStatus {
    Complete,
    Ambiguous,
    InvalidTransition,
    DimensionMismatch,
    NonUnique,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StationaryRequest {
    pub domain: String,
    pub transition: Vec<Vec<Rational>>,
    pub row_stochastic: Option<bool>,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StationaryArtifact {
    pub distribution: Vec<Rational>,
    pub state_order: Vec<usize>,
    pub residual_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StationaryResult {
    pub status: StationaryStatus,
    pub artifact: Option<StationaryArtifact>,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("stationary serializes"))
    )
}

fn payload(result: &StationaryResult) -> impl Serialize + '_ {
    (
        result.status,
        result.artifact.as_ref(),
        &result.assumptions,
        &result.reasons,
        &result.provenance,
    )
}

fn finish(
    request: &StationaryRequest,
    status: StationaryStatus,
    artifact: Option<StationaryArtifact>,
    assumptions: Vec<String>,
    reasons: Vec<String>,
) -> StationaryResult {
    let mut result = StationaryResult {
        status,
        artifact,
        assumptions,
        reasons,
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    let hash = digest(&(
        result.status,
        result.artifact.clone(),
        result.assumptions.clone(),
        result.reasons.clone(),
        result.provenance.clone(),
    ));
    result.replay_hash = hash;
    result
}

fn validate_transition(request: &StationaryRequest) -> Result<usize, StationaryStatus> {
    if request.row_stochastic != Some(true) {
        return Err(StationaryStatus::Ambiguous);
    }
    let states = request.transition.len();
    if states == 0
        || states > MAX_STATES
        || request.transition.iter().any(|row| row.len() != states)
    {
        return Err(StationaryStatus::DimensionMismatch);
    }
    if request
        .transition
        .iter()
        .flatten()
        .any(|value| !value.in_unit_interval())
    {
        return Err(StationaryStatus::InvalidTransition);
    }
    if request.transition.iter().any(|row| {
        row.iter()
            .try_fold(Rational::zero(), |sum, value| sum.add(value))
            != Some(Rational::one())
    }) {
        return Err(StationaryStatus::InvalidTransition);
    }
    Ok(states)
}

fn solve_stationary(transition: &[Vec<Rational>]) -> Result<Vec<Rational>, StationaryStatus> {
    let states = transition.len();
    // Solve (P^T - I) pi = 0, replacing the final equation with sum(pi)=1.
    let mut matrix = vec![vec![Rational::zero(); states + 1]; states];
    for row in 0..states.saturating_sub(1) {
        for column in 0..states {
            matrix[row][column] = transition[column][row]
                .sub(&if row == column {
                    Rational::one()
                } else {
                    Rational::zero()
                })
                .ok_or(StationaryStatus::InvalidTransition)?;
        }
    }
    for column in 0..states {
        matrix[states - 1][column] = Rational::one();
    }
    matrix[states - 1][states] = Rational::one();

    // Exact Gauss-Jordan elimination. Every pivot is required: rank loss means
    // that the chain has more than one stationary distribution.
    for pivot in 0..states {
        let pivot_row = (pivot..states).find(|row| matrix[*row][pivot].numerator != 0);
        let Some(pivot_row) = pivot_row else {
            return Err(StationaryStatus::NonUnique);
        };
        matrix.swap(pivot, pivot_row);
        let divisor = matrix[pivot][pivot].clone();
        for column in pivot..=states {
            matrix[pivot][column] = matrix[pivot][column]
                .div(&divisor)
                .ok_or(StationaryStatus::InvalidTransition)?;
        }
        for row in 0..states {
            if row == pivot {
                continue;
            }
            let factor = matrix[row][pivot].clone();
            if factor.numerator == 0 {
                continue;
            }
            for column in pivot..=states {
                let product = factor
                    .mul(&matrix[pivot][column])
                    .ok_or(StationaryStatus::InvalidTransition)?;
                matrix[row][column] = matrix[row][column]
                    .sub(&product)
                    .ok_or(StationaryStatus::InvalidTransition)?;
            }
        }
    }
    let distribution: Vec<Rational> = matrix.iter().map(|row| row[states].clone()).collect();
    if distribution.iter().any(|value| !value.nonnegative())
        || distribution
            .iter()
            .try_fold(Rational::zero(), |sum, value| sum.add(value))
            != Some(Rational::one())
    {
        return Err(StationaryStatus::InvalidTransition);
    }
    Ok(distribution)
}

fn residual_matches(distribution: &[Rational], transition: &[Vec<Rational>]) -> bool {
    let states = transition.len();
    (0..states).all(|target| {
        let value = (0..states).try_fold(Rational::zero(), |sum, source| {
            distribution[source]
                .mul(&transition[source][target])
                .and_then(|term| sum.add(&term))
        });
        value == Some(distribution[target].clone())
    })
}

/// Evaluate a bounded exact stationary distribution with explicit row semantics.
pub fn evaluate(request: &StationaryRequest) -> StationaryResult {
    let assumptions = vec![
        "finite exact rational probabilities".into(),
        "row-stochastic convention explicitly declared".into(),
        "fixed state ordering from zero through n-1".into(),
        "unique stationary solution established by exact rank".into(),
    ];
    if request.domain != "finite_exact_markov_stationary" {
        return finish(
            request,
            StationaryStatus::Unsupported,
            None,
            assumptions,
            vec!["domain is outside bounded finite stationary distributions".into()],
        );
    }
    if let Some(ambiguity) = &request.ambiguity {
        return finish(
            request,
            StationaryStatus::Ambiguous,
            None,
            assumptions,
            vec![ambiguity.clone()],
        );
    }
    if let Err(status) = validate_transition(request) {
        return finish(
            request,
            status,
            None,
            assumptions,
            vec!["transition semantics or dimensions are not validated".into()],
        );
    }
    let distribution = match solve_stationary(&request.transition) {
        Ok(distribution) => distribution,
        Err(status) => {
            return finish(
                request,
                status,
                None,
                assumptions,
                vec!["stationary equations do not have one validated solution".into()],
            )
        }
    };
    let residual_verified = residual_matches(&distribution, &request.transition);
    if !residual_verified {
        return finish(
            request,
            StationaryStatus::InvalidTransition,
            None,
            assumptions,
            vec!["the exact stationary residual was not zero".into()],
        );
    }
    finish(
        request,
        StationaryStatus::Complete,
        Some(StationaryArtifact {
            state_order: (0..request.transition.len()).collect(),
            distribution,
            residual_verified,
        }),
        assumptions,
        Vec::new(),
    )
}

impl StationaryResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self)) && !self.provenance.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(numerator: i128, denominator: i128) -> Rational {
        Rational::new(numerator, denominator).unwrap()
    }

    fn request(transition: Vec<Vec<Rational>>) -> StationaryRequest {
        StationaryRequest {
            domain: "finite_exact_markov_stationary".into(),
            transition,
            row_stochastic: Some(true),
            ambiguity: None,
            provenance: vec!["unit-test".into()],
        }
    }

    #[test]
    fn solves_three_state_cycle_exactly() {
        let output = evaluate(&request(vec![
            vec![q(0, 1), q(1, 1), q(0, 1)],
            vec![q(0, 1), q(0, 1), q(1, 1)],
            vec![q(1, 1), q(0, 1), q(0, 1)],
        ]));
        assert_eq!(output.status, StationaryStatus::Complete);
        assert_eq!(
            output.artifact.unwrap().distribution,
            vec![q(1, 3), q(1, 3), q(1, 3)]
        );
    }

    #[test]
    fn rejects_nonunique_and_tampered_results() {
        let output = evaluate(&request(vec![
            vec![q(1, 1), q(0, 1), q(0, 1)],
            vec![q(0, 1), q(1, 1), q(0, 1)],
            vec![q(0, 1), q(0, 1), q(1, 1)],
        ]));
        assert_eq!(output.status, StationaryStatus::NonUnique);
        let mut tampered = output.clone();
        tampered.replay_hash.push('x');
        assert!(output.replay_verified());
        assert!(!tampered.replay_verified());
    }
}
