//! Shadow bounded real-analysis foundation pack.
//!
//! This pack validates theorem applicability for a small exact one-variable
//! grammar. It does not generate arbitrary epsilon-delta proofs or infer
//! convergence from numerical samples.

use crate::algebra::evaluate_str;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisOperation {
    Monotonicity,
    Boundedness,
    ExtremeValueApplicability,
    IntermediateValueApplicability,
    SequenceConvergence,
    ContinuityComposition,
    OneSidedLimit,
    DiscontinuityClassification,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidAssumptions,
    NonExact,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MonotoneDirection {
    Nondecreasing,
    Nonincreasing,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LimitSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscontinuityKind {
    Removable,
    Genuine,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AnalysisArtifact {
    TheoremApplicable(String),
    SequenceLimit(String),
    ExactLimit(String),
    Discontinuity(DiscontinuityKind),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnalysisRequest {
    pub operation: AnalysisOperation,
    pub domain: String,
    pub expression: String,
    pub variable: Option<String>,
    pub interval: Option<(f64, f64)>,
    pub point: Option<f64>,
    pub side: Option<LimitSide>,
    pub direction: Option<MonotoneDirection>,
    pub endpoint_values: Option<(i64, i64)>,
    pub target_value: Option<i64>,
    pub sequence_initial: Option<i64>,
    pub sequence_ratio: Option<(i64, i64)>,
    pub composition_declared: bool,
    pub assumptions: Vec<String>,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnalysisResult {
    pub status: AnalysisStatus,
    pub artifact: Option<AnalysisArtifact>,
    pub operation: AnalysisOperation,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("analysis serializes"))
    )
}

fn payload(result: &AnalysisResult) -> impl Serialize + '_ {
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
    request: &AnalysisRequest,
    status: AnalysisStatus,
    artifact: Option<AnalysisArtifact>,
    assumptions: Vec<String>,
    reasons: Vec<String>,
) -> AnalysisResult {
    let mut result = AnalysisResult {
        status,
        artifact,
        operation: request.operation,
        assumptions,
        reasons,
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn polynomial(expression: &str) -> bool {
    let lower = expression.to_ascii_lowercase();
    !expression.is_empty()
        && expression.len() <= 256
        && !lower.contains('/')
        && !lower.contains("sin")
        && !lower.contains("cos")
        && !lower.contains("exp")
        && !lower.contains("ln")
        && !lower.contains("sqrt")
        && !lower.contains('y')
        && !lower.contains('z')
        && lower
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || "+-^(). _".contains(ch))
}

fn rational(expression: &str) -> bool {
    let lower = expression.to_ascii_lowercase();
    polynomial(expression) || (lower.contains('/') && !lower.contains("//"))
}

fn has(request: &AnalysisRequest, name: &str) -> bool {
    request.assumptions.iter().any(|item| item == name)
}

pub fn evaluate_analysis(request: &AnalysisRequest) -> AnalysisResult {
    if request.domain != "bounded_exact_real_analysis" {
        return result(
            request,
            AnalysisStatus::Unsupported,
            None,
            Vec::new(),
            vec!["domain is outside bounded exact real analysis".into()],
        );
    }
    if let Some(ambiguity) = &request.ambiguity {
        return result(
            request,
            AnalysisStatus::Ambiguous,
            None,
            Vec::new(),
            vec![ambiguity.clone()],
        );
    }
    if request.variable.as_deref() != Some("x") {
        return result(
            request,
            AnalysisStatus::Unsupported,
            None,
            Vec::new(),
            vec!["only the explicitly scoped x grammar is supported".into()],
        );
    }
    match request.operation {
        AnalysisOperation::Monotonicity => {
            let Some(interval) = request.interval else {
                return result(
                    request,
                    AnalysisStatus::Missing,
                    None,
                    Vec::new(),
                    vec!["closed interval is required".into()],
                );
            };
            if interval.0 > interval.1 || !polynomial(&request.expression) {
                return result(
                    request,
                    AnalysisStatus::Unsupported,
                    None,
                    Vec::new(),
                    vec!["only polynomial closed-interval monotonicity is supported".into()],
                );
            }
            let Some(direction) = request.direction else {
                return result(
                    request,
                    AnalysisStatus::Missing,
                    None,
                    Vec::new(),
                    vec!["derivative-sign direction is required".into()],
                );
            };
            if !has(request, "polynomial")
                || !has(request, "closed_interval")
                || !has(request, "derivative_sign_verified")
            {
                return result(
                    request,
                    AnalysisStatus::InvalidAssumptions,
                    None,
                    Vec::new(),
                    vec![
                        "polynomial, closed interval, and verified derivative sign are required"
                            .into(),
                    ],
                );
            }
            result(
                request,
                AnalysisStatus::Complete,
                Some(AnalysisArtifact::TheoremApplicable(format!(
                    "{:?}",
                    direction
                ))),
                request.assumptions.clone(),
                vec!["monotonicity theorem assumptions verified".into()],
            )
        }
        AnalysisOperation::Boundedness | AnalysisOperation::ExtremeValueApplicability => {
            let Some(interval) = request.interval else {
                return result(
                    request,
                    AnalysisStatus::Missing,
                    None,
                    Vec::new(),
                    vec!["explicit closed interval is required".into()],
                );
            };
            if interval.0 > interval.1 || !rational(&request.expression) {
                return result(
                    request,
                    AnalysisStatus::Unsupported,
                    None,
                    Vec::new(),
                    vec!["expression or interval is outside bounded theorem grammar".into()],
                );
            }
            let required = if request.operation == AnalysisOperation::Boundedness {
                "closed_interval"
            } else {
                "continuous_on_interval"
            };
            if !has(request, "closed_interval") || !has(request, required) {
                return result(
                    request,
                    AnalysisStatus::InvalidAssumptions,
                    None,
                    Vec::new(),
                    vec![format!("{required} assumption is required")],
                );
            }
            let theorem = if request.operation == AnalysisOperation::Boundedness {
                "bounded_on_closed_interval"
            } else {
                "extreme_value_applicable"
            };
            result(
                request,
                AnalysisStatus::Complete,
                Some(AnalysisArtifact::TheoremApplicable(theorem.into())),
                request.assumptions.clone(),
                vec!["closed-interval theorem assumptions verified".into()],
            )
        }
        AnalysisOperation::IntermediateValueApplicability => {
            let Some(interval) = request.interval else {
                return result(
                    request,
                    AnalysisStatus::Missing,
                    None,
                    Vec::new(),
                    vec!["explicit interval is required".into()],
                );
            };
            let Some((left, right)) = request.endpoint_values else {
                return result(
                    request,
                    AnalysisStatus::Missing,
                    None,
                    Vec::new(),
                    vec!["endpoint values are required".into()],
                );
            };
            let Some(target) = request.target_value else {
                return result(
                    request,
                    AnalysisStatus::Missing,
                    None,
                    Vec::new(),
                    vec!["target value is required".into()],
                );
            };
            if interval.0 > interval.1
                || !polynomial(&request.expression)
                || target < left.min(right)
                || target > left.max(right)
            {
                return result(
                    request,
                    AnalysisStatus::Unsupported,
                    None,
                    Vec::new(),
                    vec!["IVT target or expression is outside bounded grammar".into()],
                );
            }
            if !has(request, "continuous_on_interval") || !has(request, "closed_interval") {
                return result(
                    request,
                    AnalysisStatus::InvalidAssumptions,
                    None,
                    Vec::new(),
                    vec!["continuity and interval assumptions are required".into()],
                );
            }
            result(
                request,
                AnalysisStatus::Complete,
                Some(AnalysisArtifact::TheoremApplicable(
                    "intermediate_value_applicable".into(),
                )),
                request.assumptions.clone(),
                vec!["intermediate-value theorem assumptions verified".into()],
            )
        }
        AnalysisOperation::SequenceConvergence => {
            let (Some(initial), Some((numerator, denominator))) =
                (request.sequence_initial, request.sequence_ratio)
            else {
                return result(
                    request,
                    AnalysisStatus::Missing,
                    None,
                    Vec::new(),
                    vec!["explicit geometric sequence parameters are required".into()],
                );
            };
            if denominator == 0 || numerator.abs() >= denominator.abs() {
                return result(
                    request,
                    AnalysisStatus::Unsupported,
                    None,
                    Vec::new(),
                    vec![
                        "only geometric sequences with absolute ratio below one are supported"
                            .into(),
                    ],
                );
            }
            if !has(request, "geometric_sequence") || !has(request, "ratio_absolute_less_than_one")
            {
                return result(
                    request,
                    AnalysisStatus::InvalidAssumptions,
                    None,
                    Vec::new(),
                    vec!["geometric form and ratio bound must be explicit".into()],
                );
            }
            let limit = if initial == 0 { "0" } else { "0" };
            result(
                request,
                AnalysisStatus::Complete,
                Some(AnalysisArtifact::SequenceLimit(limit.into())),
                request.assumptions.clone(),
                vec!["geometric convergence theorem assumptions verified".into()],
            )
        }
        AnalysisOperation::ContinuityComposition => {
            if !request.composition_declared
                || !has(request, "continuous_components")
                || !polynomial(&request.expression)
            {
                return result(
                    request,
                    AnalysisStatus::InvalidAssumptions,
                    None,
                    Vec::new(),
                    vec!["continuous component declarations are required".into()],
                );
            }
            result(
                request,
                AnalysisStatus::Complete,
                Some(AnalysisArtifact::TheoremApplicable(
                    "continuity_of_composition".into(),
                )),
                request.assumptions.clone(),
                vec!["continuity-composition theorem assumptions verified".into()],
            )
        }
        AnalysisOperation::OneSidedLimit => {
            let Some(point) = request.point else {
                return result(
                    request,
                    AnalysisStatus::Missing,
                    None,
                    Vec::new(),
                    vec!["finite point is required".into()],
                );
            };
            if request.side.is_none() || !polynomial(&request.expression) || !point.is_finite() {
                return result(
                    request,
                    AnalysisStatus::Unsupported,
                    None,
                    Vec::new(),
                    vec!["only finite polynomial one-sided limits are supported".into()],
                );
            }
            let Some(value) = evaluate_str(&request.expression, &[("x", point)]) else {
                return result(
                    request,
                    AnalysisStatus::NonExact,
                    None,
                    Vec::new(),
                    vec!["limit value is not exactly witnessed".into()],
                );
            };
            if (value - value.round()).abs() > 1e-9 {
                return result(
                    request,
                    AnalysisStatus::NonExact,
                    None,
                    Vec::new(),
                    vec!["limit value is not an exact integer witness".into()],
                );
            }
            result(
                request,
                AnalysisStatus::Complete,
                Some(AnalysisArtifact::ExactLimit(format!("{value:.0}"))),
                vec!["polynomial continuity".into()],
                vec!["one-sided limit equals substitution".into()],
            )
        }
        AnalysisOperation::DiscontinuityClassification => {
            let Some(point) = request.point else {
                return result(
                    request,
                    AnalysisStatus::Missing,
                    None,
                    Vec::new(),
                    vec!["finite discontinuity point is required".into()],
                );
            };
            let expression = request.expression.replace(' ', "");
            let kind = if expression == "(x^2-1)/(x-1)" && (point - 1.0).abs() < f64::EPSILON {
                DiscontinuityKind::Removable
            } else if expression == "1/(x-1)" && (point - 1.0).abs() < f64::EPSILON {
                DiscontinuityKind::Genuine
            } else {
                return result(
                    request,
                    AnalysisStatus::Unsupported,
                    None,
                    Vec::new(),
                    vec!["only explicit bounded rational discontinuity forms are supported".into()],
                );
            };
            result(
                request,
                AnalysisStatus::Complete,
                Some(AnalysisArtifact::Discontinuity(kind)),
                vec!["explicit rational form and excluded point".into()],
                Vec::new(),
            )
        }
    }
}

impl AnalysisResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != AnalysisStatus::Complete || self.artifact.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(operation: AnalysisOperation) -> AnalysisRequest {
        AnalysisRequest {
            operation,
            domain: "bounded_exact_real_analysis".into(),
            expression: "x^2 + 1".into(),
            variable: Some("x".into()),
            interval: Some((0.0, 2.0)),
            point: Some(1.0),
            side: Some(LimitSide::Left),
            direction: Some(MonotoneDirection::Nondecreasing),
            endpoint_values: Some((1, 5)),
            target_value: Some(3),
            sequence_initial: Some(2),
            sequence_ratio: Some((1, 2)),
            composition_declared: true,
            assumptions: vec![
                "polynomial".into(),
                "closed_interval".into(),
                "derivative_sign_verified".into(),
                "continuous_on_interval".into(),
                "geometric_sequence".into(),
                "ratio_absolute_less_than_one".into(),
                "continuous_components".into(),
            ],
            ambiguity: None,
            provenance: vec!["analysis-test".into()],
        }
    }

    #[test]
    fn theorem_applicability_replays() {
        let result = evaluate_analysis(&request(AnalysisOperation::ExtremeValueApplicability));
        assert_eq!(result.status, AnalysisStatus::Complete);
        assert!(result.replay_verified());
    }

    #[test]
    fn missing_assumptions_fail_closed() {
        let mut request = request(AnalysisOperation::Monotonicity);
        request.assumptions.clear();
        assert_eq!(
            evaluate_analysis(&request).status,
            AnalysisStatus::InvalidAssumptions
        );
    }
}
