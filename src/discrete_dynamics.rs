//! Shadow bounded scalar/vector discrete-dynamics curriculum pack.
//!
//! This layer executes exact finite horizons only. It does not infer
//! asymptotic stability, closed forms, stationary behavior, or spectral
//! shortcuts from a finite trace.

use crate::probability_pack::Rational;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DynamicsOperation {
    ScalarAffine,
    VectorLinear,
    MatrixEvolution,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DynamicsStatus {
    Complete,
    Missing,
    Ambiguous,
    DimensionMismatch,
    InvalidParameters,
    Unsupported,
    BudgetExceeded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DynamicsArtifact {
    Scalar(Rational),
    Vector(Vec<Rational>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DynamicsRequest {
    pub operation: DynamicsOperation,
    pub domain: String,
    pub scalar_initial: Option<Rational>,
    pub coefficient: Option<Rational>,
    pub offset: Option<Rational>,
    pub vector_initial: Option<Vec<Rational>>,
    pub matrix: Option<Vec<Vec<Rational>>>,
    pub steps: usize,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DynamicsResult {
    pub status: DynamicsStatus,
    pub artifact: Option<DynamicsArtifact>,
    pub operation: DynamicsOperation,
    pub trace: Vec<DynamicsArtifact>,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("dynamics serializes"))
    )
}

fn replay_payload(result: &DynamicsResult) -> impl Serialize + '_ {
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
    request: &DynamicsRequest,
    status: DynamicsStatus,
    artifact: Option<DynamicsArtifact>,
    trace: Vec<DynamicsArtifact>,
    assumptions: Vec<String>,
    reasons: Vec<String>,
) -> DynamicsResult {
    let mut result = DynamicsResult {
        status,
        artifact,
        operation: request.operation,
        trace,
        assumptions,
        reasons,
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    let replay_hash = digest(&replay_payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn vector_step(matrix: &[Vec<Rational>], vector: &[Rational]) -> Option<Vec<Rational>> {
    if matrix.len() != vector.len() || matrix.iter().any(|row| row.len() != vector.len()) {
        return None;
    }
    matrix
        .iter()
        .map(|row| {
            row.iter()
                .zip(vector)
                .try_fold(Rational::zero(), |sum, (coefficient, value)| {
                    coefficient.mul(value).and_then(|term| sum.add(&term))
                })
        })
        .collect()
}

pub fn evaluate_dynamics(request: &DynamicsRequest) -> DynamicsResult {
    if request.domain != "finite_exact_discrete_dynamics" {
        return result(
            request,
            DynamicsStatus::Unsupported,
            None,
            Vec::new(),
            Vec::new(),
            vec!["domain is outside bounded exact dynamics".into()],
        );
    }
    if let Some(ambiguity) = &request.ambiguity {
        return result(
            request,
            DynamicsStatus::Ambiguous,
            None,
            Vec::new(),
            Vec::new(),
            vec![ambiguity.clone()],
        );
    }
    if request.steps > 8 {
        return result(
            request,
            DynamicsStatus::BudgetExceeded,
            None,
            Vec::new(),
            vec!["finite horizon budget is at most eight steps".into()],
            vec!["infinite-horizon and over-budget evolution is unsupported".into()],
        );
    }
    match request.operation {
        DynamicsOperation::ScalarAffine => {
            let (Some(mut value), Some(coefficient), Some(offset)) = (
                request.scalar_initial.clone(),
                request.coefficient.clone(),
                request.offset.clone(),
            ) else {
                return result(
                    request,
                    DynamicsStatus::Missing,
                    None,
                    Vec::new(),
                    Vec::new(),
                    vec![
                        "scalar affine evolution requires initial, coefficient, and offset".into(),
                    ],
                );
            };
            let mut trace = Vec::with_capacity(request.steps);
            for _ in 0..request.steps {
                let Some(next) = coefficient.mul(&value).and_then(|term| term.add(&offset)) else {
                    return result(
                        request,
                        DynamicsStatus::InvalidParameters,
                        None,
                        trace,
                        Vec::new(),
                        vec!["exact scalar update failed".into()],
                    );
                };
                value = next;
                trace.push(DynamicsArtifact::Scalar(value.clone()));
            }
            result(
                request,
                DynamicsStatus::Complete,
                Some(DynamicsArtifact::Scalar(value)),
                trace,
                vec![
                    "exact rational affine recurrence".into(),
                    "finite horizon".into(),
                ],
                Vec::new(),
            )
        }
        DynamicsOperation::VectorLinear | DynamicsOperation::MatrixEvolution => {
            let (Some(mut vector), Some(matrix)) =
                (request.vector_initial.clone(), request.matrix.clone())
            else {
                return result(
                    request,
                    DynamicsStatus::Missing,
                    None,
                    Vec::new(),
                    Vec::new(),
                    vec!["vector evolution requires an initial vector and matrix".into()],
                );
            };
            if vector.is_empty() || matrix.len() != vector.len() {
                return result(
                    request,
                    DynamicsStatus::DimensionMismatch,
                    None,
                    Vec::new(),
                    Vec::new(),
                    vec!["matrix and vector dimensions differ".into()],
                );
            }
            let mut trace = Vec::with_capacity(request.steps);
            for _ in 0..request.steps {
                let Some(next) = vector_step(&matrix, &vector) else {
                    return result(
                        request,
                        DynamicsStatus::DimensionMismatch,
                        None,
                        trace,
                        Vec::new(),
                        vec!["matrix rows are not square".into()],
                    );
                };
                vector = next;
                trace.push(DynamicsArtifact::Vector(vector.clone()));
            }
            result(
                request,
                DynamicsStatus::Complete,
                Some(DynamicsArtifact::Vector(vector)),
                trace,
                vec![
                    "exact rational matrix evolution".into(),
                    "finite horizon".into(),
                ],
                Vec::new(),
            )
        }
    }
}

impl DynamicsResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&replay_payload(self))
            && !self.provenance.is_empty()
            && (self.status != DynamicsStatus::Complete || self.artifact.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rational(numerator: i128, denominator: i128) -> Rational {
        Rational::new(numerator, denominator).unwrap()
    }

    fn scalar(steps: usize) -> DynamicsRequest {
        DynamicsRequest {
            operation: DynamicsOperation::ScalarAffine,
            domain: "finite_exact_discrete_dynamics".into(),
            scalar_initial: Some(rational(1, 1)),
            coefficient: Some(rational(2, 1)),
            offset: Some(rational(1, 1)),
            vector_initial: None,
            matrix: None,
            steps,
            ambiguity: None,
            provenance: vec!["test".into()],
        }
    }

    #[test]
    fn scalar_and_vector_evolution_replay() {
        let result = evaluate_dynamics(&scalar(3));
        assert_eq!(
            result.artifact,
            Some(DynamicsArtifact::Scalar(rational(15, 1)))
        );
        assert_eq!(result.trace.len(), 3);
        assert!(result.replay_verified());
        let vector = DynamicsRequest {
            operation: DynamicsOperation::VectorLinear,
            domain: "finite_exact_discrete_dynamics".into(),
            scalar_initial: None,
            coefficient: None,
            offset: None,
            vector_initial: Some(vec![rational(1, 1), rational(0, 1)]),
            matrix: Some(vec![
                vec![rational(1, 1), rational(1, 1)],
                vec![rational(0, 1), rational(1, 1)],
            ]),
            steps: 2,
            ambiguity: None,
            provenance: vec!["test".into()],
        };
        assert!(evaluate_dynamics(&vector).replay_verified());
    }

    #[test]
    fn dynamic_boundaries_fail_closed() {
        assert_eq!(
            evaluate_dynamics(&scalar(9)).status,
            DynamicsStatus::BudgetExceeded
        );
        let mut ambiguous = scalar(1);
        ambiguous.ambiguity = Some("asymptotic stability is requested".into());
        assert_eq!(
            evaluate_dynamics(&ambiguous).status,
            DynamicsStatus::Ambiguous
        );
    }
}
