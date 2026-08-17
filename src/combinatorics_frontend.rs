//! Controlled technical-language frontend for bounded exact combinatorics.
//!
//! This module constructs requests only when one counting operation and all of
//! its explicit operands are present.  It does not infer a counting model from
//! generic words such as "ways" or "arrangements" and never authorizes an
//! answer by itself.

use crate::combinatorics_pack::{CombinatoricsOperation, CombinatoricsRequest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CombinatoricsFrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CombinatoricsFrontendResult {
    pub status: CombinatoricsFrontendStatus,
    pub request: Option<CombinatoricsRequest>,
    pub unresolved: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("frontend serializes"))
    )
}

fn finish(mut result: CombinatoricsFrontendResult) -> CombinatoricsFrontendResult {
    result.replay_hash.clear();
    result.replay_hash = digest(&result);
    result
}

pub fn replay_verified(result: &CombinatoricsFrontendResult) -> bool {
    let mut copy = result.clone();
    let hash = copy.replay_hash.clone();
    copy.replay_hash.clear();
    hash == digest(&copy) && !result.provenance.is_empty()
}

fn marker_count(text: &str, marker: &str) -> usize {
    text.match_indices(marker).count()
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

fn numbers_after(text: &str, label: &str) -> Vec<u64> {
    let Some(start) = text.find(label).map(|index| index + label.len()) else {
        return Vec::new();
    };
    text[start..]
        .split(|c: char| c == ',' || c == ';' || c == ']' || c == ')')
        .flat_map(|chunk| {
            chunk.split_whitespace().filter_map(|token| {
                token
                    .trim_matches(|c: char| !c.is_ascii_digit())
                    .parse()
                    .ok()
            })
        })
        .collect()
}

fn request(operation: CombinatoricsOperation, provenance: &[String]) -> CombinatoricsRequest {
    CombinatoricsRequest {
        operation,
        n: None,
        k: None,
        parts: Vec::new(),
        first_count: None,
        second_count: None,
        intersection_count: None,
        objects: None,
        boxes: None,
        domain: "bounded_exact_combinatorics".into(),
        ambiguity: None,
        provenance: provenance.to_vec(),
    }
}

fn output(
    status: CombinatoricsFrontendStatus,
    request: Option<CombinatoricsRequest>,
    unresolved: Vec<String>,
    provenance: Vec<String>,
) -> CombinatoricsFrontendResult {
    finish(CombinatoricsFrontendResult {
        status,
        request,
        unresolved,
        provenance,
        replay_hash: String::new(),
    })
}

/// Parse one explicit bounded combinatorics request from controlled text.
pub fn formalize(text: &str, case_id: &str) -> CombinatoricsFrontendResult {
    let lower = text.to_ascii_lowercase();
    let provenance = vec![
        format!("combinatorics-frontend:{case_id}"),
        format!("source-span:0..{}", text.len()),
        "explicit-bounded-combinatorics-grammar".into(),
    ];
    if [
        "asymptotic",
        "infinite",
        "unbounded",
        "weighted",
        "random",
        "generating function",
        "approximate",
        "graph",
    ]
    .iter()
    .any(|term| lower.contains(term))
    {
        return output(
            CombinatoricsFrontendStatus::Unsupported,
            None,
            vec!["request exceeds bounded exact combinatorics".into()],
            provenance,
        );
    }
    let families: &[(&[&str], CombinatoricsOperation)] = &[
        (
            &["permutation", "arrangement", "npr"],
            CombinatoricsOperation::Permutations,
        ),
        (
            &["combination", "choose", "binomial", "ncr"],
            CombinatoricsOperation::Combinations,
        ),
        (
            &["multinomial", "partition into groups"],
            CombinatoricsOperation::Multinomial,
        ),
        (
            &["inclusion-exclusion", "union of two sets", "intersection"],
            CombinatoricsOperation::InclusionExclusionTwo,
        ),
        (
            &["pigeonhole", "objects into boxes"],
            CombinatoricsOperation::PigeonholeMinimum,
        ),
        (
            &["stirling number", "set partition"],
            CombinatoricsOperation::StirlingSecond,
        ),
        (
            &["surjection", "onto function"],
            CombinatoricsOperation::SurjectionCount,
        ),
    ];
    let matched: Vec<CombinatoricsOperation> = families
        .iter()
        .filter(|(markers, _)| markers.iter().any(|marker| lower.contains(marker)))
        .map(|(_, operation)| *operation)
        .collect();
    if matched.len() > 1 {
        return output(
            CombinatoricsFrontendStatus::Ambiguous,
            None,
            vec!["multiple counting operations or scopes are present".into()],
            provenance,
        );
    }
    if ["n=", "n =", "k=", "k =", "objects=", "boxes="]
        .iter()
        .any(|marker| marker_count(&lower, marker) > 1)
    {
        return output(
            CombinatoricsFrontendStatus::Ambiguous,
            None,
            vec!["a counting parameter appears in multiple local scopes".into()],
            provenance,
        );
    }
    let Some(operation) = matched.first().copied() else {
        return output(
            CombinatoricsFrontendStatus::Missing,
            None,
            vec!["no explicit bounded counting operation was identified".into()],
            provenance,
        );
    };
    let mut request = request(operation, &provenance);
    match operation {
        CombinatoricsOperation::Permutations
        | CombinatoricsOperation::Combinations
        | CombinatoricsOperation::StirlingSecond
        | CombinatoricsOperation::SurjectionCount => {
            request.n = integer_after(&lower, &["n=", "n =", "n ", "from "]);
            request.k = integer_after(&lower, &["k=", "k =", "k ", "choose "]);
        }
        CombinatoricsOperation::Multinomial => {
            request.parts = numbers_after(&lower, "parts");
        }
        CombinatoricsOperation::InclusionExclusionTwo => {
            request.first_count = integer_after(&lower, &["first=", "first set=", "|a|="]);
            request.second_count = integer_after(&lower, &["second=", "second set=", "|b|="]);
            request.intersection_count = integer_after(&lower, &["intersection=", "|a∩b|="]);
        }
        CombinatoricsOperation::PigeonholeMinimum => {
            request.objects = integer_after(&lower, &["objects=", "objects "]);
            request.boxes = integer_after(&lower, &["boxes=", "boxes "]);
        }
    }
    let complete = match operation {
        CombinatoricsOperation::Multinomial => !request.parts.is_empty(),
        CombinatoricsOperation::InclusionExclusionTwo => {
            request.first_count.is_some()
                && request.second_count.is_some()
                && request.intersection_count.is_some()
        }
        CombinatoricsOperation::PigeonholeMinimum => {
            request.objects.is_some() && request.boxes.is_some()
        }
        _ => request.n.is_some() && request.k.is_some(),
    };
    if !complete {
        return output(
            CombinatoricsFrontendStatus::Missing,
            None,
            vec!["the selected counting operation lacks explicit bounded operands".into()],
            provenance,
        );
    }
    output(
        CombinatoricsFrontendStatus::Complete,
        Some(request),
        Vec::new(),
        provenance,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_count_replays() {
        let result = formalize("Compute the combinations choose n=8 k=3.", "direct");
        assert_eq!(result.status, CombinatoricsFrontendStatus::Complete);
        assert!(replay_verified(&result));
    }

    #[test]
    fn competing_scope_is_ambiguous() {
        let result = formalize(
            "A quote gives combinations n=8 k=3, while another scope asks permutations n=8 k=3; choose neither.",
            "scoped",
        );
        assert_eq!(result.status, CombinatoricsFrontendStatus::Ambiguous);
    }
}
