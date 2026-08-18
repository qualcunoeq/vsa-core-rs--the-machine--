//! Generic technical-language frontend for source-derived formula catalogs.
//!
//! Formula records supply aliases, required inputs, constraints, and
//! provenance.  This frontend only lowers explicit aliases plus explicitly
//! labeled rational inputs; it never infers a formula from a subject keyword
//! or invents omitted quantities.

use crate::probability_pack::Rational;
use crate::source_formula_pack::{FormulaRecord, FormulaRequest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrontendStatus { Complete, Ambiguous, Missing, Unsupported }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceFormulaFrontendResult {
    pub status: FrontendStatus,
    pub formula_id: Option<String>,
    /// Compatibility field retained for existing source-catalog routes.
    pub formula: Option<String>,
    pub request: Option<FormulaRequest>,
    pub provenance_spans: Vec<String>,
    pub alternatives: Vec<String>,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FormulaRegionRole {
    Target,
    Definition,
    Context,
    Incidental,
    Ambiguous,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormulaRegion {
    pub span: String,
    pub role: FormulaRegionRole,
    pub candidates: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceFormulaReportResult {
    pub frontend: SourceFormulaFrontendResult,
    pub regions: Vec<FormulaRegion>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String { format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap())) }

fn payload(result: &SourceFormulaFrontendResult) -> impl Serialize + '_ {
    (&result.status, &result.formula_id, &result.formula, &result.request, &result.provenance_spans, &result.alternatives, &result.reasons)
}

fn output(status: FrontendStatus, formula_id: Option<String>, request: Option<FormulaRequest>, spans: Vec<String>, alternatives: Vec<String>, reasons: Vec<String>) -> SourceFormulaFrontendResult {
    let formula = formula_id.clone();
    let replay_hash = digest(&(&status, &formula_id, &formula, &request, &spans, &alternatives, &reasons));
    SourceFormulaFrontendResult { status, formula_id, formula, request, provenance_spans: spans, alternatives, reasons, replay_hash }
}

fn report_digest(result: &SourceFormulaReportResult) -> String {
    digest(&(&result.frontend.replay_hash, &result.regions))
}

fn region_slices(text: &str) -> Vec<(usize, usize, &str)> {
    let mut slices = Vec::new();
    let mut start = 0;
    for (index, character) in text.char_indices() {
        if matches!(character, '.' | ';' | '\n') {
            if start < index && !text[start..index].trim().is_empty() {
                slices.push((start, index, &text[start..index]));
            }
            start = index + character.len_utf8();
        }
    }
    if start < text.len() && !text[start..].trim().is_empty() {
        slices.push((start, text.len(), &text[start..]));
    }
    slices
}

fn has_any_marker(text: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| text.contains(marker))
}

fn normalize_phrase(value: &str) -> String {
    value.to_ascii_lowercase().replace(['_', '-'], " ")
}

fn parse_rational(value: &str) -> Option<Rational> {
    let value = value.trim();
    if let Some((numerator, denominator)) = value.split_once('/') {
        Rational::new(numerator.parse().ok()?, denominator.parse().ok()?)
    } else {
        Rational::new(value.parse().ok()?, 1)
    }
}

fn labeled_values(text: &str, label: &str) -> Vec<(String, Rational)> {
    let lower = text.to_ascii_lowercase().replace(['_', '-'], " ");
    let label = normalize_phrase(label);
    let mut results = Vec::new();
    let mut offset = 0;
    while let Some(relative) = lower[offset..].find(&label) {
        let start = offset + relative;
        let before_ok = start == 0 || !lower.as_bytes()[start - 1].is_ascii_alphanumeric();
        let end = start + label.len();
        let after_ok = end == lower.len() || !lower.as_bytes()[end].is_ascii_alphanumeric();
        if before_ok && after_ok {
            let mut cursor = end;
            while lower.as_bytes().get(cursor).is_some_and(|byte| byte.is_ascii_whitespace()) { cursor += 1; }
            if lower.as_bytes().get(cursor).is_some_and(|byte| *byte == b'=' || *byte == b':') {
                cursor += 1;
                while lower.as_bytes().get(cursor).is_some_and(|byte| byte.is_ascii_whitespace()) { cursor += 1; }
                let value_start = cursor;
                if lower.as_bytes().get(cursor) == Some(&b'-') { cursor += 1; }
                while lower.as_bytes().get(cursor).is_some_and(|byte| byte.is_ascii_digit()) { cursor += 1; }
                if lower.as_bytes().get(cursor) == Some(&b'/') {
                    cursor += 1;
                    while lower.as_bytes().get(cursor).is_some_and(|byte| byte.is_ascii_digit()) { cursor += 1; }
                }
                if cursor > value_start {
                    if let Some(value) = parse_rational(&lower[value_start..cursor]) {
                        results.push((format!("{label}={}", &lower[value_start..cursor]), value));
                    }
                }
            }
        }
        offset = end.max(offset + 1);
    }
    results
}

fn matching_records<'a>(text: &str, records: &'a [FormulaRecord]) -> Vec<&'a FormulaRecord> {
    let lower = text.to_ascii_lowercase().replace(['_', '-'], " ");
    let mut matches = Vec::new();
    for record in records {
        let candidates = std::iter::once(record.formula_id.as_str()).chain(record.aliases.iter().map(String::as_str));
        if candidates.into_iter().any(|candidate| lower.contains(&normalize_phrase(candidate))) {
            if !matches.iter().any(|existing: &&FormulaRecord| existing.formula_id == record.formula_id) {
                matches.push(record);
            }
        }
    }
    matches
}

/// Lower a technical report into a source-derived formula request.
pub fn formalize_source_formula_text(text: &str, domain: &str, records: &[FormulaRecord]) -> SourceFormulaFrontendResult {
    let lower = text.to_ascii_lowercase().replace(['_', '-'], " ");
    let base_spans = vec![format!("source-formula-text:0..{}", text.len())];
    if ["asymptotic", "infinite", "approximate", "continuous"].iter().any(|marker| lower.contains(marker)) {
        return output(FrontendStatus::Unsupported, None, None, base_spans, Vec::new(), vec!["request is outside the finite source-formula boundary".into()]);
    }
    let matches = matching_records(text, records);
    if matches.is_empty() {
        return output(FrontendStatus::Missing, None, None, base_spans, Vec::new(), vec!["no unique source formula alias was stated".into()]);
    }
    if matches.len() > 1 || (matches.len() == 1 && lower.contains(" or ")) {
        return output(FrontendStatus::Ambiguous, None, None, base_spans, matches.iter().map(|record| record.formula_id.clone()).collect(), vec!["multiple formula interpretations remain".into()]);
    }
    let record = matches[0];
    let mut inputs = BTreeMap::new();
    let mut spans = base_spans;
    for input in &record.required_inputs {
        let values = labeled_values(text, input);
        if values.len() != 1 {
            return output(FrontendStatus::Missing, Some(record.formula_id.clone()), None, spans, Vec::new(), vec![format!("required input {input} is missing or duplicated")]);
        }
        let (span, value) = values.into_iter().next().unwrap();
        spans.push(span);
        inputs.insert(input.clone(), value);
    }
    let request = FormulaRequest { formula: record.formula_id.clone(), inputs, domain: domain.into(), ambiguity: None, provenance: vec![format!("formula-id:{}", record.formula_id), format!("source:{}", record.source.source_id), format!("source-span:{}", record.source.evidence_span)] };
    output(FrontendStatus::Complete, Some(record.formula_id.clone()), Some(request), spans, Vec::new(), Vec::new())
}

/// Ground a multi-region technical report before formula lowering.
///
/// Operative clauses are identified only by explicit target verbs. Formula
/// mentions in definitions or incidental context are retained as provenance
/// but cannot steal the target. If multiple operative formulas remain, the
/// result is ambiguous; no lexical tie-break is used.
pub fn formalize_source_formula_report(
    text: &str,
    domain: &str,
    records: &[FormulaRecord],
) -> SourceFormulaReportResult {
    let mut regions = Vec::new();
    let mut target_ids = BTreeMap::new();
    let target_markers = ["calculate", "compute", "evaluate", "find", "determine", "apply", "use"];
    let context_markers = ["define", "given", "where", "assume", "reference", "according"];
    for (start, end, clause) in region_slices(text) {
        let lower = clause.to_ascii_lowercase().replace(['_', '-'], " ");
        let candidates = matching_records(clause, records)
            .into_iter()
            .map(|record| record.formula_id.clone())
            .collect::<Vec<_>>();
        let role = if candidates.is_empty() {
            FormulaRegionRole::Context
        } else if has_any_marker(&lower, &target_markers) {
            for candidate in &candidates {
                target_ids.insert(candidate.clone(), format!("{start}..{end}"));
            }
            if candidates.len() == 1 {
                FormulaRegionRole::Target
            } else {
                FormulaRegionRole::Ambiguous
            }
        } else if has_any_marker(&lower, &context_markers) {
            FormulaRegionRole::Definition
        } else {
            FormulaRegionRole::Incidental
        };
        regions.push(FormulaRegion {
            span: format!("source-formula-region:{start}..{end}"),
            role,
            candidates,
        });
    }

    let target_ids = target_ids.into_iter().collect::<Vec<_>>();
    let frontend = if target_ids.len() > 1 {
        output(
            FrontendStatus::Ambiguous,
            None,
            None,
            regions.iter().map(|region| region.span.clone()).collect(),
            target_ids.iter().map(|(id, _)| id.clone()).collect(),
            vec!["multiple operative formula targets remain".into()],
        )
    } else if let Some((target_id, _)) = target_ids.first() {
        let selected = records
            .iter()
            .find(|record| record.formula_id == *target_id)
            .expect("target record exists");
        formalize_source_formula_text(text, domain, std::slice::from_ref(selected))
    } else {
        let unique = regions
            .iter()
            .flat_map(|region| region.candidates.iter())
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        if unique.len() > 1 {
            output(
                FrontendStatus::Ambiguous,
                None,
                None,
                regions.iter().map(|region| region.span.clone()).collect(),
                unique.into_iter().collect(),
                vec!["formula mentions lack a unique operative target".into()],
            )
        } else {
            formalize_source_formula_text(text, domain, records)
        }
    };
    let mut result = SourceFormulaReportResult {
        frontend,
        regions,
        replay_hash: String::new(),
    };
    result.replay_hash = report_digest(&result);
    result
}

pub fn report_replay_verified(result: &SourceFormulaReportResult) -> bool {
    result.frontend.replay_verified() && result.replay_hash == report_digest(result)
}

pub fn replay_verified(result: &SourceFormulaFrontendResult) -> bool {
    result.replay_hash == digest(&payload(result)) && !result.provenance_spans.is_empty()
}

/// Compatibility names for the original generic source-catalog frontend API.
pub type FormulaFrontendStatus = FrontendStatus;
pub type FormulaFrontendResult = SourceFormulaFrontendResult;

/// Preserve the established API while routing through the stricter frontend.
pub fn formalize_formula_text(text: &str, domain: &str, records: &[FormulaRecord]) -> FormulaFrontendResult {
    formalize_source_formula_text(text, domain, records)
}

impl SourceFormulaFrontendResult {
    pub fn replay_verified(&self) -> bool { replay_verified(self) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_formula_pack::extract_formula_records;
    use crate::source_regression_pack;
    use crate::source_statistics_pack::{records, DOMAIN};

    #[test]
    fn generic_frontend_binds_source_record_without_domain_branch() {
        let result = formalize_source_formula_text("Use the sample mean: sum=30 and count=5.", DOMAIN, &records());
        assert_eq!(result.status, FrontendStatus::Complete);
        assert_eq!(result.formula_id.as_deref(), Some("arithmetic_mean"));
        assert!(replay_verified(&result));
    }

    #[test]
    fn generic_frontend_refuses_ambiguity_and_missing_inputs() {
        let records = records();
        let ambiguous = formalize_source_formula_text("Use the mean or weighted average: sum=30 count=5.", DOMAIN, &records);
        assert_eq!(ambiguous.status, FrontendStatus::Ambiguous);
        let missing = formalize_source_formula_text("Use the sample mean: sum=30.", DOMAIN, &records);
        assert_eq!(missing.status, FrontendStatus::Missing);
    }

    #[test]
    fn compatibility_api_preserves_replayable_result_shape() {
        let source = "BEGIN FORMULA ratio\nALIASES: quotient\nEXPRESSION: a / b\nINPUTS: a, b\nASSUMPTIONS: b positive\nCONSTRAINTS: positive:a; positive:b\nSOURCE_ID: test\nTITLE: Test\nSECTION: Test\nURL: https://example.invalid/test\nLICENSE: test\nRETRIEVED: 2026-08-16\nEVIDENCE: ratio definition\nEND FORMULA";
        let records = extract_formula_records(source).unwrap();
        let result = formalize_formula_text("Compute the quotient with a=6 and b=2.", "test", &records);
        assert_eq!(result.status, FormulaFrontendStatus::Complete);
        assert_eq!(result.formula.as_deref(), Some("ratio"));
        assert!(result.replay_verified());
    }

    #[test]
    fn catalog_domain_is_not_rejected_by_subject_word() {
        let result = formalize_source_formula_text(
            "Apply regression_slope: covariance_sum=3 and x_variance_sum=1.",
            source_regression_pack::DOMAIN,
            &source_regression_pack::records(),
        );
        assert_eq!(result.status, FrontendStatus::Complete);
        assert!(replay_verified(&result));
    }

    #[test]
    fn report_grounding_prefers_operative_formula_over_definition() {
        let records = records();
        let text = "For reference, arithmetic_mean is defined by sum/count. Calculate weighted_mean with weighted_sum=12 and total_weight=3.";
        let result = formalize_source_formula_report(text, DOMAIN, &records);
        assert_eq!(result.frontend.status, FrontendStatus::Complete);
        assert_eq!(result.frontend.formula_id.as_deref(), Some("weighted_mean"));
        assert!(result.regions.iter().any(|region| region.role == FormulaRegionRole::Definition));
        assert!(result.regions.iter().any(|region| region.role == FormulaRegionRole::Target));
        assert!(report_replay_verified(&result));
    }

    #[test]
    fn report_grounding_preserves_multiple_operative_targets() {
        let records = records();
        let result = formalize_source_formula_report(
            "Calculate arithmetic_mean or weighted_mean with sum=12 and count=3.",
            DOMAIN,
            &records,
        );
        assert_eq!(result.frontend.status, FrontendStatus::Ambiguous);
        assert!(result.frontend.request.is_none());
        assert!(report_replay_verified(&result));
    }
}
