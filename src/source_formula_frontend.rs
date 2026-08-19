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
pub enum FrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

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

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn payload(result: &SourceFormulaFrontendResult) -> impl Serialize + '_ {
    (
        &result.status,
        &result.formula_id,
        &result.formula,
        &result.request,
        &result.provenance_spans,
        &result.alternatives,
        &result.reasons,
    )
}

fn output(
    status: FrontendStatus,
    formula_id: Option<String>,
    request: Option<FormulaRequest>,
    spans: Vec<String>,
    alternatives: Vec<String>,
    reasons: Vec<String>,
) -> SourceFormulaFrontendResult {
    let formula = formula_id.clone();
    let replay_hash = digest(&(
        &status,
        &formula_id,
        &formula,
        &request,
        &spans,
        &alternatives,
        &reasons,
    ));
    SourceFormulaFrontendResult {
        status,
        formula_id,
        formula,
        request,
        provenance_spans: spans,
        alternatives,
        reasons,
        replay_hash,
    }
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

fn parse_simple_numeric_list(segment: &str) -> Option<Vec<Rational>> {
    let chars = segment.trim().chars().collect::<Vec<_>>();
    let mut values = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        while chars.get(index).is_some_and(|character| {
            character.is_ascii_whitespace() || matches!(character, ',' | ';')
        }) {
            index += 1;
        }
        if index >= chars.len() {
            break;
        }
        if chars[index..].starts_with(&['a', 'n', 'd'])
            && (index + 3 == chars.len() || !chars[index + 3].is_ascii_alphanumeric())
        {
            index += 3;
            continue;
        }
        let start = index;
        if chars[index] == '-' {
            index += 1;
        }
        let digit_start = index;
        while chars.get(index).is_some_and(char::is_ascii_digit) {
            index += 1;
        }
        if index == digit_start {
            return None;
        }
        if chars.get(index) == Some(&'/') && chars.get(index + 1).is_some_and(char::is_ascii_digit)
        {
            index += 1;
            while chars.get(index).is_some_and(char::is_ascii_digit) {
                index += 1;
            }
        }
        if chars
            .get(index)
            .is_some_and(|character| character.is_ascii_alphanumeric() || *character == '.')
        {
            return None;
        }
        let token = chars[start..index].iter().collect::<String>();
        values.push(parse_rational(&token)?);
    }
    (2..=20).contains(&values.len()).then_some(values)
}

fn sentence_end(text: &str, start: usize) -> usize {
    text[start..]
        .find(|character: char| matches!(character, '.' | '?' | '!' | '\n'))
        .map_or(text.len(), |offset| start + offset)
}

fn finite_list_inputs(text: &str) -> Option<(Rational, Rational, Vec<String>)> {
    let lower = text.to_ascii_lowercase();
    // This path is deliberately structural.  It accepts only a standalone
    // numeric list, never every numeral that happens to occur in a question.
    if lower.matches("arithmetic mean").count() != 1 {
        return None;
    }
    let mean_phrase = "arithmetic mean of";
    let mean_start = lower.find(mean_phrase)?;
    let mean_end = mean_start + mean_phrase.len();
    let mut candidates = Vec::new();

    // Direct form: "the arithmetic mean of 4, 8 and 12".
    let direct_end = sentence_end(text, mean_end);
    if let Some(values) = parse_simple_numeric_list(&text[mean_end..direct_end]) {
        candidates.push((mean_end, direct_end, values));
    }

    // Declared-list form: "scores are 89, 92, 88 ... What is the mean".
    // Only an explicit `are` declaration is accepted; ranges, filters, sets,
    // equations, and symbolic expressions therefore remain outside this path.
    let mut search = 0;
    while let Some(relative) = lower[search..mean_start].find(" are ") {
        let are_start = search + relative + " are ".len();
        let end = sentence_end(text, are_start);
        if let Some(values) = parse_simple_numeric_list(&text[are_start..end]) {
            candidates.push((are_start, end, values));
        }
        search = are_start;
    }
    if candidates.len() != 1 {
        return None;
    }
    let (start, end, values) = candidates.pop().unwrap();
    let sum = values
        .iter()
        .try_fold(Rational::zero(), |total, value| total.add(value))?;
    let count = Rational::new(values.len() as i128, 1)?;
    Some((sum, count, vec![format!("numeric-list:{start}..{end}")]))
}

fn sequence_integer_values(text: &str) -> Option<Vec<Rational>> {
    let lower = text.to_ascii_lowercase();
    let sequence_start = lower.find("sequence")? + "sequence".len();
    let sentence_end = sentence_end(text, sequence_start);
    let mut segment = &text[sequence_start..sentence_end];
    if let Some(are) = segment.to_ascii_lowercase().find(" are ") {
        segment = &segment[are + " are ".len()..];
    }
    if let Some(respectively) = segment.to_ascii_lowercase().find("respectively") {
        segment = &segment[..respectively];
    }
    if let Some(ellipsis) = segment.find("...") {
        segment = &segment[..ellipsis];
    }
    parse_simple_numeric_list(segment)
}

fn ordinal_token(token: &str) -> Option<i128> {
    let token = token
        .trim_matches(|character: char| !character.is_ascii_alphanumeric())
        .to_ascii_lowercase();
    if let Ok(number) = token
        .trim_end_matches(['s', 't', 'n', 'd', 'r', 'h'])
        .parse::<i128>()
    {
        return (number > 0).then_some(number);
    }
    [
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
    ]
    .into_iter()
    .find_map(|(word, number)| (token == word).then_some(number))
}

fn requested_ordinal(text: &str) -> Option<Rational> {
    let lower = text.to_ascii_lowercase();
    if ["sum", "series", "total"]
        .iter()
        .any(|marker| lower.contains(marker))
    {
        return None;
    }
    let tokens = lower.split_whitespace().collect::<Vec<_>>();
    let mut candidates = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if !token
            .trim_matches(|character: char| !character.is_ascii_alphabetic())
            .starts_with("term")
        {
            continue;
        }
        if let Some(previous) = index
            .checked_sub(1)
            .and_then(|position| tokens.get(position))
        {
            if let Some(number) = ordinal_token(previous) {
                candidates.push(number);
            }
        }
    }
    candidates.dedup();
    (candidates.len() == 1).then(|| Rational::new(candidates[0], 1).unwrap())
}

fn source_binding_value(text: &str, binding: &str) -> Option<Rational> {
    match binding {
        "first_integer_sequence_value" => sequence_integer_values(text)?.first().cloned(),
        "constant_integer_sequence_difference" => {
            let values = sequence_integer_values(text)?;
            let difference = values.get(1)?.sub(values.first()?)?;
            values
                .windows(2)
                .all(|pair| pair[1].sub(&pair[0]) == Some(difference.clone()))
                .then_some(difference)
        }
        "requested_ordinal" => requested_ordinal(text),
        _ => None,
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
            while lower
                .as_bytes()
                .get(cursor)
                .is_some_and(|byte| byte.is_ascii_whitespace())
            {
                cursor += 1;
            }
            if lower
                .as_bytes()
                .get(cursor)
                .is_some_and(|byte| *byte == b'=' || *byte == b':')
            {
                cursor += 1;
                while lower
                    .as_bytes()
                    .get(cursor)
                    .is_some_and(|byte| byte.is_ascii_whitespace())
                {
                    cursor += 1;
                }
                let value_start = cursor;
                if lower.as_bytes().get(cursor) == Some(&b'-') {
                    cursor += 1;
                }
                while lower
                    .as_bytes()
                    .get(cursor)
                    .is_some_and(|byte| byte.is_ascii_digit())
                {
                    cursor += 1;
                }
                if lower.as_bytes().get(cursor) == Some(&b'/') {
                    cursor += 1;
                    while lower
                        .as_bytes()
                        .get(cursor)
                        .is_some_and(|byte| byte.is_ascii_digit())
                    {
                        cursor += 1;
                    }
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
        let candidates = std::iter::once(record.formula_id.as_str())
            .chain(record.aliases.iter().map(String::as_str));
        if candidates
            .into_iter()
            .any(|candidate| lower.contains(&normalize_phrase(candidate)))
        {
            if !matches
                .iter()
                .any(|existing: &&FormulaRecord| existing.formula_id == record.formula_id)
            {
                matches.push(record);
            }
        }
    }
    matches
}

/// Lower a technical report into a source-derived formula request.
pub fn formalize_source_formula_text(
    text: &str,
    domain: &str,
    records: &[FormulaRecord],
) -> SourceFormulaFrontendResult {
    let lower = text.to_ascii_lowercase().replace(['_', '-'], " ");
    let base_spans = vec![format!("source-formula-text:0..{}", text.len())];
    if ["asymptotic", "infinite", "approximate", "continuous"]
        .iter()
        .any(|marker| lower.contains(marker))
    {
        return output(
            FrontendStatus::Unsupported,
            None,
            None,
            base_spans,
            Vec::new(),
            vec!["request is outside the finite source-formula boundary".into()],
        );
    }
    let matches = matching_records(text, records);
    if matches.is_empty() {
        return output(
            FrontendStatus::Missing,
            None,
            None,
            base_spans,
            Vec::new(),
            vec!["no unique source formula alias was stated".into()],
        );
    }
    if matches.len() > 1 || (matches.len() == 1 && lower.contains(" or ")) {
        return output(
            FrontendStatus::Ambiguous,
            None,
            None,
            base_spans,
            matches
                .iter()
                .map(|record| record.formula_id.clone())
                .collect(),
            vec!["multiple formula interpretations remain".into()],
        );
    }
    let record = matches[0];
    let mut inputs = BTreeMap::new();
    let mut spans = base_spans;
    if record.required_inputs == ["sum", "count"] {
        if let Some((sum, count, list_spans)) = finite_list_inputs(text) {
            inputs.insert("sum".into(), sum);
            inputs.insert("count".into(), count);
            spans.extend(list_spans);
        }
    }
    for input in &record.required_inputs {
        if inputs.contains_key(input) {
            continue;
        }
        if let Some(binding) = record.input_bindings.get(input) {
            if let Some(value) = source_binding_value(text, binding) {
                spans.push(format!("source-binding:{input}={binding}"));
                inputs.insert(input.clone(), value);
                continue;
            }
        }
        let values = labeled_values(text, input);
        if values.len() != 1 {
            return output(
                FrontendStatus::Missing,
                Some(record.formula_id.clone()),
                None,
                spans,
                Vec::new(),
                vec![format!("required input {input} is missing or duplicated")],
            );
        }
        let (span, value) = values.into_iter().next().unwrap();
        spans.push(span);
        inputs.insert(input.clone(), value);
    }
    let request = FormulaRequest {
        formula: record.formula_id.clone(),
        inputs,
        domain: domain.into(),
        ambiguity: None,
        provenance: vec![
            format!("formula-id:{}", record.formula_id),
            format!("source:{}", record.source.source_id),
            format!("source-span:{}", record.source.evidence_span),
        ],
    };
    output(
        FrontendStatus::Complete,
        Some(record.formula_id.clone()),
        Some(request),
        spans,
        Vec::new(),
        Vec::new(),
    )
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
    let target_markers = [
        "calculate",
        "compute",
        "evaluate",
        "find",
        "determine",
        "apply",
        "use",
    ];
    let context_markers = [
        "define",
        "given",
        "where",
        "assume",
        "reference",
        "according",
    ];
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
pub fn formalize_formula_text(
    text: &str,
    domain: &str,
    records: &[FormulaRecord],
) -> FormulaFrontendResult {
    formalize_source_formula_text(text, domain, records)
}

impl SourceFormulaFrontendResult {
    pub fn replay_verified(&self) -> bool {
        replay_verified(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_formula_pack::{extract_formula_records, source_formula_records};
    use crate::source_regression_pack;
    use crate::source_statistics_pack::{records, DOMAIN};

    #[test]
    fn generic_frontend_binds_source_record_without_domain_branch() {
        let result = formalize_source_formula_text(
            "Use the sample mean: sum=30 and count=5.",
            DOMAIN,
            &records(),
        );
        assert_eq!(result.status, FrontendStatus::Complete);
        assert_eq!(result.formula_id.as_deref(), Some("arithmetic_mean"));
        assert!(replay_verified(&result));
    }

    #[test]
    fn generic_frontend_refuses_ambiguity_and_missing_inputs() {
        let records = records();
        let ambiguous = formalize_source_formula_text(
            "Use the mean or weighted average: sum=30 count=5.",
            DOMAIN,
            &records,
        );
        assert_eq!(ambiguous.status, FrontendStatus::Ambiguous);
        let missing =
            formalize_source_formula_text("Use the sample mean: sum=30.", DOMAIN, &records);
        assert_eq!(missing.status, FrontendStatus::Missing);
    }

    #[test]
    fn generic_frontend_binds_one_explicit_finite_numeric_list() {
        let result = formalize_source_formula_text(
            "What is the arithmetic mean of 13, 22 and 37?",
            DOMAIN,
            &records(),
        );
        assert_eq!(result.status, FrontendStatus::Complete);
        let request = result.request.as_ref().expect("list binds a request");
        assert_eq!(request.inputs["sum"], Rational::new(72, 1).unwrap());
        assert_eq!(request.inputs["count"], Rational::new(3, 1).unwrap());
        assert!(replay_verified(&result));
    }

    #[test]
    fn generic_frontend_does_not_turn_mean_equality_into_a_list() {
        let result = formalize_source_formula_text(
            "The arithmetic mean of 5, 8 and 17 equals the mean of 12 and y.",
            DOMAIN,
            &records(),
        );
        assert_eq!(result.status, FrontendStatus::Missing);
        assert!(result.request.is_none());
        assert!(replay_verified(&result));
    }

    #[test]
    fn generic_frontend_rejects_filters_ranges_variables_and_expressions() {
        let near_misses = [
            "Find the arithmetic mean of the prime numbers in this list: 11, 13, 17, 19, 23.",
            "What is the arithmetic mean of the integers from -4 through 5, inclusive?",
            "Given that 10 is the arithmetic mean of the set {6, 13, 18, 4, x}, find x.",
            "The arithmetic mean of nine numbers is 54. If two numbers u and v are added, find their mean.",
            "The arithmetic mean of five expressions is 24: x + 8, 15, 2x, 13, and 2x + 4.",
        ];
        for prompt in near_misses {
            let result = formalize_source_formula_text(prompt, DOMAIN, &records());
            assert_ne!(result.status, FrontendStatus::Complete, "{prompt}");
            assert!(replay_verified(&result), "{prompt}");
        }
    }

    #[test]
    fn generic_frontend_accepts_only_an_explicit_declared_numeric_list() {
        let result = formalize_source_formula_text(
            "A learner's four quiz scores are 81, 87, 94 and 98. What is the arithmetic mean of these four scores?",
            DOMAIN,
            &records(),
        );
        assert_eq!(result.status, FrontendStatus::Complete);
        let request = result.request.as_ref().expect("declared list binds");
        assert_eq!(request.inputs["sum"], Rational::new(360, 1).unwrap());
        assert_eq!(request.inputs["count"], Rational::new(4, 1).unwrap());
        assert!(replay_verified(&result));
    }

    #[test]
    fn generic_frontend_uses_source_declared_sequence_bindings() {
        let records = source_formula_records();
        let result = formalize_source_formula_text(
            "What is the 100th term of the arithmetic sequence 7, 11, 15, 19, ...?",
            "goal6_source_selected_arithmeticsequence",
            &records,
        );
        assert_eq!(result.status, FrontendStatus::Complete);
        let request = result.request.as_ref().expect("source bindings complete");
        assert_eq!(request.inputs["a1"], Rational::new(7, 1).unwrap());
        assert_eq!(request.inputs["n"], Rational::new(100, 1).unwrap());
        assert_eq!(request.inputs["d"], Rational::new(4, 1).unwrap());
        assert!(replay_verified(&result));
    }

    #[test]
    fn generic_sequence_binding_rejects_sum_and_nonconstant_lists() {
        let records = source_formula_records();
        for prompt in [
            "What is the sum of the first 5 terms of the arithmetic sequence 7, 11, 15, 19, ...?",
            "What is the 100th term of the arithmetic sequence 7, 11, 16, 19, ...?",
        ] {
            let result = formalize_source_formula_text(
                prompt,
                "goal6_source_selected_arithmeticsequence",
                &records,
            );
            assert_ne!(result.status, FrontendStatus::Complete, "{prompt}");
            assert!(replay_verified(&result));
        }
    }

    #[test]
    fn compatibility_api_preserves_replayable_result_shape() {
        let source = "BEGIN FORMULA ratio\nALIASES: quotient\nEXPRESSION: a / b\nINPUTS: a, b\nASSUMPTIONS: b positive\nCONSTRAINTS: positive:a; positive:b\nSOURCE_ID: test\nTITLE: Test\nSECTION: Test\nURL: https://example.invalid/test\nLICENSE: test\nRETRIEVED: 2026-08-16\nEVIDENCE: ratio definition\nEND FORMULA";
        let records = extract_formula_records(source).unwrap();
        let result =
            formalize_formula_text("Compute the quotient with a=6 and b=2.", "test", &records);
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
        assert!(result
            .regions
            .iter()
            .any(|region| region.role == FormulaRegionRole::Definition));
        assert!(result
            .regions
            .iter()
            .any(|region| region.role == FormulaRegionRole::Target));
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
