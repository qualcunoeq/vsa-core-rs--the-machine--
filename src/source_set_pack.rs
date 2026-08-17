//! Source-derived finite set operations.
//!
//! The source supplies the semantic definitions; this pack executes only
//! explicit finite sets.  Complements always name an explicit universe and no
//! infinite, interval, measure, or diagram semantics are inferred.

use crate::source_formula_pack::SourceCitation;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const DOMAIN: &str = "source_derived_finite_set_operations";
pub const SOURCE_ID: &str = "openstax-contemporary-mathematics:finite-set-operations";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SetOperation {
    Union,
    Intersection,
    Difference,
    Complement,
    Cardinality,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SetRequest {
    pub operation: SetOperation,
    pub universe: BTreeSet<String>,
    pub left: BTreeSet<String>,
    pub right: BTreeSet<String>,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SetArtifact {
    FiniteSet(BTreeSet<String>),
    Cardinality(usize),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SetStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidUniverse,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SetResult {
    pub status: SetStatus,
    pub artifact: Option<SetArtifact>,
    pub operation: SetOperation,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub source: SourceCitation,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

pub fn source() -> SourceCitation {
    SourceCitation {
        source_id: SOURCE_ID.into(),
        title: "Contemporary Mathematics".into(),
        section: "1.4 Set Operations with Two Sets; 1.5 Set Operations with Three Sets".into(),
        url: "https://openstax.org/books/contemporary-mathematics/pages/1-4-set-operations-with-two-sets".into(),
        license: "CC BY-NC-SA 4.0; OpenStax attribution required".into(),
        retrieved_utc: "2026-08-17".into(),
        evidence_span: "finite union, intersection, difference, complement relative to a universal set, and parenthesized operation order".into(),
    }
}

pub fn validate_source_document(document: &str) -> bool {
    [
        "SOURCE_ID:",
        "URL:",
        "EVIDENCE:",
        "union",
        "intersection",
        "complement",
    ]
    .iter()
    .all(|marker| {
        document
            .to_ascii_lowercase()
            .contains(&marker.to_ascii_lowercase())
    })
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("set result serializes"))
    )
}

fn finish(mut result: SetResult) -> SetResult {
    result.replay_hash.clear();
    result.replay_hash = digest(&result);
    result
}

pub fn replay_verified(result: &SetResult) -> bool {
    let mut copy = result.clone();
    let hash = copy.replay_hash.clone();
    copy.replay_hash.clear();
    hash == digest(&copy) && !result.provenance.is_empty()
}

pub fn evaluate(request: &SetRequest) -> SetResult {
    let assumptions = vec![
        "finite explicitly enumerated sets".into(),
        "set membership is exact".into(),
    ];
    let base = |status, artifact, reasons: Vec<String>| {
        finish(SetResult {
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
        return base(
            SetStatus::Missing,
            None,
            vec!["provenance is required".into()],
        );
    }
    if request.ambiguity.is_some() {
        return base(
            SetStatus::Ambiguous,
            None,
            vec![request.ambiguity.clone().unwrap()],
        );
    }
    if request.operation == SetOperation::Complement && request.universe.is_empty() {
        return base(
            SetStatus::InvalidUniverse,
            None,
            vec!["complement requires an explicit non-empty universe".into()],
        );
    }
    let operands_in_universe =
        request.left.is_subset(&request.universe) && request.right.is_subset(&request.universe);
    if !operands_in_universe {
        return base(
            SetStatus::InvalidUniverse,
            None,
            vec!["operand contains an element outside the declared universe".into()],
        );
    }
    let artifact = match request.operation {
        SetOperation::Union => {
            SetArtifact::FiniteSet(request.left.union(&request.right).cloned().collect())
        }
        SetOperation::Intersection => {
            SetArtifact::FiniteSet(request.left.intersection(&request.right).cloned().collect())
        }
        SetOperation::Difference => {
            SetArtifact::FiniteSet(request.left.difference(&request.right).cloned().collect())
        }
        SetOperation::Complement => SetArtifact::FiniteSet(
            request
                .universe
                .difference(&request.left)
                .cloned()
                .collect(),
        ),
        SetOperation::Cardinality => SetArtifact::Cardinality(request.left.len()),
    };
    base(SetStatus::Complete, Some(artifact), Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(operation: SetOperation) -> SetRequest {
        SetRequest {
            operation,
            universe: ["1", "2", "3"].into_iter().map(String::from).collect(),
            left: ["1", "2"].into_iter().map(String::from).collect(),
            right: ["2", "3"].into_iter().map(String::from).collect(),
            ambiguity: None,
            provenance: vec!["test".into()],
        }
    }
    #[test]
    fn exact_operations_replay() {
        let result = evaluate(&request(SetOperation::Union));
        assert_eq!(
            result.artifact,
            Some(SetArtifact::FiniteSet(
                ["1", "2", "3"].into_iter().map(String::from).collect()
            ))
        );
        assert!(replay_verified(&result));
    }
    #[test]
    fn complement_requires_universe_membership() {
        let mut bad = request(SetOperation::Complement);
        bad.left.insert("9".into());
        assert_eq!(evaluate(&bad).status, SetStatus::InvalidUniverse);
    }
}
