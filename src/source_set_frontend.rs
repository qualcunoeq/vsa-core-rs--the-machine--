//! Bounded technical-language frontend for explicit finite-set operations.

use crate::source_set_pack::{SetOperation, SetRequest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SetFrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SetFrontendResult {
    pub status: SetFrontendStatus,
    pub request: Option<SetRequest>,
    pub bindings: BTreeMap<String, BTreeSet<String>>,
    pub unresolved: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}
fn finish(mut value: SetFrontendResult) -> SetFrontendResult {
    value.replay_hash.clear();
    value.replay_hash = digest(&value);
    value
}
pub fn replay_verified(value: &SetFrontendResult) -> bool {
    let mut copy = value.clone();
    let hash = copy.replay_hash.clone();
    copy.replay_hash.clear();
    hash == digest(&copy) && !value.provenance.is_empty()
}

fn parse_set(text: &str, label: &str) -> Option<BTreeSet<String>> {
    let lower = text.to_ascii_lowercase();
    let wanted = label.to_ascii_lowercase();
    let start = lower.match_indices(&wanted).find_map(|(index, _)| {
        let before_ok = index == 0 || !lower.as_bytes()[index - 1].is_ascii_alphanumeric();
        let after = index + wanted.len();
        let after_ok = after >= lower.len() || !lower.as_bytes()[after].is_ascii_alphanumeric();
        if before_ok && after_ok {
            Some(index)
        } else {
            None
        }
    })?;
    let rest = &text[start + label.len()..];
    let open = rest.find('{')?;
    let close = rest[open + 1..].find('}')? + open + 1;
    let body = &rest[open + 1..close];
    if body.trim().is_empty() {
        return Some(BTreeSet::new());
    }
    let mut values = BTreeSet::new();
    for token in body.split(',') {
        let token = token.trim();
        if token.is_empty() || token.chars().any(|c| c == '{' || c == '}' || c == ';') {
            return None;
        }
        values.insert(token.to_string());
    }
    Some(values)
}

fn result(
    status: SetFrontendStatus,
    request: Option<SetRequest>,
    bindings: BTreeMap<String, BTreeSet<String>>,
    unresolved: Vec<String>,
    provenance: Vec<String>,
) -> SetFrontendResult {
    finish(SetFrontendResult {
        status,
        request,
        bindings,
        unresolved,
        provenance,
        replay_hash: String::new(),
    })
}

pub fn formalize_set_text(text: &str, case_id: &str) -> SetFrontendResult {
    let lower = text.to_ascii_lowercase();
    let provenance = vec![
        format!("source-set-frontend:{case_id}"),
        "explicit-finite-set-parser".into(),
    ];
    if [
        "infinite",
        "interval",
        "venn diagram",
        "measure",
        "topology",
        "probability density",
        "diagram",
    ]
    .iter()
    .any(|term| lower.contains(term))
    {
        return result(SetFrontendStatus::Unsupported, None, BTreeMap::new(), vec!["infinite, diagrammatic, measure, or cross-domain set semantics are outside this bounded frontend".into()], provenance);
    }
    if lower.contains(" or ")
        || lower.contains("either")
        || (lower.contains("union") && lower.contains("intersection") && !lower.contains('('))
    {
        return result(
            SetFrontendStatus::Ambiguous,
            None,
            BTreeMap::new(),
            vec!["operation or precedence is not unique".into()],
            provenance,
        );
    }
    let universe = parse_set(text, "u");
    let left = parse_set(text, "a");
    let right = parse_set(text, "b");
    let operation = if lower.contains("complement") || lower.contains("a'") || lower.contains("aᶜ")
    {
        Some(SetOperation::Complement)
    } else if lower.contains("cardinality") || lower.contains("size of") || lower.contains("|a|") {
        Some(SetOperation::Cardinality)
    } else if lower.contains('∪') || lower.contains(" union ") {
        Some(SetOperation::Union)
    } else if lower.contains('∩') || lower.contains(" intersection ") {
        Some(SetOperation::Intersection)
    } else if lower.contains(" difference ") || lower.contains("\\") {
        Some(SetOperation::Difference)
    } else {
        None
    };
    let Some(operation) = operation else {
        return result(
            SetFrontendStatus::Unsupported,
            None,
            BTreeMap::new(),
            vec!["an explicit finite-set operation is required".into()],
            provenance,
        );
    };
    let Some(left) = left else {
        return result(
            SetFrontendStatus::Missing,
            None,
            BTreeMap::new(),
            vec!["left operand A is not explicitly enumerated".into()],
            provenance,
        );
    };
    let universe = universe.unwrap_or_default();
    let right = right.unwrap_or_default();
    if operation != SetOperation::Complement && universe.is_empty() {
        return result(
            SetFrontendStatus::Missing,
            None,
            BTreeMap::new(),
            vec!["an explicit universe U is required for bounded set operations".into()],
            provenance,
        );
    }
    if operation == SetOperation::Complement && universe.is_empty() {
        return result(
            SetFrontendStatus::Missing,
            None,
            BTreeMap::new(),
            vec!["complement requires explicit U".into()],
            provenance,
        );
    }
    let request = SetRequest {
        operation,
        universe: universe.clone(),
        left: left.clone(),
        right: right.clone(),
        ambiguity: None,
        provenance: provenance.clone(),
    };
    let mut bindings = BTreeMap::new();
    bindings.insert("U".into(), universe);
    bindings.insert("A".into(), left);
    bindings.insert("B".into(), right);
    result(
        SetFrontendStatus::Complete,
        Some(request),
        bindings,
        Vec::new(),
        provenance,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_explicit_union() {
        let result = formalize_set_text("Given U={1,2,3}, A={1,2}, B={2,3}, find A union B.", "t");
        assert_eq!(result.status, SetFrontendStatus::Complete);
        assert!(replay_verified(&result));
    }
    #[test]
    fn refuses_implicit_complement() {
        let result = formalize_set_text("Find the complement of A={1,2}.", "t");
        assert_eq!(result.status, SetFrontendStatus::Missing);
    }
}
