//! Explicit technical-language frontend for the bounded scalar ODE pack.
//!
//! The frontend accepts only labelled rational values and one unambiguous
//! operation.  It never infers a differential equation from generic calculus
//! vocabulary or silently supplies initial conditions.

use crate::ode_pack::{OdeOperation, OdeRequest, OdeResult, OdeStatus};
use crate::probability_pack::Rational;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OdeFrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OdeFrontendResult {
    pub status: OdeFrontendStatus,
    pub request: Option<OdeRequest>,
    pub alternatives: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn payload(result: &OdeFrontendResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.request,
        &result.alternatives,
        &result.reasons,
        &result.provenance,
    )
}

fn finish(
    status: OdeFrontendStatus,
    request: Option<OdeRequest>,
    alternatives: Vec<String>,
    reasons: Vec<String>,
    text: &str,
) -> OdeFrontendResult {
    let mut result = OdeFrontendResult {
        status,
        request,
        alternatives,
        reasons,
        provenance: vec![format!("ode-source-span:0..{}", text.len())],
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn rational_token(text: &str, label: &str) -> Option<Rational> {
    let lower = text.to_ascii_lowercase();
    let marker = format!("{label}=");
    let start = lower.find(&marker)? + marker.len();
    let token: String = lower[start..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || matches!(c, '-' | '/'))
        .collect();
    if token.is_empty() {
        return None;
    }
    let mut parts = token.split('/');
    let numerator = parts.next()?.parse::<i128>().ok()?;
    let denominator = parts.next().unwrap_or("1").parse::<i128>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Rational::new(numerator, denominator)
}

fn labels(text: &str, label: &str) -> bool {
    text.to_ascii_lowercase().contains(&format!("{label}="))
}

/// Parse one explicit bounded scalar ODE report into a typed request.
pub fn formalize_ode_text(text: &str, case_id: &str) -> OdeFrontendResult {
    let lower = text.to_ascii_lowercase();
    if !lower.contains("ode") && !lower.contains("differential equation") {
        return finish(
            OdeFrontendStatus::Unsupported,
            None,
            Vec::new(),
            vec!["an explicit ODE or differential-equation marker is required".into()],
            text,
        );
    }
    if [
        "nonlinear",
        "coupled",
        "numerical",
        "approximate",
        "continuous-time system",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        return finish(
            OdeFrontendStatus::Unsupported,
            None,
            Vec::new(),
            vec!["request is outside the bounded exact scalar ODE contract".into()],
            text,
        );
    }
    let mut operations = Vec::new();
    if lower.contains("constant derivative") || lower.contains("dx/dt") {
        operations.push(OdeOperation::ConstantDerivative);
    }
    if lower.contains("affine linear") || lower.contains("linear ode") {
        operations.push(OdeOperation::AffineLinear);
    }
    operations.sort_by_key(|operation| *operation as u8);
    operations.dedup();
    if operations.len() != 1 {
        return finish(
            if operations.is_empty() {
                OdeFrontendStatus::Missing
            } else {
                OdeFrontendStatus::Ambiguous
            },
            None,
            operations.iter().map(|op| format!("{op:?}")).collect(),
            vec!["one explicit bounded ODE operation is required".into()],
            text,
        );
    }
    let operation = operations[0];
    let required: Vec<&str> = match operation {
        OdeOperation::ConstantDerivative => vec!["initial", "derivative", "time"],
        OdeOperation::AffineLinear => vec!["initial", "coefficient", "forcing", "time"],
        _ => unreachable!(),
    };
    if required.iter().any(|label| !labels(text, label)) {
        return finish(
            OdeFrontendStatus::Missing,
            None,
            Vec::new(),
            vec!["all labelled initial, coefficient/derivative, forcing, and time values are required".into()],
            text,
        );
    }
    let initial = rational_token(text, "initial");
    let coefficient = rational_token(text, "coefficient");
    let forcing = rational_token(text, "forcing").or_else(|| rational_token(text, "derivative"));
    let time = rational_token(text, "time");
    if initial.is_none()
        || time.is_none()
        || (operation == OdeOperation::AffineLinear && coefficient.is_none())
        || forcing.is_none()
    {
        return finish(
            OdeFrontendStatus::Missing,
            None,
            Vec::new(),
            vec!["label values must be exact integers or rationals".into()],
            text,
        );
    }
    let request = OdeRequest {
        operation,
        initial,
        coefficient,
        forcing,
        time,
        domain: "bounded_exact_scalar_ode".into(),
        ambiguity: None,
        provenance: vec![format!("case:{case_id}"), text.into()],
    };
    finish(
        OdeFrontendStatus::Complete,
        Some(request),
        Vec::new(),
        Vec::new(),
        text,
    )
}

pub fn replay_verified(result: &OdeFrontendResult) -> bool {
    result.replay_hash == digest(&payload(result))
        && !result.provenance.is_empty()
        && (result.status != OdeFrontendStatus::Complete || result.request.is_some())
}

pub fn downstream_replay(result: &OdeFrontendResult) -> bool {
    result
        .request
        .as_ref()
        .map(|request| {
            let evaluated: OdeResult = crate::ode_pack::evaluate_ode(request);
            evaluated.replay_verified()
                && (result.status != OdeFrontendStatus::Complete
                    || evaluated.status == OdeStatus::Complete)
        })
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_constant_ode_replays_and_evaluates() {
        let result = formalize_ode_text(
            "Solve the bounded exact scalar ODE with constant derivative: initial=2 derivative=3 time=2.",
            "ode-1",
        );
        assert_eq!(result.status, OdeFrontendStatus::Complete);
        assert!(replay_verified(&result));
        assert!(downstream_replay(&result));
    }

    #[test]
    fn missing_and_unsupported_ode_boundaries_close() {
        assert_eq!(
            formalize_ode_text("Solve an ODE with constant derivative.", "missing").status,
            OdeFrontendStatus::Missing
        );
        assert_eq!(
            formalize_ode_text("Solve a nonlinear ODE numerically.", "unsupported").status,
            OdeFrontendStatus::Unsupported
        );
    }
}
