//! Source-derived bounded counting primitives.
//!
//! Ordered and unordered selection are distinct typed operations.  The pack
//! uses exact bounded integers and refuses unbounded, approximate, or
//! interpretation-dependent counting claims.

use crate::source_formula_pack::SourceCitation;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const DOMAIN: &str = "source_derived_bounded_counting";
pub const SOURCE_ID: &str = "openstax-contemporary-mathematics:counting-principles";
/// Maximum `n` accepted by the separately governed extended-combination
/// evaluator.  The ordinary counting evaluator intentionally retains its
/// original `n <= 20` scope for compatibility with its frozen corpus.
pub const EXTENDED_COMBINATION_MAX_N: u64 = 100_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CountingOperation {
    Product,
    Factorial,
    Permutation,
    Combination,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CountingRequest {
    pub operation: CountingOperation,
    pub n: Option<u64>,
    pub r: Option<u64>,
    pub factors: Vec<u64>,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CountingArtifact {
    ExactCount(u128),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CountingStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidRange,
    Overflow,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CountingSource {
    pub citation: SourceCitation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CountingResult {
    pub status: CountingStatus,
    pub artifact: Option<CountingArtifact>,
    pub operation: CountingOperation,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub source: CountingSource,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

pub fn source() -> CountingSource {
    CountingSource {
        citation: SourceCitation {
            source_id: SOURCE_ID.into(),
            title: "Contemporary Mathematics".into(),
            section: "7.1 The Multiplication Rule for Counting; 7.2 Permutations; 7.3 Combinations"
                .into(),
            url: "https://openstax.org/books/contemporary-mathematics/pages/7-3-combinations"
                .into(),
            license: "CC BY-NC-SA 4.0; OpenStax attribution required".into(),
            retrieved_utc: "2026-08-17".into(),
            evidence_span:
                "ordered permutations, unordered combinations, factorials, and multiplication rule"
                    .into(),
        },
    }
}
pub fn validate_source_document(document: &str) -> bool {
    [
        "SOURCE_ID:",
        "URL:",
        "EVIDENCE:",
        "permutation",
        "combination",
        "factorial",
    ]
    .iter()
    .all(|m| {
        document
            .to_ascii_lowercase()
            .contains(&m.to_ascii_lowercase())
    })
}
fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}
fn finish(mut result: CountingResult) -> CountingResult {
    result.replay_hash.clear();
    result.replay_hash = digest(&result);
    result
}
pub fn replay_verified(result: &CountingResult) -> bool {
    let mut copy = result.clone();
    let hash = copy.replay_hash.clone();
    copy.replay_hash.clear();
    hash == digest(&copy) && !result.provenance.is_empty()
}
fn factorial(value: u64) -> Option<u128> {
    (1..=value).try_fold(1_u128, |acc, item| acc.checked_mul(item as u128))
}

fn gcd_u128(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

/// Compute a binomial coefficient without factorial overflow.
///
/// Numerator and denominator factors are cancelled before multiplication, so
/// a result is accepted whenever the exact value fits in `u128`, even when
/// `n` itself is much larger than the ordinary factorial scope.
fn checked_combination(n: u64, r: u64) -> Option<u128> {
    let terms = r.min(n - r);
    let mut value = 1_u128;
    for index in 1..=terms {
        let mut numerator = (n - terms + index) as u128;
        let mut denominator = index as u128;
        let common = gcd_u128(numerator, denominator);
        numerator /= common;
        denominator /= common;
        let common = gcd_u128(value, denominator);
        value /= common;
        denominator /= common;
        if denominator != 1 {
            return None;
        }
        value = value.checked_mul(numerator)?;
    }
    Some(value)
}

/// Evaluate only the extended exact-combination scope.
///
/// This is deliberately separate from [`evaluate`].  The original evaluator
/// and its frozen pressure corpus retain the `n <= 20` contract; callers must
/// explicitly choose this extension and preserve its distinct receipt.
pub fn evaluate_extended_combination(request: &CountingRequest) -> CountingResult {
    let assumptions = vec![
        "finite exact counting model".into(),
        format!(
            "extended combinations require 0 <= r <= n <= {} and an exact u128 result",
            EXTENDED_COMBINATION_MAX_N
        ),
    ];
    let finish_with = |status, artifact, reasons: Vec<String>| {
        finish(CountingResult {
            status,
            artifact,
            operation: request.operation,
            assumptions: assumptions.clone(),
            reasons,
            source: source(),
            provenance: request.provenance.clone(),
            replay_hash: String::new(),
        })
    };
    if request.provenance.is_empty() {
        return finish_with(
            CountingStatus::Missing,
            None,
            vec!["provenance is required".into()],
        );
    }
    if request.operation != CountingOperation::Combination {
        return finish_with(
            CountingStatus::Unsupported,
            None,
            vec!["the extended evaluator accepts combinations only".into()],
        );
    }
    if let Some(reason) = &request.ambiguity {
        return finish_with(CountingStatus::Ambiguous, None, vec![reason.clone()]);
    }
    let (Some(n), Some(r)) = (request.n, request.r) else {
        return finish_with(
            CountingStatus::Missing,
            None,
            vec!["combination requires n and r".into()],
        );
    };
    if n > EXTENDED_COMBINATION_MAX_N || r > n {
        return finish_with(
            CountingStatus::InvalidRange,
            None,
            vec![format!(
                "extended combination requires 0 <= r <= n <= {}",
                EXTENDED_COMBINATION_MAX_N
            )],
        );
    }
    match checked_combination(n, r) {
        Some(value) => finish_with(
            CountingStatus::Complete,
            Some(CountingArtifact::ExactCount(value)),
            Vec::new(),
        ),
        None => finish_with(
            CountingStatus::Overflow,
            None,
            vec!["exact extended combination overflowed u128".into()],
        ),
    }
}

pub fn evaluate(request: &CountingRequest) -> CountingResult {
    let assumptions = vec![
        "finite exact counting model".into(),
        "all counts are bounded by n <= 20".into(),
    ];
    let finish_with = |status, artifact, reasons: Vec<String>| {
        finish(CountingResult {
            status,
            artifact,
            operation: request.operation,
            assumptions: assumptions.clone(),
            reasons,
            source: source(),
            provenance: request.provenance.clone(),
            replay_hash: String::new(),
        })
    };
    if request.provenance.is_empty() {
        return finish_with(
            CountingStatus::Missing,
            None,
            vec!["provenance is required".into()],
        );
    }
    if let Some(reason) = &request.ambiguity {
        return finish_with(CountingStatus::Ambiguous, None, vec![reason.clone()]);
    }
    let value = match request.operation {
        CountingOperation::Product => request
            .factors
            .iter()
            .try_fold(1_u128, |acc, factor| acc.checked_mul(*factor as u128)),
        CountingOperation::Factorial => request.n.filter(|n| *n <= 20).and_then(factorial),
        CountingOperation::Permutation => {
            let (Some(n), Some(r)) = (request.n, request.r) else {
                return finish_with(
                    CountingStatus::Missing,
                    None,
                    vec!["permutation requires n and r".into()],
                );
            };
            if n > 20 || r > n {
                return finish_with(
                    CountingStatus::InvalidRange,
                    None,
                    vec!["permutation requires 0 <= r <= n <= 20".into()],
                );
            }
            (0..r).try_fold(1_u128, |acc, i| acc.checked_mul((n - i) as u128))
        }
        CountingOperation::Combination => {
            let (Some(n), Some(r)) = (request.n, request.r) else {
                return finish_with(
                    CountingStatus::Missing,
                    None,
                    vec!["combination requires n and r".into()],
                );
            };
            if n > 20 || r > n {
                return finish_with(
                    CountingStatus::InvalidRange,
                    None,
                    vec!["combination requires 0 <= r <= n <= 20".into()],
                );
            }
            factorial(n).and_then(|num| {
                factorial(r).and_then(|a| {
                    factorial(n - r).and_then(|b| a.checked_mul(b).map(|den| num / den))
                })
            })
        }
    };
    match value {
        Some(value) => finish_with(
            CountingStatus::Complete,
            Some(CountingArtifact::ExactCount(value)),
            Vec::new(),
        ),
        None => finish_with(
            CountingStatus::Overflow,
            None,
            vec!["exact bounded count overflowed".into()],
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordered_and_unordered_counts_differ() {
        let base = |operation| CountingRequest {
            operation,
            n: Some(5),
            r: Some(2),
            factors: Vec::new(),
            ambiguity: None,
            provenance: vec!["test".into()],
        };
        assert_eq!(
            evaluate(&base(CountingOperation::Permutation)).artifact,
            Some(CountingArtifact::ExactCount(20))
        );
        assert_eq!(
            evaluate(&base(CountingOperation::Combination)).artifact,
            Some(CountingArtifact::ExactCount(10))
        );
    }
    #[test]
    fn ambiguity_replays() {
        let request = CountingRequest {
            operation: CountingOperation::Combination,
            n: Some(5),
            r: Some(2),
            factors: Vec::new(),
            ambiguity: Some("order is unspecified".into()),
            provenance: vec!["test".into()],
        };
        assert!(replay_verified(&evaluate(&request)));
    }

    #[test]
    fn extended_combination_handles_large_edge_values() {
        let request = CountingRequest {
            operation: CountingOperation::Combination,
            n: Some(1293),
            r: Some(1),
            factors: Vec::new(),
            ambiguity: None,
            provenance: vec!["test-extended".into()],
        };
        let result = evaluate_extended_combination(&request);
        assert_eq!(result.status, CountingStatus::Complete);
        assert_eq!(result.artifact, Some(CountingArtifact::ExactCount(1293)));
        assert!(replay_verified(&result));
    }

    #[test]
    fn extended_combination_rejects_overflow_without_authorizing() {
        let request = CountingRequest {
            operation: CountingOperation::Combination,
            n: Some(100_000),
            r: Some(50_000),
            factors: Vec::new(),
            ambiguity: None,
            provenance: vec!["test-overflow".into()],
        };
        let result = evaluate_extended_combination(&request);
        assert_eq!(result.status, CountingStatus::Overflow);
        assert!(result.artifact.is_none());
        assert!(replay_verified(&result));
    }
}
