//! Bounded frontend for a source-derived finite arithmetic-progression mean.
//!
//! It accepts explicit endpoints or a declared bounded multiple interval and
//! emits a generic source formula request. It does not solve question-specific
//! patterns or infer an unbounded progression.

use crate::probability_pack::Rational;
use crate::source_formula_frontend::{formalize_source_formula_text, SourceFormulaFrontendResult};
use crate::source_formula_pack::{extract_formula_records, FormulaRecord, FormulaRequest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub use crate::source_formula_frontend::FrontendStatus;

pub const DOMAIN: &str = "source_derived_arithmetic_progression_mean";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProgressionMeanResult {
    pub frontend: SourceFormulaFrontendResult,
    pub first: Option<Rational>,
    pub last: Option<Rational>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn parse_rational(token: &str) -> Option<Rational> {
    let token = token.trim_matches(|character: char| {
        !character.is_ascii_digit() && character != '-' && character != '/'
    });
    if let Some((numerator, denominator)) = token.split_once('/') {
        return Rational::new(numerator.parse().ok()?, denominator.parse().ok()?);
    }
    Rational::new(token.parse().ok()?, 1)
}

fn labeled_value(text: &str, labels: &[&str]) -> Option<Rational> {
    let lower = text.to_ascii_lowercase().replace(['_', '-'], " ");
    let chars: Vec<char> = lower.chars().collect();
    for label in labels {
        let mut search_start = 0;
        while let Some(relative) = lower[search_start..].find(label) {
            let start = search_start + relative;
            let before_ok = start == 0
                || !chars
                    .get(start.saturating_sub(1))
                    .is_some_and(|character| character.is_ascii_alphabetic());
            let after = start + label.len();
            let after_ok = after >= lower.len()
                || !chars
                    .get(after)
                    .is_some_and(|character| character.is_ascii_alphabetic());
            if !before_ok || !after_ok {
                search_start = after;
                continue;
            }
            let mut cursor = after;
            while lower[cursor..]
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_whitespace() || character == ':')
            {
                cursor += lower[cursor..].chars().next().unwrap().len_utf8();
            }
            if lower[cursor..].starts_with("term") {
                cursor += "term".len();
                while lower[cursor..]
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_whitespace())
                {
                    cursor += lower[cursor..].chars().next().unwrap().len_utf8();
                }
            }
            if lower[cursor..].starts_with('=') {
                cursor += 1;
            } else if lower[cursor..].starts_with("is")
                && lower[cursor + 2..]
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_whitespace())
            {
                cursor += 2;
            }
            while lower[cursor..]
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_whitespace())
            {
                cursor += lower[cursor..].chars().next().unwrap().len_utf8();
            }
            let token_end = lower[cursor..]
                .find(|character: char| {
                    character.is_ascii_whitespace() || matches!(character, ',' | ';' | '.')
                })
                .map_or(lower.len(), |offset| cursor + offset);
            if cursor < token_end {
                if let Some(value) = parse_rational(&lower[cursor..token_end]) {
                    return Some(value);
                }
            }
            search_start = after;
        }
    }
    None
}

fn ceil_div_positive(numerator: i128, denominator: i128) -> Option<i128> {
    (denominator > 0).then(|| {
        numerator.div_euclid(denominator) + i128::from(numerator.rem_euclid(denominator) != 0)
    })
}

fn floor_div_positive(numerator: i128, denominator: i128) -> Option<i128> {
    (denominator > 0).then(|| numerator.div_euclid(denominator))
}

fn bounded_multiple_endpoints(text: &str) -> Option<(Rational, Rational, String)> {
    let lower = text.to_ascii_lowercase();
    if !lower.contains("multiple") {
        return None;
    }
    let step_start = lower.find("multiples of")? + "multiples of".len();
    let step_token = lower[step_start..].split_whitespace().next()?;
    let step = parse_rational(step_token)?;
    if step.denominator != 1 || step.numerator <= 0 {
        return None;
    }
    let (low, high, span) = if lower.contains("two-digit") || lower.contains("two digit") {
        (10_i128, 99_i128, "two-digit-bound")
    } else {
        let from = lower.find("from")? + "from".len();
        let through = lower[from..]
            .find("through")
            .or_else(|| lower[from..].find("to"))?;
        let through_start = from + through;
        let low = lower[from..through_start].trim().parse::<i128>().ok()?;
        let marker = if lower[through_start..].starts_with("through") {
            "through"
        } else {
            "to"
        };
        let high_start = through_start + marker.len();
        let high = lower[high_start..]
            .split(|character: char| !character.is_ascii_digit() && character != '-')
            .find(|token| !token.is_empty())?
            .parse::<i128>()
            .ok()?;
        (low, high, "explicit-bound")
    };
    if low > high {
        return None;
    }
    let first_multiple = ceil_div_positive(low, step.numerator)? * step.numerator;
    let last_multiple = floor_div_positive(high, step.numerator)? * step.numerator;
    (first_multiple <= last_multiple).then(|| {
        (
            Rational::new(first_multiple, 1).unwrap(),
            Rational::new(last_multiple, 1).unwrap(),
            format!(
                "multiple-endpoints:{span}:{low}..{high}:step={}",
                step.numerator
            ),
        )
    })
}

fn output(
    frontend: SourceFormulaFrontendResult,
    first: Option<Rational>,
    last: Option<Rational>,
    provenance: Vec<String>,
) -> ProgressionMeanResult {
    let mut result = ProgressionMeanResult {
        frontend,
        first,
        last,
        provenance,
        replay_hash: String::new(),
    };
    result.replay_hash = digest(&(
        &result.frontend,
        &result.first,
        &result.last,
        &result.provenance,
    ));
    result
}

fn fallback_frontend(
    text: &str,
    status: FrontendStatus,
    evidence: &[String],
    alternatives: Vec<String>,
    reason: &str,
) -> SourceFormulaFrontendResult {
    let mut frontend = formalize_source_formula_text(text, DOMAIN, &[]);
    frontend.status = status;
    frontend.formula_id = None;
    frontend.formula = None;
    frontend.request = None;
    frontend.provenance_spans = evidence.to_vec();
    frontend.alternatives = alternatives;
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

pub fn records() -> Vec<FormulaRecord> {
    extract_formula_records(include_str!(
        "../docs/sources/openstax_arithmetic_progression_mean_source.txt"
    ))
    .expect("progression mean source extracts")
}

/// Lower one finite arithmetic-progression mean request.
pub fn formalize_progression_mean_text(text: &str) -> ProgressionMeanResult {
    let lower = text.to_ascii_lowercase();
    let provenance = vec![format!("progression-mean-text:0..{}", text.len())];
    let records = records();
    let progression_markers = [
        "arithmetic progression",
        "arithmetic sequence",
        "common difference",
        "multiples",
    ];
    let has_progression_marker = progression_markers
        .iter()
        .any(|marker| lower.contains(marker));
    if !(lower.contains("mean") || lower.contains("average")) {
        return output(
            fallback_frontend(
                text,
                FrontendStatus::Missing,
                &provenance,
                Vec::new(),
                "no finite arithmetic-progression mean target was stated",
            ),
            None,
            None,
            provenance,
        );
    }
    if [
        "continuous",
        "infinite",
        "weighted",
        "median",
        "variance",
        "standard deviation",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        return output(
            fallback_frontend(
                text,
                FrontendStatus::Unsupported,
                &provenance,
                Vec::new(),
                "request is outside the finite arithmetic-progression mean boundary",
            ),
            None,
            None,
            provenance,
        );
    }
    let mut first = labeled_value(text, &["first"]);
    let mut last = labeled_value(text, &["last"]);
    let mut evidence = provenance.clone();
    if let Some((multiple_first, multiple_last, span)) = bounded_multiple_endpoints(text) {
        if first.is_some() || last.is_some() {
            return output(
                fallback_frontend(
                    text,
                    FrontendStatus::Ambiguous,
                    &evidence,
                    vec![records[0].formula_id.clone()],
                    "multiple endpoint interpretations remain",
                ),
                None,
                None,
                evidence,
            );
        }
        first = Some(multiple_first);
        last = Some(multiple_last);
        evidence.push(span);
    }
    let (Some(first), Some(last)) = (first, last) else {
        let status = if has_progression_marker {
            FrontendStatus::Ambiguous
        } else {
            FrontendStatus::Missing
        };
        return output(
            fallback_frontend(
                text,
                status,
                &evidence,
                vec![records[0].formula_id.clone()],
                "finite progression endpoints are missing or not unique",
            ),
            None,
            None,
            evidence,
        );
    };
    let endpoints_descend = first.numerator * last.denominator > last.numerator * first.denominator;
    if first == last || endpoints_descend || !(lower.contains("mean") || lower.contains("average"))
    {
        return output(
            fallback_frontend(
                text,
                FrontendStatus::Unsupported,
                &evidence,
                Vec::new(),
                "endpoints do not define a supported nonempty increasing progression",
            ),
            None,
            None,
            evidence,
        );
    }
    let source = &records[0].source;
    let request = FormulaRequest {
        formula: records[0].formula_id.clone(),
        inputs: BTreeMap::from([
            ("first".into(), first.clone()),
            ("last".into(), last.clone()),
        ]),
        domain: DOMAIN.into(),
        ambiguity: None,
        provenance: vec![
            format!("formula-id:{}", records[0].formula_id),
            format!("source:{}", source.source_id),
            format!("source-span:{}", source.evidence_span),
        ],
    };
    let mut frontend = formalize_source_formula_text(text, DOMAIN, &[]);
    frontend.status = FrontendStatus::Complete;
    frontend.formula_id = Some(request.formula.clone());
    frontend.formula = Some(request.formula.clone());
    frontend.request = Some(request);
    frontend.provenance_spans = evidence.clone();
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
    output(frontend, Some(first), Some(last), evidence)
}

pub fn replay_verified(result: &ProgressionMeanResult) -> bool {
    result.replay_hash
        == digest(&(
            &result.frontend,
            &result.first,
            &result.last,
            &result.provenance,
        ))
        && result.frontend.replay_verified()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_formula_pack::{evaluate_formula_records, FormulaStatus};

    #[test]
    fn explicit_endpoints_and_multiple_bounds_lower_generic_request() {
        let explicit = formalize_progression_mean_text(
            "Find the arithmetic progression mean with first=3 and last=27.",
        );
        assert_eq!(explicit.frontend.status, FrontendStatus::Complete);
        assert!(replay_verified(&explicit));
        let result = evaluate_formula_records(
            explicit.frontend.request.as_ref().unwrap(),
            DOMAIN,
            &records(),
        );
        assert_eq!(result.status, FormulaStatus::Complete);
        assert_eq!(result.value, Some(Rational::new(15, 1).unwrap()));

        let multiples = formalize_progression_mean_text(
            "What is the arithmetic mean of all positive two-digit multiples of 7?",
        );
        assert_eq!(multiples.frontend.status, FrontendStatus::Complete);
        assert_eq!(multiples.first, Some(Rational::new(14, 1).unwrap()));
        assert_eq!(multiples.last, Some(Rational::new(98, 1).unwrap()));
        assert!(replay_verified(&multiples));
    }

    #[test]
    fn missing_or_invalid_progression_evidence_refuses() {
        for text in [
            "Find the mean of an arithmetic sequence.",
            "Find the mean of multiples of 0 from 1 through 9.",
            "Find the mean of a continuous progression from 1 through 9.",
        ] {
            let result = formalize_progression_mean_text(text);
            assert_ne!(result.frontend.status, FrontendStatus::Complete, "{text}");
            assert!(replay_verified(&result));
        }
    }
}
