//! Bounded exact eventual-hitting probabilities for finite Markov chains.
//!
//! The request names target and avoid states explicitly. The pack solves only
//! finite rational chains with at most four states and a unique solution for
//! every transient hitting equation; it does not infer target semantics or
//! authorize expected hitting times, limits, or continuous-time behavior.

use crate::probability_pack::Rational;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const MAX_STATES: usize = 4;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HittingStatus {
    Complete,
    Ambiguous,
    InvalidTransition,
    InvalidBoundary,
    DimensionMismatch,
    NonUnique,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HittingRequest {
    pub domain: String,
    pub transition: Vec<Vec<Rational>>,
    pub initial: Vec<Rational>,
    pub target_states: Vec<usize>,
    pub avoid_states: Vec<usize>,
    pub row_stochastic: Option<bool>,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HittingArtifact {
    pub state_probabilities: Vec<Rational>,
    pub initial_probability: Rational,
    pub target_states: Vec<usize>,
    pub avoid_states: Vec<usize>,
    pub residual_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HittingResult {
    pub status: HittingStatus,
    pub artifact: Option<HittingArtifact>,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("hitting serializes"))
    )
}

fn payload(result: &HittingResult) -> impl Serialize + '_ {
    (
        result.status,
        result.artifact.as_ref(),
        &result.assumptions,
        &result.reasons,
        &result.provenance,
    )
}

fn finish(
    request: &HittingRequest,
    status: HittingStatus,
    artifact: Option<HittingArtifact>,
    assumptions: Vec<String>,
    reasons: Vec<String>,
) -> HittingResult {
    let mut result = HittingResult {
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

fn validate_transition(request: &HittingRequest) -> Result<usize, HittingStatus> {
    if request.row_stochastic != Some(true) {
        return Err(HittingStatus::Ambiguous);
    }
    let states = request.transition.len();
    if states == 0
        || states > MAX_STATES
        || request.transition.iter().any(|row| row.len() != states)
    {
        return Err(HittingStatus::DimensionMismatch);
    }
    if request
        .transition
        .iter()
        .flatten()
        .any(|value| !value.in_unit_interval())
    {
        return Err(HittingStatus::InvalidTransition);
    }
    if request.transition.iter().any(|row| {
        row.iter()
            .try_fold(Rational::zero(), |sum, value| sum.add(value))
            != Some(Rational::one())
    }) {
        return Err(HittingStatus::InvalidTransition);
    }
    if request.initial.len() != states
        || request.initial.iter().any(|value| !value.nonnegative())
        || request
            .initial
            .iter()
            .try_fold(Rational::zero(), |sum, value| sum.add(value))
            != Some(Rational::one())
    {
        return Err(HittingStatus::DimensionMismatch);
    }
    Ok(states)
}

fn solve(mut matrix: Vec<Vec<Rational>>) -> Result<Vec<Rational>, HittingStatus> {
    let dimension = matrix.len();
    if dimension == 0 {
        return Ok(Vec::new());
    }
    for pivot in 0..dimension {
        let pivot_row = (pivot..dimension).find(|row| matrix[*row][pivot].numerator != 0);
        let Some(pivot_row) = pivot_row else {
            return Err(HittingStatus::NonUnique);
        };
        matrix.swap(pivot, pivot_row);
        let divisor = matrix[pivot][pivot].clone();
        for column in pivot..=dimension {
            matrix[pivot][column] = matrix[pivot][column]
                .div(&divisor)
                .ok_or(HittingStatus::InvalidTransition)?;
        }
        for row in 0..dimension {
            if row == pivot {
                continue;
            }
            let factor = matrix[row][pivot].clone();
            if factor.numerator == 0 {
                continue;
            }
            for column in pivot..=dimension {
                let product = factor
                    .mul(&matrix[pivot][column])
                    .ok_or(HittingStatus::InvalidTransition)?;
                matrix[row][column] = matrix[row][column]
                    .sub(&product)
                    .ok_or(HittingStatus::InvalidTransition)?;
            }
        }
    }
    Ok(matrix.iter().map(|row| row[dimension].clone()).collect())
}

fn residual_matches(
    probabilities: &[Rational],
    request: &HittingRequest,
    target: &BTreeSet<usize>,
    avoid: &BTreeSet<usize>,
) -> bool {
    (0..probabilities.len()).all(|state| {
        if target.contains(&state) {
            return probabilities[state] == Rational::one();
        }
        if avoid.contains(&state) {
            return probabilities[state] == Rational::zero();
        }
        let expected = (0..probabilities.len()).try_fold(Rational::zero(), |sum, next| {
            probabilities[next]
                .mul(&request.transition[state][next])
                .and_then(|term| sum.add(&term))
        });
        expected == Some(probabilities[state].clone())
    })
}

/// Evaluate a bounded exact probability of reaching a target before an avoid set.
pub fn evaluate(request: &HittingRequest) -> HittingResult {
    let assumptions = vec![
        "finite exact rational probabilities".into(),
        "row-stochastic convention explicitly declared".into(),
        "target and avoid states are explicit and disjoint".into(),
        "eventual-hitting equations have one exact solution".into(),
    ];
    if request.domain != "finite_exact_markov_hitting" {
        return finish(
            request,
            HittingStatus::Unsupported,
            None,
            assumptions,
            vec!["domain is outside bounded finite hitting probabilities".into()],
        );
    }
    if let Some(ambiguity) = &request.ambiguity {
        return finish(
            request,
            HittingStatus::Ambiguous,
            None,
            assumptions,
            vec![ambiguity.clone()],
        );
    }
    let states = match validate_transition(request) {
        Ok(states) => states,
        Err(status) => {
            return finish(
                request,
                status,
                None,
                assumptions,
                vec!["transition or initial distribution is not validated".into()],
            )
        }
    };
    let target: BTreeSet<usize> = request.target_states.iter().copied().collect();
    let avoid: BTreeSet<usize> = request.avoid_states.iter().copied().collect();
    if target.is_empty()
        || avoid.is_empty()
        || target.len() != request.target_states.len()
        || avoid.len() != request.avoid_states.len()
        || target.iter().any(|state| *state >= states)
        || avoid.iter().any(|state| *state >= states)
        || target.intersection(&avoid).next().is_some()
    {
        return finish(
            request,
            HittingStatus::InvalidBoundary,
            None,
            assumptions,
            vec!["target and avoid states must be nonempty, in range, and disjoint".into()],
        );
    }
    let transient: Vec<usize> = (0..states)
        .filter(|state| !target.contains(state) && !avoid.contains(state))
        .collect();
    let mut probabilities = vec![Rational::zero(); states];
    for state in &target {
        probabilities[*state] = Rational::one();
    }
    let mut equations = Vec::with_capacity(transient.len());
    for state in &transient {
        let mut equation = vec![Rational::zero(); transient.len() + 1];
        for (column, next) in transient.iter().enumerate() {
            let Some(value) = (if state == next {
                Rational::one()
            } else {
                Rational::zero()
            })
            .sub(&request.transition[*state][*next]) else {
                return finish(
                    request,
                    HittingStatus::InvalidTransition,
                    None,
                    assumptions,
                    vec!["exact hitting equation construction failed".into()],
                );
            };
            equation[column] = value;
        }
        let Some(rhs) = target.iter().try_fold(Rational::zero(), |sum, next| {
            sum.add(&request.transition[*state][*next])
        }) else {
            return finish(
                request,
                HittingStatus::InvalidTransition,
                None,
                assumptions,
                vec!["exact hitting right-hand side construction failed".into()],
            );
        };
        equation[transient.len()] = rhs;
        equations.push(equation);
    }
    let solution = match solve(equations) {
        Ok(solution) => solution,
        Err(status) => {
            return finish(
                request,
                status,
                None,
                assumptions,
                vec!["hitting equations do not have one validated solution".into()],
            )
        }
    };
    for (state, value) in transient.iter().zip(solution) {
        probabilities[*state] = value;
    }
    if probabilities
        .iter()
        .any(|value| !value.nonnegative() || value.numerator > value.denominator)
    {
        return finish(
            request,
            HittingStatus::InvalidTransition,
            None,
            assumptions,
            vec!["hitting probability left the unit interval".into()],
        );
    }
    let initial_probability = request
        .initial
        .iter()
        .zip(&probabilities)
        .try_fold(Rational::zero(), |sum, (weight, value)| {
            weight.mul(value).and_then(|term| sum.add(&term))
        })
        .expect("validated rational hitting probability");
    if !residual_matches(&probabilities, request, &target, &avoid) {
        return finish(
            request,
            HittingStatus::InvalidTransition,
            None,
            assumptions,
            vec!["exact hitting residual was not zero".into()],
        );
    }
    finish(
        request,
        HittingStatus::Complete,
        Some(HittingArtifact {
            state_probabilities: probabilities,
            initial_probability,
            target_states: request.target_states.clone(),
            avoid_states: request.avoid_states.clone(),
            residual_verified: true,
        }),
        assumptions,
        Vec::new(),
    )
}

impl HittingResult {
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

    #[test]
    fn solves_exact_target_probability() {
        let request = HittingRequest {
            domain: "finite_exact_markov_hitting".into(),
            transition: vec![
                vec![q(1, 1), q(0, 1), q(0, 1)],
                vec![q(1, 4), q(1, 4), q(1, 2)],
                vec![q(0, 1), q(0, 1), q(1, 1)],
            ],
            initial: vec![q(0, 1), q(1, 1), q(0, 1)],
            target_states: vec![2],
            avoid_states: vec![0],
            row_stochastic: Some(true),
            ambiguity: None,
            provenance: vec!["unit-test".into()],
        };
        let result = evaluate(&request);
        assert_eq!(result.status, HittingStatus::Complete);
        assert_eq!(result.artifact.unwrap().initial_probability, q(2, 3));
    }

    #[test]
    fn rejects_non_unique_and_tamper() {
        let request = HittingRequest {
            domain: "finite_exact_markov_hitting".into(),
            transition: vec![
                vec![q(1, 1), q(0, 1), q(0, 1)],
                vec![q(0, 1), q(1, 1), q(0, 1)],
                vec![q(0, 1), q(0, 1), q(1, 1)],
            ],
            initial: vec![q(0, 1), q(1, 1), q(0, 1)],
            target_states: vec![2],
            avoid_states: vec![0],
            row_stochastic: Some(true),
            ambiguity: None,
            provenance: vec!["unit-test".into()],
        };
        let result = evaluate(&request);
        assert_eq!(result.status, HittingStatus::NonUnique);
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        assert!(result.replay_verified());
        assert!(!tampered.replay_verified());
    }
}
