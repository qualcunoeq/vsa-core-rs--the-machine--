//! Explicit technical-language frontend for bounded prime-field polynomials.
//!
//! Polynomial coefficients and the prime modulus must be written in the
//! report.  The frontend does not infer a coefficient domain or turn a
//! generic algebra question into a polynomial request.

use crate::polynomial_pack::{
    evaluate_polynomial, Polynomial, PolynomialOperation, PolynomialRequest, PolynomialResult,
    PolynomialStatus,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolynomialFrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolynomialFrontendResult {
    pub status: PolynomialFrontendStatus,
    pub request: Option<PolynomialRequest>,
    pub operation: Option<PolynomialOperation>,
    pub alternatives: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn payload(result: &PolynomialFrontendResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.request,
        result.operation,
        &result.alternatives,
        &result.reasons,
        &result.provenance,
    )
}

fn finish(
    status: PolynomialFrontendStatus,
    request: Option<PolynomialRequest>,
    operation: Option<PolynomialOperation>,
    alternatives: Vec<String>,
    reasons: Vec<String>,
    text: &str,
) -> PolynomialFrontendResult {
    let mut result = PolynomialFrontendResult {
        status,
        request,
        operation,
        alternatives,
        reasons,
        provenance: vec![format!("polynomial-source-span:0..{}", text.len())],
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn polynomial(text: &str, label: &str) -> Option<Polynomial> {
    let lower = text.to_ascii_lowercase();
    let marker = format!("{label}=[");
    let start = lower.find(&marker)? + marker.len();
    let end = lower[start..].find(']')? + start;
    let coefficients = lower[start..end]
        .split(',')
        .map(|token| token.trim().parse::<u64>().ok())
        .collect::<Option<Vec<_>>>()?;
    let modulus = lower
        .find("mod=")
        .and_then(|index| lower[index + 4..].split_whitespace().next())
        .and_then(|token| {
            token
                .trim_matches(|c: char| !c.is_ascii_digit())
                .parse()
                .ok()
        })?;
    Some(Polynomial {
        coefficients,
        modulus,
    })
}

fn point(text: &str) -> Option<u64> {
    let lower = text.to_ascii_lowercase();
    let index = lower.find("point=")? + 6;
    lower[index..]
        .split_whitespace()
        .next()?
        .trim_matches(|c: char| !c.is_ascii_digit())
        .parse()
        .ok()
}

/// Parse one explicit bounded polynomial operation.
pub fn formalize_polynomial_text(text: &str, case_id: &str) -> PolynomialFrontendResult {
    let lower = text.to_ascii_lowercase();
    if !lower.contains("polynomial") && !lower.contains("prime field") {
        return finish(
            PolynomialFrontendStatus::Unsupported,
            None,
            None,
            Vec::new(),
            vec!["an explicit polynomial or prime-field marker is required".into()],
            text,
        );
    }
    if [
        "minimal polynomial",
        "integer polynomial",
        "analytic",
        "unbounded factor",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        return finish(
            PolynomialFrontendStatus::Unsupported,
            None,
            None,
            Vec::new(),
            vec!["request exceeds the bounded prime-field polynomial contract".into()],
            text,
        );
    }
    let operations = [
        ("add", PolynomialOperation::Add),
        ("multiply", PolynomialOperation::Multiply),
        ("divide", PolynomialOperation::Divide),
        ("gcd", PolynomialOperation::Gcd),
        ("evaluate", PolynomialOperation::Evaluate),
        ("roots", PolynomialOperation::Roots),
        ("factor quadratic", PolynomialOperation::FactorQuadratic),
    ]
    .iter()
    .filter(|(marker, _)| lower.contains(marker))
    .map(|(_, operation)| *operation)
    .collect::<Vec<_>>();
    let mut operations = operations;
    operations.sort_by_key(|operation| *operation as u8);
    operations.dedup();
    if operations.len() != 1 {
        return finish(
            if operations.is_empty() {
                PolynomialFrontendStatus::Missing
            } else {
                PolynomialFrontendStatus::Ambiguous
            },
            None,
            None,
            operations
                .iter()
                .map(|operation| format!("{operation:?}"))
                .collect(),
            vec!["one explicit polynomial operation is required".into()],
            text,
        );
    }
    let operation = operations[0];
    let Some(left) = polynomial(text, "p") else {
        return finish(
            PolynomialFrontendStatus::Missing,
            None,
            Some(operation),
            Vec::new(),
            vec!["p=[...] and mod=... are required".into()],
            text,
        );
    };
    let right = polynomial(text, "q");
    if matches!(
        operation,
        PolynomialOperation::Add
            | PolynomialOperation::Multiply
            | PolynomialOperation::Divide
            | PolynomialOperation::Gcd
    ) && right.is_none()
    {
        return finish(
            PolynomialFrontendStatus::Missing,
            None,
            Some(operation),
            Vec::new(),
            vec!["binary polynomial operations require q=[...]".into()],
            text,
        );
    }
    if operation == PolynomialOperation::Evaluate && point(text).is_none() {
        return finish(
            PolynomialFrontendStatus::Missing,
            None,
            Some(operation),
            Vec::new(),
            vec!["evaluation requires point=...".into()],
            text,
        );
    }
    let request = PolynomialRequest {
        operation,
        left: Some(left),
        right,
        point: point(text),
        domain: "bounded_exact_prime_field_polynomial".into(),
        ambiguity: None,
        provenance: vec![format!("case:{case_id}"), text.into()],
    };
    finish(
        PolynomialFrontendStatus::Complete,
        Some(request),
        Some(operation),
        Vec::new(),
        Vec::new(),
        text,
    )
}

pub fn replay_verified(result: &PolynomialFrontendResult) -> bool {
    result.replay_hash == digest(&payload(result))
        && !result.provenance.is_empty()
        && (result.status != PolynomialFrontendStatus::Complete || result.request.is_some())
}

pub fn downstream_replay(result: &PolynomialFrontendResult) -> bool {
    result
        .request
        .as_ref()
        .map(|request| {
            let evaluated: PolynomialResult = evaluate_polynomial(request);
            evaluated.replay_verified()
                && (result.status != PolynomialFrontendStatus::Complete
                    || evaluated.status == PolynomialStatus::Complete)
        })
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_prime_field_polynomial_replays_and_evaluates() {
        let result = formalize_polynomial_text(
            "Over a prime field, evaluate polynomial p=[1,2,1] mod=5 at point=2.",
            "poly-1",
        );
        assert_eq!(result.status, PolynomialFrontendStatus::Complete);
        assert!(replay_verified(&result));
        assert!(downstream_replay(&result));
    }

    #[test]
    fn polynomial_boundaries_fail_closed() {
        assert_eq!(
            formalize_polynomial_text(
                "Find the roots of polynomial p=[1,2] without mod.",
                "missing"
            )
            .status,
            PolynomialFrontendStatus::Missing
        );
        assert_eq!(
            formalize_polynomial_text("Find the minimal polynomial of a matrix.", "unsupported")
                .status,
            PolynomialFrontendStatus::Unsupported
        );
    }
}
