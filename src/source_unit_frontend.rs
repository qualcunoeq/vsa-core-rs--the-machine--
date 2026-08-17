//! Bounded language frontend for the source-derived unit catalog.

use crate::source_formula_pack::{FormulaRecord, FormulaRequest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnitFrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnitFrontendResult {
    pub status: UnitFrontendStatus,
    pub request: Option<FormulaRequest>,
    pub source_unit: Option<String>,
    pub target_unit: Option<String>,
    pub evidence: Vec<String>,
    pub unresolved: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn output(mut result: UnitFrontendResult) -> UnitFrontendResult {
    result.replay_hash.clear();
    result.replay_hash = digest(&result);
    result
}

pub fn replay_verified(result: &UnitFrontendResult) -> bool {
    let mut copy = result.clone();
    let hash = copy.replay_hash.clone();
    copy.replay_hash.clear();
    hash == digest(&copy) && !result.provenance.is_empty()
}

fn amount_and_units(text: &str) -> Option<(i128, String, String)> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let amount_index = tokens
        .iter()
        .position(|token| token.parse::<i128>().is_ok())?;
    let amount = tokens[amount_index].parse::<i128>().ok()?;
    let source_index = amount_index + 1;
    let source = tokens
        .get(source_index)?
        .trim_matches(|c: char| !c.is_ascii_alphabetic());
    let target_marker = tokens
        .iter()
        .enumerate()
        .skip(source_index + 1)
        .find(|(_, token)| **token == "to" || **token == "into" || **token == "as")
        .map(|(index, _)| index)?;
    let target = tokens
        .get(target_marker + 1)?
        .trim_matches(|c: char| !c.is_ascii_alphabetic());
    if source.is_empty() || target.is_empty() {
        return None;
    }
    Some((
        amount,
        source.to_ascii_lowercase(),
        target.to_ascii_lowercase(),
    ))
}

pub fn formalize_unit_text(
    text: &str,
    case_id: &str,
    records: &[FormulaRecord],
) -> UnitFrontendResult {
    let lower = text.to_ascii_lowercase();
    let provenance = vec![format!("source-unit-frontend:{case_id}")];
    if lower.contains("approx")
        || lower.contains("unknown unit")
        || lower.contains("temperature")
        || lower.contains("density")
    {
        return output(UnitFrontendResult {
            status: UnitFrontendStatus::Unsupported,
            request: None,
            source_unit: None,
            target_unit: None,
            evidence: Vec::new(),
            unresolved: vec!["unsupported or approximate unit semantics".into()],
            provenance,
            replay_hash: String::new(),
        });
    }
    if lower.contains(" or ") || lower.contains("either ") {
        return output(UnitFrontendResult {
            status: UnitFrontendStatus::Ambiguous,
            request: None,
            source_unit: None,
            target_unit: None,
            evidence: Vec::new(),
            unresolved: vec!["more than one target conversion is stated".into()],
            provenance,
            replay_hash: String::new(),
        });
    }
    let Some((amount, source_unit, target_unit)) = amount_and_units(&lower) else {
        return output(UnitFrontendResult {
            status: UnitFrontendStatus::Missing,
            request: None,
            source_unit: None,
            target_unit: None,
            evidence: Vec::new(),
            unresolved: vec!["amount and explicit source/target units are required".into()],
            provenance,
            replay_hash: String::new(),
        });
    };
    let alias = format!("{source_unit} to {target_unit}");
    let matches: Vec<&FormulaRecord> = records
        .iter()
        .filter(|record| record.aliases.iter().any(|candidate| candidate == &alias))
        .collect();
    if matches.len() != 1 {
        return output(UnitFrontendResult {
            status: if matches.is_empty() {
                UnitFrontendStatus::Unsupported
            } else {
                UnitFrontendStatus::Ambiguous
            },
            request: None,
            source_unit: Some(source_unit),
            target_unit: Some(target_unit),
            evidence: vec![alias],
            unresolved: vec!["unit pair does not select one source record".into()],
            provenance,
            replay_hash: String::new(),
        });
    }
    let record = matches[0];
    output(UnitFrontendResult {
        status: UnitFrontendStatus::Complete,
        request: Some(FormulaRequest {
            formula: record.formula_id.clone(),
            inputs: BTreeMap::from([(
                "amount".into(),
                crate::probability_pack::Rational::new(amount, 1).unwrap(),
            )]),
            domain: "source_catalog_unit_conversion".into(),
            ambiguity: None,
            provenance: provenance.clone(),
        }),
        source_unit: Some(source_unit),
        target_unit: Some(target_unit),
        evidence: vec![alias],
        unresolved: Vec::new(),
        provenance,
        replay_hash: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_formula_pack::extract_formula_records;

    #[test]
    fn selects_one_explicit_catalog_conversion() {
        let source = include_str!("../docs/sources/openstax_unit_conversion_catalog.txt");
        let records = extract_formula_records(source).unwrap();
        let result = formalize_unit_text("Convert 3 meters to centimeters.", "test", &records);
        assert_eq!(result.status, UnitFrontendStatus::Complete);
        assert!(replay_verified(&result));
    }
}
