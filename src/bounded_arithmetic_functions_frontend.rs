//! Controlled technical-language frontend for bounded arithmetic functions.
//!
//! Only explicit operation phrases and positive integer bindings are accepted.
//! The frontend never infers that a generic "number theory" or "prime"
//! question means one of these finite functions.

use crate::bounded_arithmetic_functions_pack::{
    ArithmeticFunctionOperation, ArithmeticFunctionRequest,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArithmeticFrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArithmeticFrontendResult {
    pub status: ArithmeticFrontendStatus,
    pub request: Option<ArithmeticFunctionRequest>,
    pub unresolved: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn finish(mut result: ArithmeticFrontendResult) -> ArithmeticFrontendResult {
    result.replay_hash.clear();
    result.replay_hash = digest(&result);
    result
}

pub fn replay_verified(result: &ArithmeticFrontendResult) -> bool {
    let mut copy = result.clone();
    let hash = copy.replay_hash.clone();
    copy.replay_hash.clear();
    hash == digest(&copy) && !result.provenance.is_empty()
}

fn integer_after(text: &str, labels: &[&str]) -> Option<u64> {
    labels.iter().find_map(|label| {
        let start = text.find(label)? + label.len();
        let token = text[start..]
            .trim_start_matches(|c: char| c == '=' || c == ':' || c.is_whitespace())
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>();
        (!token.is_empty()).then(|| token.parse().ok()).flatten()
    })
}

fn marker_count(text: &str, marker: &str) -> usize {
    text.match_indices(marker).count()
}

fn output(
    status: ArithmeticFrontendStatus,
    request: Option<ArithmeticFunctionRequest>,
    unresolved: Vec<String>,
    provenance: Vec<String>,
) -> ArithmeticFrontendResult {
    finish(ArithmeticFrontendResult {
        status,
        request,
        unresolved,
        provenance,
        replay_hash: String::new(),
    })
}

/// Parse one bounded arithmetic-function request from controlled text.
pub fn formalize(text: &str, case_id: &str) -> ArithmeticFrontendResult {
    let lower = text.to_ascii_lowercase();
    let provenance = vec![
        format!("arithmetic-functions-frontend:{case_id}"),
        format!("source-span:0..{}", text.len()),
        "explicit-bounded-arithmetic-function-grammar".into(),
    ];
    if [
        "analytic",
        "asymptotic",
        "dirichlet series",
        "l-function",
        "infinite",
        "unbounded",
        "approximate",
    ]
    .iter()
    .any(|term| lower.contains(term))
    {
        return output(
            ArithmeticFrontendStatus::Unsupported,
            None,
            vec!["analytic or unbounded semantics exceed the finite arithmetic contract".into()],
            provenance,
        );
    }
    let operation_families: &[&[&str]] = &[
        &["number of divisors", "divisor count", "tau(", "τ("],
        &["sum of divisors", "divisor sum", "sigma(", "σ("],
        &["möbius", "mobius", "mu(", "μ("],
        &[
            "prime-counting",
            "prime counting",
            "number of primes up to",
            "pi(",
            "π(",
        ],
        &["phi(", "φ(", "totient"],
    ];
    if operation_families
        .iter()
        .filter(|family| family.iter().any(|marker| lower.contains(marker)))
        .count()
        > 1
    {
        return output(
            ArithmeticFrontendStatus::Ambiguous,
            None,
            vec!["multiple arithmetic-function operations or scoped formulas are present".into()],
            provenance,
        );
    }
    if ["n=", "n =", "value=", "value "]
        .iter()
        .any(|marker| marker_count(&lower, marker) > 1)
    {
        return output(
            ArithmeticFrontendStatus::Ambiguous,
            None,
            vec!["the arithmetic-function input appears in multiple local scopes".into()],
            provenance,
        );
    }
    let mut operations = Vec::new();
    if lower.contains("number of divisors")
        || lower.contains("divisor count")
        || lower.contains("tau(")
        || lower.contains("τ(")
    {
        operations.push(ArithmeticFunctionOperation::DivisorCount);
    }
    if lower.contains("sum of divisors")
        || lower.contains("divisor sum")
        || lower.contains("sigma(")
        || lower.contains("σ(")
    {
        operations.push(ArithmeticFunctionOperation::DivisorSum);
    }
    if lower.contains("möbius")
        || lower.contains("mobius")
        || lower.contains("mu(")
        || lower.contains("μ(")
    {
        operations.push(ArithmeticFunctionOperation::Mobius);
    }
    if lower.contains("prime-counting")
        || lower.contains("prime counting")
        || lower.contains("number of primes up to")
        || lower.contains("pi(")
        || lower.contains("π(")
    {
        operations.push(ArithmeticFunctionOperation::PrimeCounting);
    }
    operations.sort_by_key(|operation| format!("{operation:?}"));
    operations.dedup();
    let operation = match operations.as_slice() {
        [operation] => *operation,
        [] => {
            if lower.contains("arithmetic function") || lower.contains("arithmetic-function") {
                return output(
                    ArithmeticFrontendStatus::Ambiguous,
                    None,
                    vec!["an arithmetic-function request is present but its operation is unspecified".into()],
                    provenance,
                );
            }
            return output(
                ArithmeticFrontendStatus::Missing,
                None,
                vec!["no bounded arithmetic-function operation was identified".into()],
                provenance,
            );
        }
        _ => {
            return output(
                ArithmeticFrontendStatus::Ambiguous,
                None,
                vec!["more than one arithmetic-function operation is plausible".into()],
                provenance,
            )
        }
    };
    let value = integer_after(
        &lower,
        &["n=", "n =", "value=", "value ", "at n ", "up to n="],
    );
    let Some(value) = value else {
        return output(
            ArithmeticFrontendStatus::Missing,
            None,
            vec!["the positive integer argument is not explicit".into()],
            provenance,
        );
    };
    let request = ArithmeticFunctionRequest {
        operation,
        value: Some(value),
        domain: "bounded_arithmetic_functions".into(),
        ambiguity: None,
        provenance: provenance.clone(),
    };
    output(
        ArithmeticFrontendStatus::Complete,
        Some(request),
        Vec::new(),
        provenance,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_operations_bind() {
        let divisor = formalize("Find the number of divisors of n=36.", "divisor");
        assert_eq!(divisor.status, ArithmeticFrontendStatus::Complete);
        assert!(replay_verified(&divisor));
        let mobius = formalize("Evaluate the Möbius function μ(n=30).", "mobius");
        assert_eq!(mobius.status, ArithmeticFrontendStatus::Complete);
    }

    #[test]
    fn ambiguity_and_unsupported_are_preserved() {
        let ambiguous = formalize(
            "Find the divisor count or divisor sum at n=36.",
            "ambiguous",
        );
        assert_eq!(ambiguous.status, ArithmeticFrontendStatus::Ambiguous);
        let unsupported = formalize(
            "Estimate the asymptotic prime-counting function.",
            "unsupported",
        );
        assert_eq!(unsupported.status, ArithmeticFrontendStatus::Unsupported);
        let scoped = formalize(
            "A quoted formula contains μ(n=12) and μ(n=36); select neither scope.",
            "scoped",
        );
        assert_eq!(scoped.status, ArithmeticFrontendStatus::Ambiguous);
    }
}
