//! Fail-closed natural-language frontend for positional-base conversion.

use crate::source_base_conversion_pack::{numeral_is_valid, BaseConversionRequest, DOMAIN};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BaseConversionFrontendResult {
    pub status: FrontendStatus,
    pub request: Option<BaseConversionRequest>,
    pub source_literal_span: Option<String>,
    pub target_span: Option<String>,
    pub alternatives: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance_spans: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn payload(result: &BaseConversionFrontendResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.request,
        &result.source_literal_span,
        &result.target_span,
        &result.alternatives,
        &result.reasons,
        &result.provenance_spans,
    )
}

fn output(
    status: FrontendStatus,
    request: Option<BaseConversionRequest>,
    source_literal_span: Option<String>,
    target_span: Option<String>,
    alternatives: Vec<String>,
    reasons: Vec<String>,
    provenance_spans: Vec<String>,
) -> BaseConversionFrontendResult {
    let mut result = BaseConversionFrontendResult {
        status,
        request,
        source_literal_span,
        target_span,
        alternatives,
        reasons,
        provenance_spans,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn normalize(text: &str) -> String {
    text.replace("\\rm", "")
        .replace(['$', '{', '}', '\\'], "")
        .replace("_{", "_")
}

fn base_after(text: &str, marker: &str) -> Vec<u32> {
    let lower = text.to_ascii_lowercase();
    let mut values = Vec::new();
    let mut offset = 0;
    while let Some(relative) = lower[offset..].find(marker) {
        let start = offset + relative + marker.len();
        let mut cursor = start;
        let attached_hyphen = lower.as_bytes().get(cursor) == Some(&b'-');
        if attached_hyphen {
            cursor += 1;
        } else {
            while lower
                .as_bytes()
                .get(cursor)
                .is_some_and(u8::is_ascii_whitespace)
            {
                cursor += 1;
            }
        }
        if lower.as_bytes().get(cursor) == Some(&b'_') {
            cursor += 1;
        }
        while lower.as_bytes().get(cursor) == Some(&b' ') {
            cursor += 1;
        }
        let begin = cursor;
        while lower.as_bytes().get(cursor).is_some_and(u8::is_ascii_digit) {
            cursor += 1;
        }
        if cursor > begin {
            if let Ok(value) = lower[begin..cursor].parse::<u32>() {
                values.push(value);
            }
        }
        offset = (start + 1).min(lower.len());
    }
    values
}

fn source_literal(text: &str) -> Option<(String, u32, String)> {
    let normalized = normalize(text);
    let bytes = normalized.as_bytes();
    let mut candidates = Vec::new();
    for index in 0..bytes.len() {
        if bytes[index] != b'_' {
            continue;
        }
        let mut left = index;
        while left > 0 && bytes[left - 1].is_ascii_alphanumeric() {
            left -= 1;
        }
        let mut right = index + 1;
        while right < bytes.len() && bytes[right].is_ascii_digit() {
            right += 1;
        }
        if left == index || right == index + 1 {
            continue;
        }
        let numeral = normalized[left..index].to_ascii_uppercase();
        let base = normalized[index + 1..right].parse::<u32>().ok()?;
        if numeral.chars().all(|c| c.is_ascii_alphanumeric()) {
            candidates.push((numeral, base, format!("{}..{}", left, right)));
        }
    }
    (candidates.len() == 1).then(|| candidates.remove(0))
}

fn source_literal_is_negative(text: &str, span: &str) -> bool {
    let normalized = normalize(text);
    let Some((start, _)) = span
        .split_once("..")
        .and_then(|(start, end)| Some((start.parse::<usize>().ok()?, end.parse::<usize>().ok()?)))
    else {
        return false;
    };
    start > 0
        && normalized
            .as_bytes()
            .get(start - 1)
            .is_some_and(|byte| *byte == b'-')
}

pub fn formalize_base_conversion_text(text: &str, case_id: &str) -> BaseConversionFrontendResult {
    let normalized = normalize(text);
    let lower = normalized.to_ascii_lowercase();
    let provenance = vec![format!("source-base-conversion-frontend:{case_id}")];
    if lower.contains("fraction")
        || lower.contains("decimal point")
        || lower.contains('/')
        || lower.contains("negative")
        || lower.contains("approx")
        || lower.contains("how many more digits")
        || lower.contains("number of zeros")
        || lower.contains("number of ones")
        || lower.contains("number of even digits")
        || lower.contains("number of odd digits")
        || lower.contains("how many even digits")
        || lower.contains("how many odd digits")
        || lower.contains("y-x")
        || lower.contains("first digit")
        || lower.contains("leftmost digit")
    {
        return output(
            FrontendStatus::Unsupported,
            None,
            None,
            None,
            Vec::new(),
            vec!["request is a fractional, approximate, digit-statistic, or derived target".into()],
            provenance,
        );
    }
    let targets = base_after(&lower, "base");
    let mut unique_targets = targets.clone();
    unique_targets.sort_unstable();
    unique_targets.dedup();
    if unique_targets.len() != 1 {
        return output(
            if unique_targets.is_empty() {
                FrontendStatus::Missing
            } else {
                FrontendStatus::Ambiguous
            },
            None,
            None,
            None,
            unique_targets
                .iter()
                .map(|value| value.to_string())
                .collect(),
            vec!["one explicit target base is required".into()],
            provenance,
        );
    }
    let target_base = unique_targets[0];
    let Some((numeral, source_base, span)) = source_literal(&normalized) else {
        return output(
            FrontendStatus::Missing,
            None,
            None,
            Some(format!("base={target_base}")),
            Vec::new(),
            vec!["one explicit source-base numeral is required".into()],
            provenance,
        );
    };
    if source_literal_is_negative(&normalized, &span) {
        return output(
            FrontendStatus::Unsupported,
            None,
            Some(span),
            Some(format!("base={target_base}")),
            Vec::new(),
            vec!["negative numerals are outside the finite nonnegative scope".into()],
            provenance,
        );
    }
    if source_base < 2 || source_base > 36 || target_base < 2 || target_base > 36 {
        return output(
            FrontendStatus::Unsupported,
            None,
            Some(span),
            Some(format!("base={target_base}")),
            Vec::new(),
            vec!["only bases 2 through 36 are supported".into()],
            provenance,
        );
    }
    if !numeral_is_valid(&numeral, source_base) {
        return output(
            FrontendStatus::Unsupported,
            None,
            Some(span),
            Some(format!("base={target_base}")),
            Vec::new(),
            vec!["source numeral contains a digit outside its declared base".into()],
            provenance,
        );
    }
    if !lower.contains("convert") && !lower.contains("express") && !lower.contains("base") {
        return output(
            FrontendStatus::Missing,
            None,
            Some(span),
            Some(format!("base={target_base}")),
            Vec::new(),
            vec!["an explicit conversion target is required".into()],
            provenance,
        );
    }
    let request = BaseConversionRequest {
        numeral: numeral.clone(),
        source_base,
        target_base,
        domain: DOMAIN.into(),
        ambiguity: None,
        provenance: provenance.clone(),
    };
    output(
        FrontendStatus::Complete,
        Some(request),
        Some(span),
        Some(format!("base={target_base}")),
        Vec::new(),
        Vec::new(),
        provenance,
    )
}

pub fn replay_verified(result: &BaseConversionFrontendResult) -> bool {
    result.replay_hash == digest(&payload(result))
        && !result.provenance_spans.is_empty()
        && (result.status != FrontendStatus::Complete || result.request.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_decimal_and_hexadecimal_source_literals() {
        let ternary =
            formalize_base_conversion_text("Convert $10101_3$ to a base 10 integer.", "t");
        assert_eq!(ternary.status, FrontendStatus::Complete);
        assert_eq!(ternary.request.as_ref().unwrap().numeral, "10101");
        let hexadecimal =
            formalize_base_conversion_text("Convert \\rm{A}03_{16} to a base 10 integer.", "h");
        assert_eq!(hexadecimal.status, FrontendStatus::Complete);
        assert_eq!(hexadecimal.request.as_ref().unwrap().source_base, 16);
        assert!(replay_verified(&hexadecimal));
    }

    #[test]
    fn preserves_derived_and_ambiguous_targets() {
        let derived = formalize_base_conversion_text(
            "Convert 199_10 to base 2 and let x be the number of zeros and y the number of ones.",
            "d",
        );
        assert_eq!(derived.status, FrontendStatus::Unsupported);
        let first_digit = formalize_base_conversion_text(
            "What is the first digit of the base 8 representation of 473_10?",
            "first-digit",
        );
        assert_eq!(first_digit.status, FrontendStatus::Unsupported);
        let digit_count = formalize_base_conversion_text(
            "Find the number of even digits in the base-7 representation of 403_10.",
            "digit-count",
        );
        assert_eq!(digit_count.status, FrontendStatus::Unsupported);
        let ambiguous = formalize_base_conversion_text("Convert 101_2 to base 8 or base 10.", "a");
        assert_eq!(ambiguous.status, FrontendStatus::Ambiguous);
        assert!(replay_verified(&derived));
        assert!(replay_verified(&ambiguous));
    }

    #[test]
    fn does_not_treat_a_hyphenated_base_label_as_a_negative_numeral() {
        let result = formalize_base_conversion_text(
            "Convert 101_2 to a base-10 integer.",
            "hyphenated-base",
        );
        assert_eq!(result.status, FrontendStatus::Complete);
        assert_eq!(result.request.as_ref().unwrap().target_base, 10);
    }
}
