//! Bounded exact finite Markov-chain curriculum pack.
//!
//! This pack extends the finite probability substrate with explicitly typed
//! row-stochastic transitions.  It supports exact one-step and finite-horizon
//! evolution, plus a closed-form stationary distribution only for a declared
//! two-state chain with a unique stationary solution.  It refuses spectral,
//! asymptotic, continuous-time, and convention-ambiguous requests.

use crate::probability_pack::Rational;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAX_STEPS: usize = 8;
const MAX_STATES: usize = 4;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MarkovOperation {
    OneStep,
    FiniteHorizon,
    StationaryDistribution,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MarkovStatus {
    Complete,
    Missing,
    Ambiguous,
    InvalidTransition,
    DimensionMismatch,
    BudgetExceeded,
    Unsupported,
    NonUniqueStationary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MarkovArtifact {
    Distribution(Vec<Rational>),
    Trace(Vec<Vec<Rational>>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarkovRequest {
    pub operation: MarkovOperation,
    pub domain: String,
    pub initial: Vec<Rational>,
    pub transition: Vec<Vec<Rational>>,
    pub steps: usize,
    pub row_stochastic: Option<bool>,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarkovResult {
    pub status: MarkovStatus,
    pub artifact: Option<MarkovArtifact>,
    pub operation: MarkovOperation,
    pub trace: Vec<Vec<Rational>>,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).expect("markov serializes")))
}

fn payload(result: &MarkovResult) -> impl Serialize + '_ {
    (
        result.status,
        result.artifact.as_ref(),
        result.operation,
        &result.trace,
        &result.assumptions,
        &result.reasons,
        &result.provenance,
    )
}

fn result(
    request: &MarkovRequest,
    status: MarkovStatus,
    artifact: Option<MarkovArtifact>,
    trace: Vec<Vec<Rational>>,
    assumptions: Vec<String>,
    reasons: Vec<String>,
) -> MarkovResult {
    let mut output = MarkovResult {
        status,
        artifact,
        operation: request.operation,
        trace,
        assumptions,
        reasons,
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&output));
    output.replay_hash = replay_hash;
    output
}

fn valid_distribution(values: &[Rational], states: usize) -> bool {
    values.len() == states
        && values.iter().all(Rational::nonnegative)
        && values
            .iter()
            .try_fold(Rational::zero(), |sum, value| sum.add(value))
            == Some(Rational::one())
}

fn validate_transition(request: &MarkovRequest) -> Result<usize, MarkovStatus> {
    if request.row_stochastic != Some(true) {
        return Err(MarkovStatus::Ambiguous);
    }
    let states = request.transition.len();
    if states == 0 || states > MAX_STATES || request.transition.iter().any(|row| row.len() != states)
    {
        return Err(MarkovStatus::DimensionMismatch);
    }
    if request
        .transition
        .iter()
        .flatten()
        .any(|value| !value.in_unit_interval())
    {
        return Err(MarkovStatus::InvalidTransition);
    }
    if request.transition.iter().any(|row| {
        row.iter()
            .try_fold(Rational::zero(), |sum, value| sum.add(value))
            != Some(Rational::one())
    }) {
        return Err(MarkovStatus::InvalidTransition);
    }
    if !valid_distribution(&request.initial, states) {
        return Err(MarkovStatus::DimensionMismatch);
    }
    Ok(states)
}

fn step(distribution: &[Rational], transition: &[Vec<Rational>]) -> Option<Vec<Rational>> {
    let states = transition.len();
    (0..states)
        .map(|target| {
            (0..states).try_fold(Rational::zero(), |sum, source| {
                distribution[source]
                    .mul(&transition[source][target])
                    .and_then(|term| sum.add(&term))
            })
        })
        .collect()
}

/// Evaluate a bounded, exact, explicitly row-stochastic Markov request.
pub fn evaluate_markov(request: &MarkovRequest) -> MarkovResult {
    if request.domain != "finite_exact_markov_chain" {
        return result(
            request,
            MarkovStatus::Unsupported,
            None,
            Vec::new(),
            Vec::new(),
            vec!["domain is outside finite exact Markov chains".into()],
        );
    }
    if let Some(ambiguity) = &request.ambiguity {
        return result(
            request,
            MarkovStatus::Ambiguous,
            None,
            Vec::new(),
            Vec::new(),
            vec![ambiguity.clone()],
        );
    }
    let states = match validate_transition(request) {
        Ok(states) => states,
        Err(status) => {
            return result(
                request,
                status,
                None,
                Vec::new(),
                Vec::new(),
                vec!["transition semantics or dimensions are not validated".into()],
            )
        }
    };
    if request.operation == MarkovOperation::FiniteHorizon && request.steps > MAX_STEPS {
        return result(
            request,
            MarkovStatus::BudgetExceeded,
            None,
            Vec::new(),
            vec!["finite horizon is bounded to eight transitions".into()],
            Vec::new(),
        );
    }
    let assumptions = vec![
        "finite exact rational probabilities".into(),
        "row-stochastic convention explicitly declared".into(),
        "fixed state ordering and immutable transition matrix".into(),
    ];
    match request.operation {
        MarkovOperation::OneStep => {
            let Some(next) = step(&request.initial, &request.transition) else {
                return result(request, MarkovStatus::InvalidTransition, None, Vec::new(), assumptions, vec!["exact transition failed".into()]);
            };
            result(request, MarkovStatus::Complete, Some(MarkovArtifact::Distribution(next)), Vec::new(), assumptions, Vec::new())
        }
        MarkovOperation::FiniteHorizon => {
            let mut current = request.initial.clone();
            let mut trace = Vec::with_capacity(request.steps);
            for _ in 0..request.steps {
                let Some(next) = step(&current, &request.transition) else {
                    return result(request, MarkovStatus::InvalidTransition, None, trace, assumptions, vec!["exact transition failed".into()]);
                };
                if !valid_distribution(&next, states) {
                    return result(request, MarkovStatus::InvalidTransition, None, trace, assumptions, vec!["normalization was not preserved".into()]);
                }
                current = next.clone();
                trace.push(next);
            }
            result(request, MarkovStatus::Complete, Some(MarkovArtifact::Trace(trace.clone())), trace, assumptions, Vec::new())
        }
        MarkovOperation::StationaryDistribution => {
            if states != 2 {
                return result(request, MarkovStatus::Unsupported, None, Vec::new(), assumptions, vec!["stationary solver is bounded to two-state chains".into()]);
            }
            let a = &request.transition[0][0];
            let b = &request.transition[1][0];
            let denominator = match b.add(&Rational::one().sub(a).unwrap()) {
                Some(value) => value,
                None => return result(request, MarkovStatus::NonUniqueStationary, None, Vec::new(), assumptions, vec!["stationary denominator is undefined".into()]),
            };
            if !denominator.positive() {
                return result(request, MarkovStatus::NonUniqueStationary, None, Vec::new(), assumptions, vec!["chain does not have a unique stationary distribution".into()]);
            }
            let first = b.div(&denominator).expect("positive stationary denominator");
            let second = Rational::one().sub(&first).expect("stationary complement");
            result(request, MarkovStatus::Complete, Some(MarkovArtifact::Distribution(vec![first, second])), Vec::new(), assumptions, Vec::new())
        }
    }
}

impl MarkovResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(numerator: i128, denominator: i128) -> Rational {
        Rational::new(numerator, denominator).unwrap()
    }

    fn request(operation: MarkovOperation) -> MarkovRequest {
        MarkovRequest {
            operation,
            domain: "finite_exact_markov_chain".into(),
            initial: vec![q(1, 1), q(0, 1)],
            transition: vec![vec![q(3, 4), q(1, 4)], vec![q(1, 2), q(1, 2)]],
            steps: 1,
            row_stochastic: Some(true),
            ambiguity: None,
            provenance: vec!["unit-test".into()],
        }
    }

    #[test]
    fn exact_stationary_distribution_is_replayable() {
        let output = evaluate_markov(&request(MarkovOperation::StationaryDistribution));
        assert_eq!(output.status, MarkovStatus::Complete);
        assert_eq!(
            output.artifact,
            Some(MarkovArtifact::Distribution(vec![q(2, 3), q(1, 3)]))
        );
        assert!(output.replay_verified());
    }

    #[test]
    fn convention_and_budget_are_fail_closed() {
        let mut ambiguous = request(MarkovOperation::OneStep);
        ambiguous.row_stochastic = None;
        assert_eq!(evaluate_markov(&ambiguous).status, MarkovStatus::Ambiguous);
        let mut over_budget = request(MarkovOperation::FiniteHorizon);
        over_budget.steps = MAX_STEPS + 1;
        assert_eq!(
            evaluate_markov(&over_budget).status,
            MarkovStatus::BudgetExceeded
        );
    }

    #[test]
    fn tampered_receipt_is_rejected() {
        let output = evaluate_markov(&request(MarkovOperation::OneStep));
        let mut tampered = output.clone();
        tampered.replay_hash.push('x');
        assert!(output.replay_verified());
        assert!(!tampered.replay_verified());
    }
}
