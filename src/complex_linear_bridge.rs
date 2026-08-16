//! Lossless bounded bridge from integral complex pairs to real matrices.
//!
//! The bridge preserves the complex pair as the semantic source and delegates
//! the real matrix determinant to the existing exact linear-algebra pack. It
//! refuses non-integral coordinates, scalar/polar artifacts, invalid domains,
//! and unresolved ambiguity rather than silently rounding or reinterpreting.

use crate::linear_algebra_pack::{
    evaluate_linear_algebra, LinearAlgebraOperation, LinearAlgebraRequest, LinearAlgebraResult,
    LinearAlgebraStatus,
};
use crate::probability_pack::Rational;
use crate::source_complex_pack::ComplexArtifact;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const DOMAIN: &str = "complex_to_real_matrix_bridge";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BridgeStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    NonIntegral,
    InvalidDomain,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComplexMatrixBridgeRequest {
    pub complex: Option<ComplexArtifact>,
    pub domain: String,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComplexMatrixBridgeResult {
    pub status: BridgeStatus,
    pub matrix: Option<Vec<Vec<i64>>>,
    pub complex_source: Option<ComplexArtifact>,
    pub linear_algebra: Option<LinearAlgebraResult>,
    pub norm_squared: Option<Rational>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("bridge serializes"))
    )
}

fn payload(result: &ComplexMatrixBridgeResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.matrix,
        &result.complex_source,
        &result.linear_algebra,
        &result.norm_squared,
        &result.reasons,
        &result.provenance,
    )
}

fn result(
    request: &ComplexMatrixBridgeRequest,
    status: BridgeStatus,
    matrix: Option<Vec<Vec<i64>>>,
    linear_algebra: Option<LinearAlgebraResult>,
    norm_squared: Option<Rational>,
    reasons: Vec<String>,
) -> ComplexMatrixBridgeResult {
    let mut output = ComplexMatrixBridgeResult {
        status,
        matrix,
        complex_source: request.complex.clone(),
        linear_algebra,
        norm_squared,
        reasons,
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&output));
    output.replay_hash = replay_hash;
    output
}

fn integral(value: &Rational) -> Option<i64> {
    (value.denominator == 1).then_some(value.numerator.try_into().ok()?)
}

fn norm_squared(real: &Rational, imag: &Rational) -> Option<Rational> {
    real.mul(real)?.add(&imag.mul(imag)?)
}

/// Convert `a + bi` to `[[a, -b], [b, a]]` only when both coordinates are
/// exact integers accepted by the linear-algebra pack.
pub fn bridge_complex_to_real_matrix(
    request: &ComplexMatrixBridgeRequest,
) -> ComplexMatrixBridgeResult {
    if request.domain != DOMAIN {
        return result(
            request,
            BridgeStatus::InvalidDomain,
            None,
            None,
            None,
            vec!["bridge domain is outside the declared complex-to-matrix contract".into()],
        );
    }
    if let Some(ambiguity) = &request.ambiguity {
        return result(
            request,
            BridgeStatus::Ambiguous,
            None,
            None,
            None,
            vec![ambiguity.clone()],
        );
    }
    let Some(complex) = &request.complex else {
        return result(
            request,
            BridgeStatus::Missing,
            None,
            None,
            None,
            vec!["an exact complex pair artifact is required".into()],
        );
    };
    let ComplexArtifact::Pair { real, imag } = complex else {
        return result(
            request,
            BridgeStatus::Unsupported,
            None,
            None,
            None,
            vec!["scalar or polar complex artifacts cannot enter this matrix bridge".into()],
        );
    };
    let (Some(real), Some(imag)) = (integral(real), integral(imag)) else {
        return result(
            request,
            BridgeStatus::NonIntegral,
            None,
            None,
            None,
            vec![
                "the exact integer linear-algebra pack cannot accept fractional coordinates".into(),
            ],
        );
    };
    let matrix = vec![vec![real, -imag], vec![imag, real]];
    let linear_algebra = evaluate_linear_algebra(&LinearAlgebraRequest {
        operation: LinearAlgebraOperation::Determinant,
        matrix: Some(matrix.clone()),
        vector_a: None,
        vector_b: None,
        domain: "finite_exact_integer".into(),
        requested_output: "determinant of complex real representation".into(),
        provenance: request.provenance.clone(),
    });
    if linear_algebra.status != LinearAlgebraStatus::Complete {
        return result(
            request,
            BridgeStatus::Unsupported,
            Some(matrix),
            Some(linear_algebra),
            None,
            vec!["delegated matrix determinant was outside the validated boundary".into()],
        );
    }
    result(
        request,
        BridgeStatus::Complete,
        Some(matrix),
        Some(linear_algebra),
        norm_squared(
            &Rational::new(real as i128, 1).expect("integer rational"),
            &Rational::new(imag as i128, 1).expect("integer rational"),
        ),
        Vec::new(),
    )
}

impl ComplexMatrixBridgeResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && match self.status {
                BridgeStatus::Complete => {
                    self.matrix.is_some()
                        && self.complex_source.is_some()
                        && self.linear_algebra.is_some()
                        && self.norm_squared.is_some()
                        && self
                            .linear_algebra
                            .as_ref()
                            .is_some_and(LinearAlgebraResult::replay_verified)
                }
                _ => true,
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(value: i128) -> Rational {
        Rational::new(value, 1).unwrap()
    }

    #[test]
    fn integral_pair_bridges_and_preserves_norm_determinant() {
        let output = bridge_complex_to_real_matrix(&ComplexMatrixBridgeRequest {
            complex: Some(ComplexArtifact::Pair {
                real: q(3),
                imag: q(-4),
            }),
            domain: DOMAIN.into(),
            ambiguity: None,
            provenance: vec!["unit-test".into()],
        });
        assert_eq!(output.status, BridgeStatus::Complete);
        assert_eq!(output.matrix, Some(vec![vec![3, 4], vec![-4, 3]]));
        assert_eq!(output.norm_squared, Some(q(25)));
        assert!(output.replay_verified());
    }

    #[test]
    fn fractional_and_ambiguous_inputs_fail_closed() {
        let fractional = bridge_complex_to_real_matrix(&ComplexMatrixBridgeRequest {
            complex: Some(ComplexArtifact::Pair {
                real: Rational::new(1, 2).unwrap(),
                imag: q(1),
            }),
            domain: DOMAIN.into(),
            ambiguity: None,
            provenance: vec!["unit-test".into()],
        });
        assert_eq!(fractional.status, BridgeStatus::NonIntegral);
        let ambiguous = bridge_complex_to_real_matrix(&ComplexMatrixBridgeRequest {
            complex: None,
            domain: DOMAIN.into(),
            ambiguity: Some("matrix convention unresolved".into()),
            provenance: vec!["unit-test".into()],
        });
        assert_eq!(ambiguous.status, BridgeStatus::Ambiguous);
    }
}
