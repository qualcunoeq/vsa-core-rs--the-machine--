//! Bounded frontend for source-derived finite-mean updates.
//!
//! The accepted form contains one explicitly enumerated old finite list and
//! one explicitly stated added value.  It does not infer grouped weights,
//! graph values, rates, or target means from incidental numbers.

use crate::probability_pack::Rational;
use crate::source_formula_frontend::{formalize_source_formula_text, SourceFormulaFrontendResult};
use crate::source_formula_pack::{extract_formula_records, FormulaRecord, FormulaRequest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub use crate::source_formula_frontend::FrontendStatus;

pub const DOMAIN: &str = "source_derived_mean_update";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MeanUpdateResult {
    pub frontend: SourceFormulaFrontendResult,
    pub old_sum: Option<Rational>,
    pub old_count: Option<Rational>,
    pub added_value: Option<Rational>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
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

fn parse_numeric_list(segment: &str) -> Option<Vec<Rational>> {
    let chars: Vec<char> = segment.chars().collect();
    let mut values = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        while chars.get(index).is_some_and(|character| {
            character.is_ascii_whitespace() || matches!(character, ',' | ';' | '$')
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
        if chars.get(index) == Some(&'/') {
            index += 1;
            let denominator_start = index;
            while chars.get(index).is_some_and(char::is_ascii_digit) {
                index += 1;
            }
            if index == denominator_start {
                return None;
            }
        }
        if chars
            .get(index)
            .is_some_and(|character| character.is_ascii_alphanumeric() || *character == '.')
        {
            return None;
        }
        values.push(rational_token(
            &chars[start..index].iter().collect::<String>(),
        )?);
    }
    (2..=20).contains(&values.len()).then_some(values)
}

fn list_values(text: &str) -> Option<(Vec<Rational>, String)> {
    let lower = text.to_ascii_lowercase();
    let marker = [
        "scores were",
        "scores of",
        "values were",
        "values of",
        "observations were",
        "observations of",
        "measurements were",
        "measurements of",
    ]
    .iter()
    .filter_map(|marker| lower.find(marker).map(|start| (start, *marker)))
    .min_by_key(|(start, _)| *start)?;
    let start = marker.0 + marker.1.len();
    let punctuation_end = text[start..]
        .find(|character: char| matches!(character, '.' | '?' | ';'))
        .map_or(text.len(), |offset| start + offset);
    let contextual_end = if marker.1.ends_with(" of") {
        [" on ", " in ", " during ", " were ", " was "]
            .iter()
            .filter_map(|separator| lower[start..punctuation_end].find(separator))
            .map(|offset| start + offset)
            .min()
            .unwrap_or(punctuation_end)
    } else {
        punctuation_end
    };
    let end = contextual_end;
    let values = parse_numeric_list(&text[start..end])?;
    Some((values, format!("old-list-span:{start}..{end}")))
}

fn added_value(text: &str) -> Option<(Rational, String)> {
    let lower = text.to_ascii_lowercase();
    let marker = [
        "after a score of",
        "after a value of",
        "after one score of",
        "receives a score of",
        "gets a score of",
        "earns a score of",
        "after receiving a score of",
    ]
    .iter()
    .filter_map(|marker| lower.find(marker).map(|start| (start, *marker)))
    .min_by_key(|(start, _)| *start)?;
    let mut start = marker.0 + marker.1.len();
    start += lower[start..]
        .find(|character: char| !character.is_ascii_whitespace())
        .unwrap_or(lower.len().saturating_sub(start));
    let token_end = lower[start..]
        .find(|character: char| {
            character.is_ascii_whitespace() || matches!(character, '.' | '?' | ',')
        })
        .map_or(lower.len(), |offset| start + offset);
    let value = rational_token(&lower[start..token_end])?;
    Some((value, format!("added-value-span:{start}..{token_end}")))
}

fn fallback(
    text: &str,
    status: FrontendStatus,
    spans: Vec<String>,
    reason: &str,
) -> SourceFormulaFrontendResult {
    let mut frontend = formalize_source_formula_text(text, DOMAIN, &[]);
    frontend.status = status;
    frontend.formula_id = None;
    frontend.formula = None;
    frontend.request = None;
    frontend.provenance_spans = spans;
    frontend.alternatives.clear();
    frontend.reasons = vec![reason.into()];
    frontend.replay_hash = digest(&(
        &frontend.status,
        &frontend.formula_id,
        &frontend.formula,
        &frontend.request,
        &frontend.provenance_spans,
        &frontend.alternatives,
        &frontend.reasons,
    ));
    frontend
}

fn output(
    frontend: SourceFormulaFrontendResult,
    old_sum: Option<Rational>,
    old_count: Option<Rational>,
    added_value: Option<Rational>,
    provenance: Vec<String>,
) -> MeanUpdateResult {
    let mut result = MeanUpdateResult {
        frontend,
        old_sum,
        old_count,
        added_value,
        provenance,
        replay_hash: String::new(),
    };
    result.replay_hash = digest(&(
        &result.frontend,
        &result.old_sum,
        &result.old_count,
        &result.added_value,
        &result.provenance,
    ));
    result
}

pub fn records() -> Vec<FormulaRecord> {
    extract_formula_records(include_str!(
        "../docs/sources/openstax_mean_update_source.txt"
    ))
    .expect("mean update source extracts")
}

pub fn formalize_mean_update_text(text: &str) -> MeanUpdateResult {
    let lower = text.to_ascii_lowercase();
    let provenance = vec![format!("mean-update-text:0..{}", text.len())];
    if !(lower.contains("average increase") || lower.contains("mean increase")) {
        return output(
            fallback(
                text,
                FrontendStatus::Missing,
                provenance.clone(),
                "no mean-increase target was stated",
            ),
            None,
            None,
            None,
            provenance,
        );
    }
    if [
        "weighted",
        "graph",
        "table",
        "rate",
        "average speed",
        "days",
        "hours",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        return output(
            fallback(text, FrontendStatus::Unsupported, provenance.clone(), "grouped, graphical, weighted, or rate semantics are outside the one-added-value boundary"),
            None,
            None,
            None,
            provenance,
        );
    }
    let Some((values, list_span)) = list_values(text) else {
        return output(
            fallback(
                text,
                FrontendStatus::Ambiguous,
                provenance.clone(),
                "one explicit old finite list is required",
            ),
            None,
            None,
            None,
            provenance,
        );
    };
    let Some((added, added_span)) = added_value(text) else {
        return output(
            fallback(
                text,
                FrontendStatus::Ambiguous,
                vec![provenance[0].clone(), list_span],
                "one explicitly added value is required",
            ),
            None,
            None,
            None,
            provenance,
        );
    };
    let old_sum = values
        .iter()
        .try_fold(Rational::zero(), |sum, value| sum.add(value))
        .expect("finite list sum remains exact");
    let old_count = Rational::new(values.len() as i128, 1).expect("nonempty old list");
    let record = &records()[0];
    let request = FormulaRequest {
        formula: record.formula_id.clone(),
        inputs: BTreeMap::from([
            ("old_sum".into(), old_sum.clone()),
            ("old_count".into(), old_count.clone()),
            ("added_value".into(), added.clone()),
        ]),
        domain: DOMAIN.into(),
        ambiguity: None,
        provenance: vec![
            format!("formula-id:{}", record.formula_id),
            format!("source:{}", record.source.source_id),
            format!("source-span:{}", record.source.evidence_span),
        ],
    };
    let mut frontend = formalize_source_formula_text(text, DOMAIN, &[]);
    frontend.status = FrontendStatus::Complete;
    frontend.formula_id = Some(record.formula_id.clone());
    frontend.formula = Some(record.formula_id.clone());
    frontend.request = Some(request);
    frontend.provenance_spans = vec![provenance[0].clone(), list_span, added_span];
    frontend.alternatives.clear();
    frontend.reasons.clear();
    frontend.replay_hash = digest(&(
        &frontend.status,
        &frontend.formula_id,
        &frontend.formula,
        &frontend.request,
        &frontend.provenance_spans,
        &frontend.alternatives,
        &frontend.reasons,
    ));
    output(
        frontend,
        Some(old_sum),
        Some(old_count),
        Some(added),
        provenance,
    )
}

pub fn replay_verified(result: &MeanUpdateResult) -> bool {
    result.replay_hash
        == digest(&(
            &result.frontend,
            &result.old_sum,
            &result.old_count,
            &result.added_value,
            &result.provenance,
        ))
        && result.frontend.replay_verified()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binds_one_added_value_and_replays() {
        let result = formalize_mean_update_text(
            "Scores were 87, 83, and 88. By how much will the average increase after a score of 90?",
        );
        assert_eq!(result.frontend.status, FrontendStatus::Complete);
        assert_eq!(result.old_sum, Some(Rational::new(258, 1).unwrap()));
        assert_eq!(result.old_count, Some(Rational::new(3, 1).unwrap()));
        assert_eq!(result.added_value, Some(Rational::new(90, 1).unwrap()));
        assert!(replay_verified(&result));
    }

    #[test]
    fn rejects_grouped_or_missing_context() {
        for text in [
            "The average increase after a score of 90 is requested.",
            "Scores were shown in a graph. By how much will the average increase after a score of 90?",
            "The average increase after five days at 80 and three days at 90 is requested.",
        ] {
            let result = formalize_mean_update_text(text);
            assert_ne!(result.frontend.status, FrontendStatus::Complete);
            assert!(replay_verified(&result));
        }
    }
}
