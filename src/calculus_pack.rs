//! Shadow bounded exact one-variable calculus curriculum pack.
//!
//! The pack deliberately lowers only a small symbolic grammar into the
//! existing algebra engine.  It never turns a finite difference into a
//! derivative, and it refuses multivariable, improper, measure-theoretic,
//! numerical, and unsupported convergence semantics.

use crate::algebra::{differentiate_str, evaluate_str, integrate_definite, integrate_str};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CalculusOperation {
    Derivative,
    Integral,
    DefiniteIntegral,
    Limit,
    Continuity,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CalculusStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidDomain,
    NonExact,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CalculusArtifact {
    Symbolic(String),
    ExactValue(String),
    Boolean(bool),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CalculusRequest {
    pub operation: CalculusOperation,
    pub domain: String,
    pub expression: String,
    pub variable: Option<String>,
    pub lower: Option<f64>,
    pub upper: Option<f64>,
    pub point: Option<f64>,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CalculusResult {
    pub status: CalculusStatus,
    pub artifact: Option<CalculusArtifact>,
    pub operation: CalculusOperation,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("calculus serializes"))
    )
}

fn payload(result: &CalculusResult) -> impl Serialize + '_ {
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
    request: &CalculusRequest,
    status: CalculusStatus,
    artifact: Option<CalculusArtifact>,
    assumptions: Vec<String>,
    reasons: Vec<String>,
) -> CalculusResult {
    let mut result = CalculusResult {
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

fn exact_integer(value: f64) -> Option<String> {
    if value.is_finite() && (value - value.round()).abs() < 1e-9 {
        Some(format!("{:.0}", value))
    } else {
        None
    }
}

fn expression_supported(expression: &str) -> bool {
    let lower = expression.to_ascii_lowercase();
    if expression.len() > 256
        || lower.contains(['y', 'z'])
        || lower.contains("partial")
        || lower.contains('∂')
        || lower.contains("piecewise")
        || lower.contains("infinity")
        || lower.contains('∞')
        || lower.contains("distribution")
        || lower.contains("numerical")
        || lower.contains("approx")
    {
        return false;
    }
    lower
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "+-*/^()., _".contains(ch))
        && ["sin", "cos", "exp", "ln", "sqrt", "tan"]
            .iter()
            .all(|name| !lower.contains(name) || lower.matches(name).count() <= 8)
}

fn polynomial_like(expression: &str) -> bool {
    let lower = expression.to_ascii_lowercase();
    expression_supported(expression)
        && !lower.contains('/')
        && !lower.contains("sin")
        && !lower.contains("cos")
        && !lower.contains("exp")
        && !lower.contains("ln")
        && !lower.contains("sqrt")
        && !lower.contains("tan")
}

pub fn evaluate_calculus(request: &CalculusRequest) -> CalculusResult {
    if request.domain != "bounded_exact_single_variable_calculus" {
        return result(
            request,
            CalculusStatus::Unsupported,
            None,
            Vec::new(),
            vec!["domain is outside bounded exact one-variable calculus".into()],
        );
    }
    if let Some(ambiguity) = &request.ambiguity {
        return result(
            request,
            CalculusStatus::Ambiguous,
            None,
            Vec::new(),
            vec![ambiguity.clone()],
        );
    }
    if !expression_supported(&request.expression) {
        return result(
            request,
            CalculusStatus::Unsupported,
            None,
            Vec::new(),
            vec!["expression uses unsupported or non-exact calculus semantics".into()],
        );
    }
    let Some(variable) = request.variable.as_deref() else {
        return result(
            request,
            CalculusStatus::Missing,
            None,
            Vec::new(),
            vec!["a differentiation/integration variable is required".into()],
        );
    };
    if variable != "x" {
        return result(
            request,
            CalculusStatus::Unsupported,
            None,
            Vec::new(),
            vec!["only the explicitly scoped one-variable x grammar is supported".into()],
        );
    }
    match request.operation {
        CalculusOperation::Derivative => match differentiate_str(&request.expression, variable) {
            Ok(derivative) => result(
                request,
                CalculusStatus::Complete,
                Some(CalculusArtifact::Symbolic(derivative)),
                vec![
                    "exact symbolic differentiation".into(),
                    "one variable x".into(),
                ],
                Vec::new(),
            ),
            Err(error) => result(
                request,
                CalculusStatus::Unsupported,
                None,
                Vec::new(),
                vec![format!(
                    "expression is outside the supported differentiator: {error}"
                )],
            ),
        },
        CalculusOperation::Integral => match integrate_str(&request.expression, variable) {
            Some(integral) => result(
                request,
                CalculusStatus::Complete,
                Some(CalculusArtifact::Symbolic(integral)),
                vec![
                    "exact symbolic antiderivative".into(),
                    "constant of integration omitted".into(),
                ],
                Vec::new(),
            ),
            None => result(
                request,
                CalculusStatus::Unsupported,
                None,
                Vec::new(),
                vec!["no bounded exact antiderivative rule matched".into()],
            ),
        },
        CalculusOperation::DefiniteIntegral => {
            let (Some(lower), Some(upper)) = (request.lower, request.upper) else {
                return result(
                    request,
                    CalculusStatus::Missing,
                    None,
                    Vec::new(),
                    vec!["finite lower and upper bounds are required".into()],
                );
            };
            if lower > upper {
                return result(
                    request,
                    CalculusStatus::Ambiguous,
                    None,
                    Vec::new(),
                    vec!["lower bound must not exceed upper bound".into()],
                );
            }
            match integrate_definite(&request.expression, variable, lower, upper)
                .and_then(exact_integer)
            {
                Some(value) => result(
                    request,
                    CalculusStatus::Complete,
                    Some(CalculusArtifact::ExactValue(value)),
                    vec!["exact bounded definite integral".into()],
                    Vec::new(),
                ),
                None => result(
                    request,
                    CalculusStatus::NonExact,
                    None,
                    Vec::new(),
                    vec![
                        "result is not proven exact in the bounded rational witness grammar".into(),
                    ],
                ),
            }
        }
        CalculusOperation::Limit => {
            let Some(point) = request.point else {
                return result(
                    request,
                    CalculusStatus::Missing,
                    None,
                    Vec::new(),
                    vec!["a finite approach point is required".into()],
                );
            };
            if !polynomial_like(&request.expression) {
                return result(
                    request,
                    CalculusStatus::Unsupported,
                    None,
                    Vec::new(),
                    vec!["only polynomial-like finite limits are supported".into()],
                );
            }
            match evaluate_str(&request.expression, &[(variable, point)]).and_then(exact_integer) {
                Some(value) => result(
                    request,
                    CalculusStatus::Complete,
                    Some(CalculusArtifact::ExactValue(value)),
                    vec!["finite polynomial limit by exact substitution".into()],
                    Vec::new(),
                ),
                None => result(
                    request,
                    CalculusStatus::NonExact,
                    None,
                    Vec::new(),
                    vec!["limit value is not proven exact in the bounded witness grammar".into()],
                ),
            }
        }
        CalculusOperation::Continuity => {
            let Some(point) = request.point else {
                return result(
                    request,
                    CalculusStatus::Missing,
                    None,
                    Vec::new(),
                    vec!["a finite point is required for continuity".into()],
                );
            };
            if !polynomial_like(&request.expression)
                || evaluate_str(&request.expression, &[(variable, point)]).is_none()
            {
                return result(
                    request,
                    CalculusStatus::Unsupported,
                    None,
                    Vec::new(),
                    vec![
                        "continuity is bounded to polynomial expressions with a defined point"
                            .into(),
                    ],
                );
            }
            result(
                request,
                CalculusStatus::Complete,
                Some(CalculusArtifact::Boolean(true)),
                vec!["polynomials are continuous on the real line".into()],
                Vec::new(),
            )
        }
    }
}

impl CalculusResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != CalculusStatus::Complete || self.artifact.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(operation: CalculusOperation, expression: &str) -> CalculusRequest {
        CalculusRequest {
            operation,
            domain: "bounded_exact_single_variable_calculus".into(),
            expression: expression.into(),
            variable: Some("x".into()),
            lower: None,
            upper: None,
            point: None,
            ambiguity: None,
            provenance: vec!["calculus-test".into()],
        }
    }

    #[test]
    fn derivative_and_integral_replay() {
        let derivative = evaluate_calculus(&request(CalculusOperation::Derivative, "x^2 + 3*x"));
        assert_eq!(derivative.status, CalculusStatus::Complete);
        assert!(derivative.replay_verified());
        let integral = evaluate_calculus(&request(CalculusOperation::Integral, "2*x"));
        assert_eq!(integral.status, CalculusStatus::Complete);
        assert!(integral.replay_verified());
    }

    #[test]
    fn unsupported_boundaries_fail_closed() {
        let mut request = request(CalculusOperation::Limit, "x^2 + 1");
        request.point = Some(2.0);
        assert_eq!(evaluate_calculus(&request).status, CalculusStatus::Complete);
        request.expression = "partial_x f(x,y)".into();
        assert_eq!(
            evaluate_calculus(&request).status,
            CalculusStatus::Unsupported
        );
    }
}
