//! Bounded frontend for explicit natural-language combination requests.
//!
//! This is a narrow bridge to the externally sourced combination evaluator.
//! It accepts only the structural grammar `choose R out of N`; it does not
//! infer unordered selection from a role, probability, restriction, or
//! merely related counting vocabulary.

use crate::source_counting_pack::{CountingOperation, CountingRequest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CombinationFrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CombinationFrontendResult {
    pub status: CombinationFrontendStatus,
    pub request: Option<CountingRequest>,
    pub unresolved: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn finish(mut result: CombinationFrontendResult) -> CombinationFrontendResult {
    result.replay_hash.clear();
    result.replay_hash = digest(&result);
    result
}

pub fn replay_verified(result: &CombinationFrontendResult) -> bool {
    let mut copy = result.clone();
    let hash = copy.replay_hash.clone();
    copy.replay_hash.clear();
    hash == digest(&copy) && !result.provenance.is_empty()
}

fn bounded_count(token: &str) -> Option<u64> {
    let normalized = token
        .trim_matches(|character: char| !character.is_ascii_alphanumeric())
        .to_ascii_lowercase();
    if let Ok(value) = normalized.parse::<u64>() {
        return Some(value);
    }
    [
        ("one", 1),
        ("two", 2),
        ("three", 3),
        ("four", 4),
        ("five", 5),
        ("six", 6),
        ("seven", 7),
        ("eight", 8),
        ("nine", 9),
        ("ten", 10),
        ("eleven", 11),
        ("twelve", 12),
        ("thirteen", 13),
        ("fourteen", 14),
        ("fifteen", 15),
        ("sixteen", 16),
        ("seventeen", 17),
        ("eighteen", 18),
        ("nineteen", 19),
        ("twenty", 20),
        ("first", 1),
        ("second", 2),
        ("third", 3),
        ("fourth", 4),
        ("fifth", 5),
        ("sixth", 6),
        ("seventh", 7),
        ("eighth", 8),
        ("ninth", 9),
        ("tenth", 10),
        ("eleventh", 11),
        ("twelfth", 12),
        ("thirteenth", 13),
        ("fourteenth", 14),
        ("fifteenth", 15),
        ("sixteenth", 16),
        ("seventeenth", 17),
        ("eighteenth", 18),
        ("nineteenth", 19),
        ("twentieth", 20),
    ]
    .into_iter()
    .find_map(|(word, value)| (normalized == word).then_some(value))
}

fn tokens(text: &str) -> Vec<String> {
    text.to_ascii_lowercase()
        .split_whitespace()
        .map(|token| {
            token
                .trim_matches(|character: char| {
                    !character.is_ascii_alphanumeric() && character != '-'
                })
                .to_owned()
        })
        .filter(|token| !token.is_empty())
        .collect()
}

/// Lower only an explicit finite `choose R out of N` request.
pub fn formalize_combination_text(text: &str, case_id: &str) -> CombinationFrontendResult {
    let lower = text.to_ascii_lowercase();
    let provenance = vec![
        format!("source-combination-frontend:{case_id}"),
        "explicit-choose-out-of-grammar".into(),
    ];
    if [
        "random",
        "probability",
        "permutation",
        "arrange",
        "order",
        "different suits",
        "triplet",
        "at least",
        "exactly",
        "without",
        "restriction",
        "condition",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        return finish(CombinationFrontendResult {
            status: CombinationFrontendStatus::Unsupported,
            request: None,
            unresolved: vec![
                "selection has probability, ordering, or additional constraints outside the simple combination grammar".into(),
            ],
            provenance,
            replay_hash: String::new(),
        });
    }
    if !lower.contains("way") || !lower.contains("choose") {
        return finish(CombinationFrontendResult {
            status: CombinationFrontendStatus::Missing,
            request: None,
            unresolved: vec!["a bounded choose request is not stated".into()],
            provenance,
            replay_hash: String::new(),
        });
    }
    let words = tokens(text);
    let mut candidates = Vec::new();
    for index in 0..words.len().saturating_sub(4) {
        if words[index] != "choose" {
            continue;
        }
        let r_index = if words.get(index + 1).map(String::as_str) == Some("the") {
            index + 2
        } else {
            index + 1
        };
        if words.get(r_index + 1).map(String::as_str) != Some("out")
            || words.get(r_index + 2).map(String::as_str) != Some("of")
        {
            continue;
        }
        let Some(r) = words.get(r_index).and_then(|word| bounded_count(word)) else {
            continue;
        };
        let Some(n) = words.get(r_index + 3).and_then(|word| bounded_count(word)) else {
            continue;
        };
        candidates.push((r, n, index));
    }
    if candidates.len() != 1 {
        return finish(CombinationFrontendResult {
            status: CombinationFrontendStatus::Ambiguous,
            request: None,
            unresolved: vec![
                "the selection does not contain one unique `choose R out of N` binding".into(),
            ],
            provenance,
            replay_hash: String::new(),
        });
    }
    let (r, n, index) = candidates[0];
    if n > 20 || r > n {
        return finish(CombinationFrontendResult {
            status: CombinationFrontendStatus::Unsupported,
            request: None,
            unresolved: vec!["the exact combination is outside the bounded n <= 20 range".into()],
            provenance,
            replay_hash: String::new(),
        });
    }
    let request = CountingRequest {
        operation: CountingOperation::Combination,
        n: Some(n),
        r: Some(r),
        factors: Vec::new(),
        ambiguity: None,
        provenance: vec![
            format!("source-combination-frontend:{case_id}"),
            format!("choose-span-token:{index}..{}", index + 5),
            "source:openstax-contemporary-mathematics:counting-principles".into(),
        ],
    };
    finish(CombinationFrontendResult {
        status: CombinationFrontendStatus::Complete,
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
    fn binds_only_explicit_choose_out_of() {
        let result = formalize_combination_text(
            "In how many ways can a student choose three out of eight classes?",
            "test",
        );
        assert_eq!(result.status, CombinationFrontendStatus::Complete);
        assert_eq!(result.request.as_ref().unwrap().n, Some(8));
        assert_eq!(result.request.as_ref().unwrap().r, Some(3));
        assert!(replay_verified(&result));
    }

    #[test]
    fn refuses_related_but_constrained_selection() {
        for text in [
            "How many ways can you choose 3 cards out of 52 if all have different suits?",
            "What is the probability of choosing 2 out of 7?",
            "How many ways can you choose 3 out of 8 when order matters?",
        ] {
            let result = formalize_combination_text(text, "test");
            assert_ne!(result.status, CombinationFrontendStatus::Complete);
            assert!(replay_verified(&result));
        }
    }
}
