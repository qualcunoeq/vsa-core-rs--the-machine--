//! Shadow-only grounding from a question-level law reference to typed records.
//!
//! Matching is deliberately evidence-based: aliases and descriptive terms are
//! supplied by the catalog, and domain/variable constraints only narrow a
//! match. The module never invents a law from a broad subject label.

use crate::law_bridge::LawRecord;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GroundingLaw {
    pub law: LawRecord,
    pub descriptive_terms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LawGroundingRequest {
    pub explicit_references: Vec<String>,
    pub described_phenomenon: Option<String>,
    pub domain: Option<String>,
    pub expected_variables: Vec<String>,
    pub requested_output: String,
    pub nearby_equations: Vec<String>,
    pub context: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GroundingStatus {
    Unique,
    Ambiguous,
    Unsupported,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GroundingEvidence {
    pub law_id: String,
    pub explicit_alias_match: bool,
    pub descriptive_match: bool,
    pub domain_match: bool,
    pub expected_variables_present: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LawGroundingResult {
    pub status: GroundingStatus,
    pub candidates: Vec<LawRecord>,
    pub evidence: Vec<GroundingEvidence>,
    pub unresolved: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("grounding value serializes"))
    )
}

fn normalized(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn same_phrase(left: &str, right: &str) -> bool {
    let left = left
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let right = right
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_lowercase())
        .collect::<Vec<_>>();
    !left.is_empty() && left == right
}

pub fn ground_law_reference(
    request: &LawGroundingRequest,
    catalog: &[GroundingLaw],
) -> LawGroundingResult {
    let domain = request.domain.as_deref().map(normalized);
    let mut candidates = Vec::new();
    let mut evidence = Vec::new();
    for entry in catalog {
        let explicit_alias_match = request.explicit_references.iter().any(|reference| {
            same_phrase(&entry.law.law_id, reference)
                || entry
                    .law
                    .aliases
                    .iter()
                    .any(|alias| same_phrase(alias, reference))
        });
        let descriptive_match = request
            .described_phenomenon
            .as_deref()
            .map(|phenomenon| {
                entry
                    .descriptive_terms
                    .iter()
                    .any(|term| same_phrase(term, phenomenon))
            })
            .unwrap_or(false);
        let domain_match = domain
            .as_deref()
            .map(|expected| normalized(&entry.law.domain) == expected)
            .unwrap_or(true);
        let expected_variables_present = request
            .expected_variables
            .iter()
            .all(|variable| entry.law.variables.contains(variable));
        let evidence_row = GroundingEvidence {
            law_id: entry.law.law_id.clone(),
            explicit_alias_match,
            descriptive_match,
            domain_match,
            expected_variables_present,
        };
        if (explicit_alias_match || descriptive_match) && domain_match {
            evidence.push(evidence_row);
            if expected_variables_present {
                candidates.push(entry.law.clone());
            }
        }
    }
    candidates.sort_by(|left, right| left.law_id.cmp(&right.law_id));
    evidence.sort_by(|left, right| left.law_id.cmp(&right.law_id));
    let (status, unresolved) = match candidates.len() {
        0 if request.explicit_references.is_empty() && request.described_phenomenon.is_none() => (
            GroundingStatus::Missing,
            vec!["no explicit law reference or descriptive phenomenon".into()],
        ),
        0 => (
            GroundingStatus::Unsupported,
            vec!["references do not identify a catalog law with compatible variables".into()],
        ),
        1 => (GroundingStatus::Unique, Vec::new()),
        _ => (
            GroundingStatus::Ambiguous,
            vec!["reference identifies multiple compatible law records".into()],
        ),
    };
    let replay_hash = digest(&(&status, &candidates, &evidence, &unresolved));
    LawGroundingResult {
        status,
        candidates,
        evidence,
        unresolved,
        replay_hash,
    }
}

pub fn replay_grounding(result: &LawGroundingResult) -> bool {
    digest(&(
        &result.status,
        &result.candidates,
        &result.evidence,
        &result.unresolved,
    )) == result.replay_hash
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ohm() -> GroundingLaw {
        GroundingLaw {
            law: LawRecord {
                law_id: "ohms_law".into(),
                aliases: vec!["Ohm's law".into()],
                domain: "physics".into(),
                equation: "V = I * R".into(),
                variables: vec!["V".into(), "I".into(), "R".into()],
                assumptions: vec!["constant resistance".into()],
                validity_domain: "lumped circuit".into(),
                unit_constraints: vec!["V=volt".into(), "I=ampere".into(), "R=ohm".into()],
                provenance: "independent:physics".into(),
            },
            descriptive_terms: vec!["voltage-current-resistance relation".into()],
        }
    }

    #[test]
    fn exact_alias_and_description_are_groundable() {
        let catalog = vec![ohm()];
        let alias = ground_law_reference(
            &LawGroundingRequest {
                explicit_references: vec!["Ohm's law".into()],
                described_phenomenon: None,
                domain: Some("physics".into()),
                expected_variables: vec!["V".into(), "I".into(), "R".into()],
                requested_output: "V".into(),
                nearby_equations: Vec::new(),
                context: "resistor".into(),
            },
            &catalog,
        );
        assert_eq!(alias.status, GroundingStatus::Unique);
        assert!(replay_grounding(&alias));

        let description = ground_law_reference(
            &LawGroundingRequest {
                explicit_references: Vec::new(),
                described_phenomenon: Some("voltage-current-resistance relation".into()),
                domain: Some("physics".into()),
                expected_variables: vec!["V".into(), "I".into(), "R".into()],
                requested_output: "V".into(),
                nearby_equations: Vec::new(),
                context: "resistor".into(),
            },
            &catalog,
        );
        assert_eq!(description.status, GroundingStatus::Unique);
    }

    #[test]
    fn broad_or_missing_references_do_not_guess() {
        let result = ground_law_reference(
            &LawGroundingRequest {
                explicit_references: vec!["law".into()],
                described_phenomenon: None,
                domain: None,
                expected_variables: Vec::new(),
                requested_output: "choice".into(),
                nearby_equations: Vec::new(),
                context: "background mentions a law".into(),
            },
            &[ohm()],
        );
        assert_eq!(result.status, GroundingStatus::Unsupported);
        assert!(replay_grounding(&result));
    }
}
