//! Domain-agnostic frontend for declarative source formula catalogs.
//!
//! The frontend uses only aliases and declared input names from source
//! records. It has no formula- or subject-specific branches: a unique source
//! record plus explicit labeled values is required before a typed request is
//! emitted.

use crate::probability_pack::Rational;
use crate::source_formula_pack::{FormulaRecord, FormulaRequest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FormulaFrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormulaFrontendResult {
    pub status: FormulaFrontendStatus,
    pub formula: Option<String>,
    pub request: Option<FormulaRequest>,
    pub provenance_spans: Vec<String>,
    pub alternatives: Vec<String>,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("formula frontend serializes"))
    )
}

fn payload(result: &FormulaFrontendResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.formula,
        &result.request,
        &result.provenance_spans,
        &result.alternatives,
        &result.reasons,
    )
}

fn result(
    status: FormulaFrontendStatus,
    formula: Option<String>,
    request: Option<FormulaRequest>,
    provenance_spans: Vec<String>,
    alternatives: Vec<String>,
    reasons: Vec<String>,
) -> FormulaFrontendResult {
    let mut output = FormulaFrontendResult {
        status,
        formula,
        request,
        provenance_spans,
        alternatives,
        reasons,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&output));
    output.replay_hash = replay_hash;
    output
}

fn rational_token(token: &str) -> Option<Rational> {
    let cleaned = token.trim_matches(|character: char| {
        !character.is_ascii_digit() && character != '-' && character != '/'
    });
    if let Some((numerator, denominator)) = cleaned.split_once('/') {
        return Rational::new(numerator.parse().ok()?, denominator.parse().ok()?);
    }
    Rational::new(cleaned.parse().ok()?, 1)
}

fn labeled_value(text: &str, label: &str) -> Option<(String, Rational)> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    for (index, token) in tokens.iter().enumerate() {
        let normalized = token.trim_matches(|character: char| {
            !character.is_ascii_alphanumeric()
                && character != '_'
                && character != '='
                && character != ':'
                && character != '-'
                && character != '/'
        });
        if normalized == label {
            let mut value_index = index + 1;
            while matches!(tokens.get(value_index), Some(&"=" | &":")) {
                value_index += 1;
            }
            if let Some(value) = tokens
                .get(value_index)
                .and_then(|value| rational_token(value))
            {
                return Some((
                    format!("{label} {}", tokens[index..=value_index].join(" ")),
                    value,
                ));
            }
        }
        for separator in ['=', ':'] {
            let prefix = format!("{label}{separator}");
            if let Some(value) = normalized.strip_prefix(&prefix) {
                return rational_token(value).map(|value| (normalized.to_owned(), value));
            }
        }
    }
    None
}

fn normalize(value: &str) -> String {
    value.to_ascii_lowercase()
}

/// Convert raw text into a typed formula request using only source-declared
/// aliases and input names. No subject vocabulary is hard-coded here.
pub fn formalize_formula_text(
    text: &str,
    domain: &str,
    records: &[FormulaRecord],
) -> FormulaFrontendResult {
    let lower = normalize(text);
    if ["continuous", "infinite", "differential", "optimization"]
        .iter()
        .any(|marker| lower.contains(marker))
    {
        return result(
            FormulaFrontendStatus::Unsupported,
            None,
            None,
            vec![text.into()],
            Vec::new(),
            vec!["text requests semantics outside the declarative formula boundary".into()],
        );
    }
    let mut candidates = BTreeSet::new();
    for record in records {
        if lower.contains(&normalize(&record.formula_id))
            || record
                .aliases
                .iter()
                .any(|alias| lower.contains(&normalize(alias)))
        {
            candidates.insert(record.formula_id.clone());
        }
    }
    if candidates.is_empty() {
        return result(
            FormulaFrontendStatus::Missing,
            None,
            None,
            vec![text.into()],
            Vec::new(),
            vec!["no unique source formula alias was found".into()],
        );
    }
    if candidates.len() != 1 {
        return result(
            FormulaFrontendStatus::Ambiguous,
            None,
            None,
            vec![text.into()],
            candidates.into_iter().collect(),
            vec!["multiple source formulas match the text".into()],
        );
    }
    let formula = candidates.into_iter().next().expect("one candidate");
    let record = records
        .iter()
        .find(|record| record.formula_id == formula)
        .expect("candidate exists");
    let mut inputs = BTreeMap::new();
    let mut spans = vec![format!("formula:{formula}")];
    for input in &record.required_inputs {
        let Some((span, value)) = labeled_value(&lower, input) else {
            return result(
                FormulaFrontendStatus::Missing,
                Some(formula),
                None,
                spans,
                Vec::new(),
                vec![format!("declared input {input} is not explicitly labeled")],
            );
        };
        spans.push(span);
        inputs.insert(input.clone(), value);
    }
    result(
        FormulaFrontendStatus::Complete,
        Some(formula.clone()),
        Some(FormulaRequest {
            formula,
            inputs,
            domain: domain.into(),
            ambiguity: None,
            provenance: spans.clone(),
        }),
        spans,
        Vec::new(),
        Vec::new(),
    )
}

impl FormulaFrontendResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_formula_pack::extract_formula_records;

    #[test]
    fn generic_alias_and_input_binding_is_fail_closed() {
        let source = "BEGIN FORMULA ratio\nALIASES: quotient\nEXPRESSION: a / b\nINPUTS: a, b\nASSUMPTIONS: b positive\nCONSTRAINTS: positive:a; positive:b\nSOURCE_ID: test\nTITLE: Test\nSECTION: Test\nURL: https://example.invalid/test\nLICENSE: test\nRETRIEVED: 2026-08-16\nEVIDENCE: ratio definition\nEND FORMULA";
        let records = extract_formula_records(source).unwrap();
        let complete =
            formalize_formula_text("Compute the quotient with a=6 and b = 2.", "test", &records);
        assert_eq!(complete.status, FormulaFrontendStatus::Complete);
        assert!(complete.replay_verified());
        let missing = formalize_formula_text("Compute the quotient with a=6.", "test", &records);
        assert_eq!(missing.status, FormulaFrontendStatus::Missing);
    }
}
