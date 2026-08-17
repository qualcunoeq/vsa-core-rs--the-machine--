//! Controlled technical-language frontend for bounded finite characters.
//!
//! The frontend accepts only explicit finite-character requests.  It does not
//! infer a modulus from a nearby prime, turn analytic vocabulary into a
//! finite computation, or authorize a character merely because the word
//! "Dirichlet" appears in a report.

use crate::dirichlet_character_pack::{CharacterOperation, DirichletCharacterRequest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CharacterFrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CharacterFrontendResult {
    pub status: CharacterFrontendStatus,
    pub request: Option<DirichletCharacterRequest>,
    pub unresolved: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn finish(mut result: CharacterFrontendResult) -> CharacterFrontendResult {
    result.replay_hash.clear();
    result.replay_hash = digest(&result);
    result
}

pub fn replay_verified(result: &CharacterFrontendResult) -> bool {
    let mut copy = result.clone();
    let hash = copy.replay_hash.clone();
    copy.replay_hash.clear();
    hash == digest(&copy) && !result.provenance.is_empty()
}

fn integer_after(text: &str, labels: &[&str]) -> Option<i64> {
    labels.iter().find_map(|label| {
        let start = text.find(label)? + label.len();
        let token = text[start..]
            .trim_start_matches(|c: char| c == '=' || c == ':' || c.is_whitespace())
            .strip_prefix('−')
            .map(|rest| format!("-{rest}"))
            .unwrap_or_else(|| text[start..].trim_start().to_string());
        let token = token
            .trim_start_matches(|c: char| c == '=' || c == ':' || c.is_whitespace())
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '-')
            .collect::<String>();
        (!token.is_empty()).then(|| token.parse().ok()).flatten()
    })
}

fn output(
    status: CharacterFrontendStatus,
    request: Option<DirichletCharacterRequest>,
    unresolved: Vec<String>,
    provenance: Vec<String>,
) -> CharacterFrontendResult {
    finish(CharacterFrontendResult {
        status,
        request,
        unresolved,
        provenance,
        replay_hash: String::new(),
    })
}

/// Parse a bounded finite-character request from controlled technical text.
pub fn formalize(text: &str, case_id: &str) -> CharacterFrontendResult {
    let lower = text.to_ascii_lowercase();
    let provenance = vec![
        format!("dirichlet-character-frontend:{case_id}"),
        format!("source-span:0..{}", text.len()),
        "explicit-bounded-finite-character-grammar".into(),
    ];
    if [
        "analytic",
        "asymptotic",
        "dirichlet series",
        "analytic continuation",
        "l-function",
        "infinite sum",
        "approximate complex",
        "continuous",
    ]
    .iter()
    .any(|term| lower.contains(term))
    {
        return output(
            CharacterFrontendStatus::Unsupported,
            None,
            vec!["analytic or continuous semantics exceed the bounded character contract".into()],
            provenance,
        );
    }
    if lower.contains(" or ")
        || lower.contains("either")
        || (lower.contains("character") && lower.contains("which operation"))
    {
        return output(
            CharacterFrontendStatus::Ambiguous,
            None,
            vec!["more than one character operation is plausible".into()],
            provenance,
        );
    }
    let operation = if lower.contains("orthogonality") || lower.contains("orthogonal") {
        CharacterOperation::Orthogonality
    } else if lower.contains("partial sum") || lower.contains("finite sum") {
        CharacterOperation::PartialSum
    } else if lower.contains("validate") || lower.contains("check the character") {
        CharacterOperation::ValidateCharacter
    } else if lower.contains("evaluate")
        || lower.contains("character value")
        || lower.contains("value of the character")
    {
        CharacterOperation::Evaluate
    } else {
        return output(
            CharacterFrontendStatus::Missing,
            None,
            vec!["a unique finite-character operation is not stated".into()],
            provenance,
        );
    };
    let modulus = integer_after(
        &lower,
        &["modulus=", "modulus ", "prime=", "prime ", "p=", "p ="],
    )
    .and_then(|value| u32::try_from(value).ok());
    let exponent = integer_after(
        &lower,
        &["exponent=", "exponent ", "character exponent=", "k=", "k ="],
    )
    .and_then(|value| u32::try_from(value).ok());
    let value = integer_after(&lower, &["value=", "value ", "x=", "x =", "input="]);
    let sum_limit = integer_after(&lower, &["limit=", "limit ", "through ", "up to "])
        .and_then(|value| u32::try_from(value).ok());
    if lower.contains("composite modulus") || lower.contains("modulus is composite") {
        return output(
            CharacterFrontendStatus::Unsupported,
            None,
            vec!["the bounded character pack requires a prime modulus".into()],
            provenance,
        );
    }
    if modulus.is_none() || exponent.is_none() {
        return output(
            CharacterFrontendStatus::Missing,
            None,
            vec!["prime modulus and character exponent must be explicit".into()],
            provenance,
        );
    }
    let missing_operation_field = match operation {
        CharacterOperation::Evaluate => value.is_none(),
        CharacterOperation::PartialSum => sum_limit.is_none(),
        CharacterOperation::ValidateCharacter | CharacterOperation::Orthogonality => false,
    };
    if missing_operation_field {
        return output(
            CharacterFrontendStatus::Missing,
            None,
            vec!["the selected operation's required input is not explicit".into()],
            provenance,
        );
    }
    let request = DirichletCharacterRequest {
        operation,
        modulus,
        exponent,
        value,
        sum_limit,
        domain: "bounded_dirichlet_character".into(),
        ambiguity: None,
        provenance: provenance.clone(),
    };
    output(
        CharacterFrontendStatus::Complete,
        Some(request),
        Vec::new(),
        provenance,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_character_operations_bind() {
        let value = formalize(
            "Evaluate the character value at x=2 modulo p=5 with exponent k=1.",
            "value",
        );
        assert_eq!(value.status, CharacterFrontendStatus::Complete);
        assert_eq!(
            value.request.as_ref().unwrap().operation,
            CharacterOperation::Evaluate
        );
        assert!(replay_verified(&value));
        let sum = formalize(
            "Compute the partial sum through limit=8 modulo p=5 with exponent k=1.",
            "sum",
        );
        assert_eq!(sum.status, CharacterFrontendStatus::Complete);
    }

    #[test]
    fn ambiguity_and_unsupported_remain_fail_closed() {
        let ambiguous = formalize(
            "Evaluate or compute the partial sum of a character modulo p=5 with exponent k=1.",
            "ambiguous",
        );
        assert_eq!(ambiguous.status, CharacterFrontendStatus::Ambiguous);
        let unsupported = formalize(
            "Estimate the asymptotic Dirichlet series associated with modulus p=5 and exponent k=1.",
            "unsupported",
        );
        assert_eq!(unsupported.status, CharacterFrontendStatus::Unsupported);
    }
}
