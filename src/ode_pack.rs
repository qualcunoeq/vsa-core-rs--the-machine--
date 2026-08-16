//! Shadow bounded ordinary-differential-equation curriculum pack.
//!
//! The pack is intentionally narrow: scalar autonomous equations with a
//! constant derivative or an affine linear right-hand side.  It emits exact
//! symbolic solution artifacts and refuses numerical approximation, nonlinear
//! equations, coupled systems, and claims outside the declared finite scope.

use crate::probability_pack::Rational;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAX_TIME_NUMERATOR: i128 = 8;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OdeOperation {
    ConstantDerivative,
    AffineLinear,
    Nonlinear,
    CoupledSystem,
    NumericalApproximation,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OdeStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidDomain,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OdeArtifact {
    ConstantValue {
        initial: Rational,
        derivative: Rational,
        time: Rational,
        value: Rational,
    },
    AffineLinearSolution {
        initial: Rational,
        coefficient: Rational,
        forcing: Rational,
        time: Rational,
        equilibrium: Option<Rational>,
        exponential_argument: Option<Rational>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OdeRequest {
    pub operation: OdeOperation,
    pub initial: Option<Rational>,
    pub coefficient: Option<Rational>,
    pub forcing: Option<Rational>,
    pub time: Option<Rational>,
    pub domain: String,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OdeResult {
    pub status: OdeStatus,
    pub artifact: Option<OdeArtifact>,
    pub operation: OdeOperation,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("ode serializes"))
    )
}

fn payload(result: &OdeResult) -> impl Serialize + '_ {
    (
        result.status,
        result.artifact.as_ref(),
        result.operation,
        &result.assumptions,
        &result.reasons,
        &result.provenance,
    )
}

fn result(
    request: &OdeRequest,
    status: OdeStatus,
    artifact: Option<OdeArtifact>,
    assumptions: Vec<String>,
    reasons: Vec<String>,
) -> OdeResult {
    let mut output = OdeResult {
        status,
        artifact,
        operation: request.operation,
        assumptions,
        reasons,
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&output));
    output.replay_hash = replay_hash;
    output
}

fn add(left: &Rational, right: &Rational) -> Option<Rational> {
    left.add(right)
}

fn multiply(left: &Rational, right: &Rational) -> Option<Rational> {
    left.mul(right)
}

fn negate(value: &Rational) -> Option<Rational> {
    Rational::new(-value.numerator, value.denominator)
}

fn within_bound(time: &Rational) -> bool {
    time.denominator > 0
        && time.numerator >= 0
        && time.numerator <= MAX_TIME_NUMERATOR * time.denominator
}

/// Evaluate an exact bounded first-order scalar ODE request without mutation.
pub fn evaluate_ode(request: &OdeRequest) -> OdeResult {
    if request.domain != "bounded_exact_scalar_ode" {
        return result(
            request,
            OdeStatus::InvalidDomain,
            None,
            Vec::new(),
            vec!["domain is outside bounded exact scalar ODEs".into()],
        );
    }
    if let Some(ambiguity) = &request.ambiguity {
        return result(
            request,
            OdeStatus::Ambiguous,
            None,
            Vec::new(),
            vec![format!("unresolved interpretation: {ambiguity}")],
        );
    }
    if matches!(
        request.operation,
        OdeOperation::Nonlinear
            | OdeOperation::CoupledSystem
            | OdeOperation::NumericalApproximation
    ) {
        return result(
            request,
            OdeStatus::Unsupported,
            None,
            Vec::new(),
            vec!["operation is outside the bounded exact scalar ODE contract".into()],
        );
    }
    let Some(initial) = request.initial.clone() else {
        return result(
            request,
            OdeStatus::Missing,
            None,
            Vec::new(),
            vec!["initial value is required".into()],
        );
    };
    let Some(time) = request.time.clone() else {
        return result(
            request,
            OdeStatus::Missing,
            None,
            Vec::new(),
            vec!["evaluation time is required".into()],
        );
    };
    if !within_bound(&time) {
        return result(
            request,
            OdeStatus::Unsupported,
            None,
            Vec::new(),
            vec!["evaluation time exceeds the bounded exact horizon".into()],
        );
    }
    match request.operation {
        OdeOperation::ConstantDerivative => {
            let Some(derivative) = request.forcing.clone() else {
                return result(
                    request,
                    OdeStatus::Missing,
                    None,
                    Vec::new(),
                    vec!["constant derivative is required".into()],
                );
            };
            let Some(value) = multiply(&derivative, &time).and_then(|delta| add(&initial, &delta))
            else {
                return result(
                    request,
                    OdeStatus::Unsupported,
                    None,
                    Vec::new(),
                    vec!["exact rational arithmetic overflowed".into()],
                );
            };
            result(
                request,
                OdeStatus::Complete,
                Some(OdeArtifact::ConstantValue {
                    initial,
                    derivative,
                    time,
                    value,
                }),
                vec![
                    "scalar autonomous constant derivative".into(),
                    "exact rational arithmetic".into(),
                ],
                Vec::new(),
            )
        }
        OdeOperation::AffineLinear => {
            let Some(coefficient) = request.coefficient.clone() else {
                return result(
                    request,
                    OdeStatus::Missing,
                    None,
                    Vec::new(),
                    vec!["linear coefficient is required".into()],
                );
            };
            let Some(forcing) = request.forcing.clone() else {
                return result(
                    request,
                    OdeStatus::Missing,
                    None,
                    Vec::new(),
                    vec!["constant forcing is required".into()],
                );
            };
            if coefficient.numerator == 0 {
                let Some(value) = multiply(&forcing, &time).and_then(|delta| add(&initial, &delta))
                else {
                    return result(
                        request,
                        OdeStatus::Unsupported,
                        None,
                        Vec::new(),
                        vec!["exact rational arithmetic overflowed".into()],
                    );
                };
                return result(
                    request,
                    OdeStatus::Complete,
                    Some(OdeArtifact::AffineLinearSolution {
                        initial,
                        coefficient,
                        forcing,
                        time,
                        equilibrium: None,
                        exponential_argument: Some(Rational::zero()),
                    }),
                    vec![
                        "degenerate affine equation reduces to a constant derivative".into(),
                        format!("exact value is {value:?}"),
                    ],
                    Vec::new(),
                );
            }
            let Some(equilibrium) =
                negate(&forcing).and_then(|negative| negative.div(&coefficient))
            else {
                return result(
                    request,
                    OdeStatus::Unsupported,
                    None,
                    Vec::new(),
                    vec!["equilibrium could not be represented exactly".into()],
                );
            };
            let Some(exponential_argument) = multiply(&coefficient, &time) else {
                return result(
                    request,
                    OdeStatus::Unsupported,
                    None,
                    Vec::new(),
                    vec!["exponential argument overflowed".into()],
                );
            };
            result(
                request,
                OdeStatus::Complete,
                Some(OdeArtifact::AffineLinearSolution {
                    initial,
                    coefficient,
                    forcing,
                    time,
                    equilibrium: Some(equilibrium),
                    exponential_argument: Some(exponential_argument),
                }),
                vec![
                    "scalar autonomous affine linear equation".into(),
                    "solution retained symbolically as an exact exponential".into(),
                ],
                Vec::new(),
            )
        }
        OdeOperation::Nonlinear
        | OdeOperation::CoupledSystem
        | OdeOperation::NumericalApproximation => unreachable!(),
    }
}

impl OdeResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != OdeStatus::Complete || self.artifact.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(operation: OdeOperation) -> OdeRequest {
        OdeRequest {
            operation,
            initial: Some(Rational::new(2, 1).unwrap()),
            coefficient: Some(Rational::new(3, 1).unwrap()),
            forcing: Some(Rational::new(4, 1).unwrap()),
            time: Some(Rational::new(2, 1).unwrap()),
            domain: "bounded_exact_scalar_ode".into(),
            ambiguity: None,
            provenance: vec!["ode-test".into()],
        }
    }

    #[test]
    fn exact_constant_and_affine_solutions_replay() {
        let constant = evaluate_ode(&request(OdeOperation::ConstantDerivative));
        assert_eq!(constant.status, OdeStatus::Complete);
        assert!(constant.replay_verified());
        let affine = evaluate_ode(&request(OdeOperation::AffineLinear));
        assert_eq!(affine.status, OdeStatus::Complete);
        assert!(affine.replay_verified());
    }

    #[test]
    fn unsupported_and_tampered_cases_fail_closed() {
        let mut nonlinear = request(OdeOperation::Nonlinear);
        nonlinear.ambiguity = None;
        let result = evaluate_ode(&nonlinear);
        assert_eq!(result.status, OdeStatus::Unsupported);
        assert!(result.replay_verified());
        let mut tampered = evaluate_ode(&request(OdeOperation::AffineLinear));
        tampered.replay_hash.push('x');
        assert!(!tampered.replay_verified());
    }
}
