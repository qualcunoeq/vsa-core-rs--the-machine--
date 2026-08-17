//! Bounded language-to-request frontend for source-derived sequences.
//!
//! This frontend only recognizes explicitly stated finite arithmetic and
//! geometric sequence inputs.  It emits a typed formula request; it never
//! evaluates a formula or authorizes an answer.

use crate::probability_pack::Rational;
use crate::source_formula_pack::FormulaRequest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SequenceFrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SequenceFrontendResult {
    pub status: SequenceFrontendStatus,
    pub request: Option<FormulaRequest>,
    pub evidence: Vec<String>,
    pub unresolved: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn rational(value: &str) -> Option<Rational> {
    value
        .trim()
        .parse::<i128>()
        .ok()
        .map(|n| Rational::new(n, 1).unwrap())
}

fn find_value(text: &str, labels: &[&str]) -> Option<(String, Rational)> {
    for label in labels {
        let Some(start) = text.find(label) else {
            continue;
        };
        let tail = &text[start + label.len()..];
        let token = tail
            .trim_start_matches(|c: char| c == ' ' || c == ':' || c == '=')
            .split(|c: char| !c.is_ascii_digit() && c != '-')
            .next()
            .unwrap_or_default();
        if let Some(value) = rational(token) {
            return Some(((*label).into(), value));
        }
    }
    None
}

fn output(result: SequenceFrontendResult) -> SequenceFrontendResult {
    let mut result = result;
    result.replay_hash.clear();
    result.replay_hash = digest(&result);
    result
}

pub fn replay_verified(result: &SequenceFrontendResult) -> bool {
    let mut copy = result.clone();
    let hash = copy.replay_hash.clone();
    copy.replay_hash.clear();
    hash == digest(&copy) && !result.provenance.is_empty()
}

pub fn formalize_sequence_text(text: &str, case_id: &str) -> SequenceFrontendResult {
    let lower = text.to_ascii_lowercase();
    let provenance = vec![format!("source-sequence-frontend:{case_id}")];
    let mut unresolved = Vec::new();
    if [
        "infinite",
        "converges",
        "convergence",
        "limit",
        "recurrence",
    ]
    .iter()
    .any(|term| lower.contains(term))
    {
        return output(SequenceFrontendResult {
            status: SequenceFrontendStatus::Unsupported,
            request: None,
            evidence: Vec::new(),
            unresolved: vec![
                "infinite or recurrence semantics are outside the finite catalog".into(),
            ],
            provenance,
            replay_hash: String::new(),
        });
    }
    let arithmetic = lower.contains("arithmetic") || lower.contains("common difference");
    let geometric = lower.contains("geometric") || lower.contains("common ratio");
    if !arithmetic && !geometric {
        return output(SequenceFrontendResult {
            status: SequenceFrontendStatus::Unsupported,
            request: None,
            evidence: Vec::new(),
            unresolved: vec!["no supported finite sequence family is stated".into()],
            provenance,
            replay_hash: String::new(),
        });
    }
    if arithmetic && geometric {
        unresolved.push("sequence family is not uniquely identified".into());
    }
    let partial =
        lower.contains("sum of") || lower.contains("partial sum") || lower.contains("series sum");
    let nth =
        lower.contains("nth term") || lower.contains("n-th term") || lower.contains("term number");
    if partial == nth {
        unresolved.push("requested finite operation is not uniquely identified".into());
    }
    let Some((a1_label, a1)) = find_value(&lower, &["first term", "first value", "a1", "a_1"])
    else {
        unresolved.push("first term is not explicitly bound".into());
        return output(SequenceFrontendResult {
            status: if unresolved.len() > 1 {
                SequenceFrontendStatus::Ambiguous
            } else {
                SequenceFrontendStatus::Missing
            },
            request: None,
            evidence: Vec::new(),
            unresolved,
            provenance,
            replay_hash: String::new(),
        });
    };
    let Some((n_label, n)) = find_value(&lower, &["n =", "n is", "term number", "term n"]) else {
        unresolved.push("positive term index is not explicitly bound".into());
        return output(SequenceFrontendResult {
            status: SequenceFrontendStatus::Missing,
            request: None,
            evidence: vec![a1_label],
            unresolved,
            provenance,
            replay_hash: String::new(),
        });
    };
    let mut inputs = BTreeMap::from([("a1".into(), a1), ("n".into(), n)]);
    let (formula, parameter_label) = if arithmetic {
        let Some((label, value)) = find_value(&lower, &["common difference", "difference", "d ="])
        else {
            unresolved.push("common difference is not explicitly bound".into());
            return output(SequenceFrontendResult {
                status: SequenceFrontendStatus::Missing,
                request: None,
                evidence: vec![a1_label, n_label],
                unresolved,
                provenance,
                replay_hash: String::new(),
            });
        };
        inputs.insert("d".into(), value);
        (
            if partial {
                "arithmetic_partial_sum"
            } else {
                "arithmetic_nth_term"
            },
            label,
        )
    } else if geometric {
        let Some((label, value)) = find_value(&lower, &["common ratio", "ratio", "r ="]) else {
            unresolved.push("common ratio is not explicitly bound".into());
            return output(SequenceFrontendResult {
                status: SequenceFrontendStatus::Missing,
                request: None,
                evidence: vec![a1_label, n_label],
                unresolved,
                provenance,
                replay_hash: String::new(),
            });
        };
        inputs.insert("r".into(), value);
        (
            if partial {
                "geometric_partial_sum"
            } else {
                "geometric_nth_term"
            },
            label,
        )
    } else {
        ("", String::new())
    };
    if !unresolved.is_empty() {
        return output(SequenceFrontendResult {
            status: SequenceFrontendStatus::Ambiguous,
            request: None,
            evidence: vec![a1_label, n_label, parameter_label],
            unresolved,
            provenance,
            replay_hash: String::new(),
        });
    }
    output(SequenceFrontendResult {
        status: SequenceFrontendStatus::Complete,
        request: Some(FormulaRequest {
            formula: formula.into(),
            inputs,
            domain: "source_catalog_sequences_series".into(),
            ambiguity: None,
            provenance: provenance.clone(),
        }),
        evidence: vec![a1_label, n_label, parameter_label],
        unresolved,
        provenance,
        replay_hash: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binds_arithmetic_nth_term_without_evaluating() {
        let result = formalize_sequence_text(
            "An arithmetic sequence has first term = 3, common difference = 4; find the nth term for n = 5.",
            "test-arithmetic",
        );
        assert_eq!(result.status, SequenceFrontendStatus::Complete);
        assert_eq!(
            result.request.as_ref().unwrap().formula,
            "arithmetic_nth_term"
        );
        assert!(replay_verified(&result));
    }

    #[test]
    fn refuses_infinite_language() {
        let result = formalize_sequence_text(
            "Determine whether the infinite geometric series converges.",
            "test-infinite",
        );
        assert_eq!(result.status, SequenceFrontendStatus::Unsupported);
        assert!(replay_verified(&result));
    }
}
