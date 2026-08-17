//! Bounded frontend for explicit counting requests.

use crate::source_counting_pack::{CountingOperation, CountingRequest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CountingFrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CountingFrontendResult {
    pub status: CountingFrontendStatus,
    pub request: Option<CountingRequest>,
    pub unresolved: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}
fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}
fn finish(mut result: CountingFrontendResult) -> CountingFrontendResult {
    result.replay_hash.clear();
    result.replay_hash = digest(&result);
    result
}
pub fn replay_verified(result: &CountingFrontendResult) -> bool {
    let mut copy = result.clone();
    let hash = copy.replay_hash.clone();
    copy.replay_hash.clear();
    hash == digest(&copy) && !result.provenance.is_empty()
}
fn binding(text: &str, label: &str) -> Option<u64> {
    let lower = text.to_ascii_lowercase();
    let start = lower.find(label)?;
    let token = lower[start + label.len()..]
        .trim_start_matches(|c: char| c == '=' || c == ':' || c.is_whitespace())
        .split(|c: char| !c.is_ascii_digit())
        .next()?;
    token.parse().ok()
}
pub fn formalize_counting_text(text: &str, case_id: &str) -> CountingFrontendResult {
    let lower = text.to_ascii_lowercase();
    let provenance = vec![
        format!("source-counting-frontend:{case_id}"),
        "explicit-bounded-count-parser".into(),
    ];
    if [
        "infinite",
        "asymptotic",
        "approx",
        "probability density",
        "unbounded",
        "diagram",
    ]
    .iter()
    .any(|term| lower.contains(term))
    {
        return finish(CountingFrontendResult {
            status: CountingFrontendStatus::Unsupported,
            request: None,
            unresolved: vec![
                "unbounded, approximate, or non-finite counting semantics are outside the pack"
                    .into(),
            ],
            provenance,
            replay_hash: String::new(),
        });
    }
    if lower.contains(" or ")
        || lower.contains("either")
        || (lower.contains("permutation")
            && lower.contains("combination")
            && !lower.contains("order matters"))
    {
        return finish(CountingFrontendResult {
            status: CountingFrontendStatus::Ambiguous,
            request: None,
            unresolved: vec!["ordered versus unordered selection is not uniquely stated".into()],
            provenance,
            replay_hash: String::new(),
        });
    }
    let operation = if lower.contains("permutation")
        || (lower.contains("ordered") && !lower.contains("unordered"))
    {
        CountingOperation::Permutation
    } else if lower.contains("combination") || lower.contains("unordered") {
        CountingOperation::Combination
    } else if lower.contains("factorial") {
        CountingOperation::Factorial
    } else if lower.contains("multiply")
        || lower.contains("product")
        || lower.contains("multiplication rule")
    {
        CountingOperation::Product
    } else {
        return finish(CountingFrontendResult {
            status: CountingFrontendStatus::Unsupported,
            request: None,
            unresolved: vec!["no explicit bounded counting operation".into()],
            provenance,
            replay_hash: String::new(),
        });
    };
    let n = binding(&lower, "n=")
        .or_else(|| binding(&lower, "n ="))
        .or_else(|| binding(&lower, "total="));
    let r = binding(&lower, "r=")
        .or_else(|| binding(&lower, "r ="))
        .or_else(|| binding(&lower, "choose="));
    if matches!(
        operation,
        CountingOperation::Permutation | CountingOperation::Combination
    ) && (n.is_none() || r.is_none())
    {
        return finish(CountingFrontendResult {
            status: CountingFrontendStatus::Missing,
            request: None,
            unresolved: vec!["ordered or unordered selection requires explicit n and r".into()],
            provenance,
            replay_hash: String::new(),
        });
    }
    if operation == CountingOperation::Factorial && n.is_none() {
        return finish(CountingFrontendResult {
            status: CountingFrontendStatus::Missing,
            request: None,
            unresolved: vec!["factorial requires explicit n".into()],
            provenance,
            replay_hash: String::new(),
        });
    }
    let factors = if operation == CountingOperation::Product {
        match (n, r) {
            (Some(left), Some(right)) => vec![left, right],
            _ => Vec::new(),
        }
    } else {
        Vec::new()
    };
    let request = CountingRequest {
        operation,
        n,
        r,
        factors,
        ambiguity: None,
        provenance: provenance.clone(),
    };
    finish(CountingFrontendResult {
        status: CountingFrontendStatus::Complete,
        request: Some(request),
        unresolved: Vec::new(),
        provenance,
        replay_hash: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distinguishes_order() {
        let result = formalize_counting_text("There are n=5 and r=2 ordered permutations.", "t");
        assert_eq!(result.status, CountingFrontendStatus::Complete);
        assert!(replay_verified(&result));
    }
    #[test]
    fn preserves_order_ambiguity() {
        let result = formalize_counting_text(
            "Choose n=5 and r=2, either a permutation or combination.",
            "t",
        );
        assert_eq!(result.status, CountingFrontendStatus::Ambiguous);
    }
}
