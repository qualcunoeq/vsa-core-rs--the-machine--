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

fn numeric_values(text: &str) -> Vec<u64> {
    let mut values = Vec::new();
    let mut current = String::new();
    for character in text.chars() {
        if character.is_ascii_digit() {
            current.push(character);
        } else if !current.is_empty() {
            if let Ok(value) = current.parse() {
                values.push(value);
            }
            current.clear();
        }
    }
    if !current.is_empty() {
        if let Ok(value) = current.parse() {
            values.push(value);
        }
    }
    values
}

fn phrase_selection_values(text: &str) -> Option<(u64, u64)> {
    let has_selection_verb = text.contains("choose") || text.contains("select");
    let has_range_phrase = text.contains(" from ") || text.contains(" out of ");
    if !has_selection_verb || !has_range_phrase {
        return None;
    }
    let values = numeric_values(text);
    (values.len() == 2).then(|| (values[0], values[1]))
}

/// Parse only explicit numeric LaTeX binomial notation.  Symbolic entries,
/// malformed braces, and alternate domain uses remain outside this frontend.
fn latex_binomial_values(text: &str) -> Option<(u64, u64)> {
    let lower = text.to_ascii_lowercase();
    let marker = ["\\binom{", "\\dbinom{"]
        .iter()
        .find(|marker| lower.contains(**marker))?;
    let start = lower.find(marker)? + marker.len();
    let remainder = &text[start..];
    let close_n = remainder.find('}')?;
    let n_text = remainder[..close_n].trim();
    let after_n = remainder[close_n + 1..].trim_start();
    let after_open = after_n.strip_prefix('{')?;
    let close_r = after_open.find('}')?;
    let r_text = after_open[..close_r].trim();
    if n_text.is_empty()
        || r_text.is_empty()
        || !n_text.chars().all(|character| character.is_ascii_digit())
        || !r_text.chars().all(|character| character.is_ascii_digit())
    {
        return None;
    }
    Some((n_text.parse().ok()?, r_text.parse().ok()?))
}

pub fn formalize_counting_text(text: &str, case_id: &str) -> CountingFrontendResult {
    let lower = text.to_ascii_lowercase();
    let latex_binomial = latex_binomial_values(text);
    let has_latex_binomial = lower.contains("\\binom") || lower.contains("\\dbinom");
    let explicit_unordered = lower.contains("unordered")
        || lower.contains("order does not matter")
        || (lower.contains("order") && lower.contains("does not matter"));
    let explicit_ordered = lower.contains("ordered") || lower.contains("order matters");
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
        || lower.contains("ambiguous")
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
    let operation = if has_latex_binomial {
        if explicit_ordered || lower.contains("permutation") {
            return finish(CountingFrontendResult {
                status: CountingFrontendStatus::Ambiguous,
                request: None,
                unresolved: vec![
                    "LaTeX binomial notation conflicts with an ordered counting request".into(),
                ],
                provenance,
                replay_hash: String::new(),
            });
        }
        CountingOperation::Combination
    } else if lower.contains("permutation") || (explicit_ordered && !explicit_unordered) {
        CountingOperation::Permutation
    } else if lower.contains("combination") || explicit_unordered {
        CountingOperation::Combination
    } else if lower.contains("order matters") {
        CountingOperation::Permutation
    } else if lower.contains("factorial") || lower.contains("n!") || lower.contains("n !") {
        CountingOperation::Factorial
    } else if lower.contains("multiply")
        || lower.contains("product")
        || lower.contains("multiplication rule")
    {
        CountingOperation::Product
    } else {
        if ["count", "ways", "select", "arrange", "choose"]
            .iter()
            .any(|marker| lower.contains(marker))
        {
            return finish(CountingFrontendResult {
                status: CountingFrontendStatus::Missing,
                request: None,
                unresolved: vec![
                    "a counting request is present, but its bounded operation is not explicit"
                        .into(),
                ],
                provenance,
                replay_hash: String::new(),
            });
        }
        return finish(CountingFrontendResult {
            status: CountingFrontendStatus::Unsupported,
            request: None,
            unresolved: vec!["no explicit bounded counting operation".into()],
            provenance,
            replay_hash: String::new(),
        });
    };
    let mut n = latex_binomial.map(|(n, _)| n).or_else(|| binding(&lower, "n="))
        .or_else(|| binding(&lower, "n ="))
        .or_else(|| binding(&lower, "total="));
    let mut r = latex_binomial.map(|(_, r)| r).or_else(|| binding(&lower, "r="))
        .or_else(|| binding(&lower, "r ="))
        .or_else(|| binding(&lower, "choose="));
    if matches!(
        operation,
        CountingOperation::Permutation | CountingOperation::Combination
    ) && (n.is_none() || r.is_none())
    {
        let order_is_explicit = explicit_unordered || explicit_ordered;
        if order_is_explicit {
            if let Some((requested, available)) = phrase_selection_values(&lower) {
                r.get_or_insert(requested);
                n.get_or_insert(available);
            }
        }
    }
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
            _ => {
                return finish(CountingFrontendResult {
                    status: CountingFrontendStatus::Missing,
                    request: None,
                    unresolved: vec![
                        "a multiplication count requires explicitly bound finite factors".into(),
                    ],
                    provenance,
                    replay_hash: String::new(),
                });
            }
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

    #[test]
    fn preserves_missing_operation_as_non_authorizing() {
        let result = formalize_counting_text("Count the ways, but no model is stated.", "t");
        assert_eq!(result.status, CountingFrontendStatus::Missing);
        assert!(replay_verified(&result));
    }

    #[test]
    fn binds_explicit_unordered_phrase_without_guessing() {
        let result = formalize_counting_text(
            "Choose 3 cards from a deck of 52; the order does not matter.",
            "t",
        );
        assert_eq!(result.status, CountingFrontendStatus::Complete);
        let request = result.request.as_ref().unwrap();
        assert_eq!(request.n, Some(52));
        assert_eq!(request.r, Some(3));
        assert_eq!(request.operation, CountingOperation::Combination);
    }

    #[test]
    fn refuses_phrase_selection_without_order_semantics() {
        let result = formalize_counting_text("Choose 3 cards from a deck of 52.", "t");
        assert_eq!(result.status, CountingFrontendStatus::Missing);
        assert!(replay_verified(&result));
    }

    #[test]
    fn binds_numeric_latex_binomial_as_combination() {
        let result = formalize_counting_text(r"Compute $\dbinom{8}{4}$.", "latex");
        assert_eq!(result.status, CountingFrontendStatus::Complete);
        let request = result.request.as_ref().unwrap();
        assert_eq!(request.operation, CountingOperation::Combination);
        assert_eq!(request.n, Some(8));
        assert_eq!(request.r, Some(4));
        assert!(replay_verified(&result));
    }

    #[test]
    fn preserves_symbolic_latex_binomial_as_missing() {
        let result = formalize_counting_text(r"What is $\binom{n}{k}$?", "symbolic");
        assert_eq!(result.status, CountingFrontendStatus::Missing);
        assert!(replay_verified(&result));
    }

    #[test]
    fn rejects_latex_binomial_when_ordered_semantics_conflict() {
        let result = formalize_counting_text(
            r"Interpret $\binom{5}{2}$ as an ordered permutation.",
            "conflict",
        );
        assert_eq!(result.status, CountingFrontendStatus::Ambiguous);
        assert!(replay_verified(&result));
    }
}
