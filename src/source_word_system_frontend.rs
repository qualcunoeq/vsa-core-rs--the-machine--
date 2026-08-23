//! Bounded natural-language frontend for two-number linear systems.
//!
//! The contract is deliberately structural: an explicit sum of two numbers
//! plus one explicit relation of the form `one number is k less/more than m
//! times the other`, or an explicit signed difference.  It does not infer
//! quantities from arbitrary stories, assume units, or interpret products as
//! differences.

use crate::linear_system::{
    execute_linear_system, replay_linear_system, LinearSystemExecutionReceipt,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WordSystemStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WordSystemRequest {
    pub equations: Vec<String>,
    pub variables: Vec<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WordSystemResult {
    pub status: WordSystemStatus,
    pub request: Option<WordSystemRequest>,
    pub evidence: Vec<String>,
    pub unresolved: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn finish(mut result: WordSystemResult) -> WordSystemResult {
    result.replay_hash.clear();
    result.replay_hash = digest(&result);
    result
}

pub fn replay_verified(result: &WordSystemResult) -> bool {
    let mut copy = result.clone();
    let hash = copy.replay_hash.clone();
    copy.replay_hash.clear();
    hash == digest(&copy) && !result.provenance.is_empty()
}

fn unit_value(word: &str) -> Option<i128> {
    Some(match word {
        "zero" => 0,
        "one" => 1,
        "two" => 2,
        "three" => 3,
        "four" => 4,
        "five" => 5,
        "six" => 6,
        "seven" => 7,
        "eight" => 8,
        "nine" => 9,
        "ten" => 10,
        "eleven" => 11,
        "twelve" => 12,
        "thirteen" => 13,
        "fourteen" => 14,
        "fifteen" => 15,
        "sixteen" => 16,
        "seventeen" => 17,
        "eighteen" => 18,
        "nineteen" => 19,
        _ => return None,
    })
}

fn tens_value(word: &str) -> Option<i128> {
    Some(match word {
        "twenty" => 20,
        "thirty" => 30,
        "forty" => 40,
        "fifty" => 50,
        "sixty" => 60,
        "seventy" => 70,
        "eighty" => 80,
        "ninety" => 90,
        _ => return None,
    })
}

/// Parse one bounded integer at the beginning of a phrase and return the
/// unconsumed suffix.  Hundreds, decimals, and implicit units are rejected.
fn leading_integer(text: &str) -> Option<(i128, &str)> {
    let trimmed = text.trim_start();
    let mut end = 0;
    for (index, character) in trimmed.char_indices() {
        if character.is_whitespace() || matches!(character, '.' | ',' | ';' | '?') {
            break;
        }
        end = index + character.len_utf8();
    }
    if end == 0 {
        return None;
    }
    let first = &trimmed[..end];
    let first_normalized = first
        .replace('−', "-")
        .trim_matches(|character: char| !character.is_ascii_alphanumeric() && character != '-')
        .to_owned();
    if let Ok(value) = first_normalized.parse::<i128>() {
        return Some((value, trimmed[end..].trim_start()));
    }
    if let Some((tens, unit)) = first_normalized.split_once('-') {
        if let (Some(tens), Some(unit)) = (tens_value(tens), unit_value(unit)) {
            return Some((tens + unit, trimmed[end..].trim_start()));
        }
    }
    if first_normalized == "negative" {
        let (value, rest) = leading_integer(trimmed[end..].trim_start())?;
        return Some((-value, rest));
    }
    if let Some(value) = unit_value(&first_normalized) {
        return Some((value, trimmed[end..].trim_start()));
    }
    if let Some(value) = tens_value(&first_normalized) {
        let rest = trimmed[end..].trim_start();
        let mut rest_end = 0;
        for (index, character) in rest.char_indices() {
            if character.is_whitespace() || matches!(character, '.' | ',' | ';' | '?') {
                break;
            }
            rest_end = index + character.len_utf8();
        }
        if rest_end > 0 {
            if let Some(unit) = unit_value(&rest[..rest_end].to_ascii_lowercase()) {
                return Some((value + unit, rest[rest_end..].trim_start()));
            }
        }
        return Some((value, rest));
    }
    None
}

fn relation_multiplier(text: &str) -> Option<(i128, i128)> {
    let lower = text.to_ascii_lowercase();
    let multiplier = if lower.contains("twice the other") {
        2
    } else if lower.contains("three times the other") {
        3
    } else if lower.contains("four times the other") {
        4
    } else if lower.contains("five times the other") {
        5
    } else if lower.contains("times the other") {
        return None;
    } else if lower.contains("the other") {
        1
    } else {
        return None;
    };
    let sign = if lower.contains("less than") {
        -1
    } else if lower.contains("more than") {
        1
    } else {
        return None;
    };
    Some((multiplier, sign))
}

pub fn formalize_two_number_system(text: &str, case_id: &str) -> WordSystemResult {
    let lower = text.to_ascii_lowercase();
    let provenance = vec![format!("source-word-system-frontend:{case_id}")];
    if !lower.contains("two number") {
        return finish(WordSystemResult {
            status: WordSystemStatus::Unsupported,
            request: None,
            evidence: Vec::new(),
            unresolved: vec!["exactly two named numbers are required".into()],
            provenance,
            replay_hash: String::new(),
        });
    }
    let sum_marker = lower
        .find("sum of two number")
        .and_then(|index| lower[index..].find("is ").map(|offset| index + offset + 3));
    let Some(sum_start) = sum_marker else {
        return finish(WordSystemResult {
            status: WordSystemStatus::Missing,
            request: None,
            evidence: Vec::new(),
            unresolved: vec!["an explicit sum of the two numbers is required".into()],
            provenance,
            replay_hash: String::new(),
        });
    };
    let Some((sum, sum_rest)) = leading_integer(&lower[sum_start..]) else {
        return finish(WordSystemResult {
            status: WordSystemStatus::Ambiguous,
            request: None,
            evidence: vec![lower[sum_start..].split('.').next().unwrap_or("").into()],
            unresolved: vec!["the sum must be an explicit bounded integer".into()],
            provenance,
            replay_hash: String::new(),
        });
    };
    let relation_start = lower.find("one number is ");
    let Some(relation_start) = relation_start else {
        return finish(WordSystemResult {
            status: WordSystemStatus::Missing,
            request: None,
            evidence: vec![format!("sum={sum}")],
            unresolved: vec!["one explicit relation between the two numbers is required".into()],
            provenance,
            replay_hash: String::new(),
        });
    };
    let relation_text = &lower[relation_start + "one number is ".len()..];
    let Some((offset, after_offset)) = leading_integer(relation_text) else {
        return finish(WordSystemResult {
            status: WordSystemStatus::Ambiguous,
            request: None,
            evidence: vec![format!("sum={sum}")],
            unresolved: vec!["the relation offset must be an explicit bounded integer".into()],
            provenance,
            replay_hash: String::new(),
        });
    };
    let Some((multiplier, sign)) = relation_multiplier(after_offset) else {
        return finish(WordSystemResult {
            status: WordSystemStatus::Unsupported,
            request: None,
            evidence: vec![format!("sum={sum}"), format!("offset={offset}")],
            unresolved: vec!["only explicit less/more-than-other relations are supported".into()],
            provenance,
            replay_hash: String::new(),
        });
    };
    let delta = offset * sign;
    let request = WordSystemRequest {
        equations: vec![format!("x+y={sum}"), format!("x-{multiplier}*y={delta}")],
        variables: vec!["x".into(), "y".into()],
        provenance: provenance.clone(),
    };
    let _ = sum_rest;
    finish(WordSystemResult {
        status: WordSystemStatus::Complete,
        request: Some(request),
        evidence: vec![
            format!("sum={sum}"),
            format!("offset={offset}"),
            format!("multiplier={multiplier}"),
            format!("sign={sign}"),
        ],
        unresolved: Vec::new(),
        provenance,
        replay_hash: String::new(),
    })
}

pub fn execute_word_system(result: &WordSystemResult) -> Option<LinearSystemExecutionReceipt> {
    if result.status != WordSystemStatus::Complete || !replay_verified(result) {
        return None;
    }
    let request = result.request.as_ref()?;
    let source = format!("Solve system: {} for x,y", request.equations.join("; "));
    execute_linear_system(&source).ok()
}

pub fn execution_replay_verified(receipt: &LinearSystemExecutionReceipt) -> bool {
    replay_linear_system(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_numeric_sum_and_less_relation() {
        let result = formalize_two_number_system(
            "The sum of two numbers is 20. One number is 4 less than the other. Find the numbers.",
            "test-1",
        );
        assert_eq!(result.status, WordSystemStatus::Complete);
        let receipt = execute_word_system(&result).unwrap();
        assert!(execution_replay_verified(&receipt));
    }

    #[test]
    fn parses_written_sum_and_multiple_relation() {
        let result = formalize_two_number_system(
            "The sum of two numbers is twenty-seven. One number is seven less than twice the other.",
            "test-2",
        );
        assert_eq!(result.status, WordSystemStatus::Complete);
        assert!(execute_word_system(&result).is_some());
    }

    #[test]
    fn preserves_unicode_negative_sum() {
        let result = formalize_two_number_system(
            "The sum of two numbers is −16. One number is 20 less than the other.",
            "test-unicode-negative",
        );
        let receipt = execute_word_system(&result).expect("unique negative system");
        assert_eq!(receipt.result, r#"{"x": "-18", "y": "2"}"#);
        assert!(execution_replay_verified(&receipt));
    }

    #[test]
    fn rejects_missing_sum() {
        let result = formalize_two_number_system(
            "One number is 4 less than the other. Find the numbers.",
            "test-3",
        );
        assert_ne!(result.status, WordSystemStatus::Complete);
    }

    #[test]
    fn rejects_product_as_relation() {
        let result = formalize_two_number_system(
            "The sum of two numbers is 11 and the product is 24. Find the numbers.",
            "test-4",
        );
        assert_ne!(result.status, WordSystemStatus::Complete);
    }

    #[test]
    fn tampered_frontend_is_rejected() {
        let mut result = formalize_two_number_system(
            "The sum of two numbers is 20. One number is 4 less than the other.",
            "test-5",
        );
        assert!(replay_verified(&result));
        result.replay_hash.push('x');
        assert!(!replay_verified(&result));
    }
}
