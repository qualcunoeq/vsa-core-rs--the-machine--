//! Fail-closed technical-language frontend for finite positional arithmetic.
//!
//! It accepts exactly two explicitly subscripted numerals in one base and an
//! explicit target base.  Mixed-base expressions, digit statistics, fractions,
//! unknown bases, and more than two operands remain outside this bridge.

use crate::source_base_arithmetic_pack::{
    evaluate, BaseArithmeticOperation, BaseArithmeticRequest, BaseArithmeticResult,
    BaseArithmeticStatus, DOMAIN, MAX_BASE, MIN_BASE,
};
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
pub struct FrontendResult {
    pub status: FrontendStatus,
    pub request: Option<BaseArithmeticRequest>,
    pub operation: Option<BaseArithmeticOperation>,
    pub source_spans: Vec<String>,
    pub alternatives: Vec<String>,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn payload(result: &FrontendResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.request,
        result.operation,
        &result.source_spans,
        &result.alternatives,
        &result.reasons,
    )
}

fn output(
    status: FrontendStatus,
    request: Option<BaseArithmeticRequest>,
    operation: Option<BaseArithmeticOperation>,
    source_spans: Vec<String>,
    alternatives: Vec<String>,
    reasons: Vec<String>,
) -> FrontendResult {
    let mut result = FrontendResult {
        status,
        request,
        operation,
        source_spans,
        alternatives,
        reasons,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn normalize(text: &str) -> String {
    text.replace("\\rm", "")
        .replace(['$', '{', '}', '\\'], "")
        .replace("_ ", "_")
}

fn digit_value(digit: char) -> Option<u32> {
    match digit.to_ascii_uppercase() {
        value @ '0'..='9' => Some(value as u32 - '0' as u32),
        value @ 'A'..='Z' => Some(value as u32 - 'A' as u32 + 10),
        _ => None,
    }
}

fn base_word(value: &str) -> Option<u32> {
    match value.trim_matches(|c: char| !c.is_ascii_alphabetic()) {
        "two" => Some(2),
        "three" => Some(3),
        "four" => Some(4),
        "five" => Some(5),
        "six" => Some(6),
        "seven" => Some(7),
        "eight" => Some(8),
        "nine" => Some(9),
        "ten" => Some(10),
        "eleven" => Some(11),
        "twelve" => Some(12),
        "thirteen" => Some(13),
        "fourteen" => Some(14),
        "fifteen" => Some(15),
        "sixteen" => Some(16),
        _ => None,
    }
}

fn parse_base_after(text: &str, start: usize) -> Option<(u32, usize)> {
    let bytes = text.as_bytes();
    let mut cursor = start;
    while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) || bytes.get(cursor) == Some(&b'-')
    {
        cursor += 1;
    }
    let begin = cursor;
    while bytes.get(cursor).is_some_and(u8::is_ascii_alphanumeric) {
        cursor += 1;
    }
    if begin == cursor {
        return None;
    }
    let token = &text[begin..cursor];
    let base = token.parse().ok().or_else(|| base_word(token))?;
    Some((base, cursor))
}

fn target_bases(text: &str) -> Vec<u32> {
    let lower = text.to_ascii_lowercase();
    let explicit_target_marker = [
        "express",
        "answer",
        "convert",
        "represented",
        "equivalent",
        "written",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    let mut result = Vec::new();
    let mut offset = 0;
    while let Some(relative) = lower[offset..].find("base") {
        let position = offset + relative;
        let Some((base, end)) = parse_base_after(&lower, position + 4) else {
            offset = position + 4;
            continue;
        };
        let context_start = position.saturating_sub(32);
        let context = &lower[context_start..position];
        let target_context = explicit_target_marker
            || [
                "express",
                "answer",
                "convert",
                "represented",
                "equivalent",
                "written",
                "in ",
            ]
            .iter()
            .any(|marker| context.contains(marker));
        if target_context {
            result.push(base);
        }
        offset = end.max(position + 4);
    }
    result.sort_unstable();
    result.dedup();
    result
}

fn source_literals(text: &str) -> Vec<(String, u32, String)> {
    let bytes = text.as_bytes();
    let mut output = Vec::new();
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
        let numeral = text[left..index].to_ascii_uppercase();
        let Ok(base) = text[index + 1..right].parse::<u32>() else {
            continue;
        };
        if !numeral.is_empty()
            && numeral
                .chars()
                .all(|digit| digit_value(digit).is_some_and(|value| value < base))
        {
            output.push((numeral, base, format!("base-arithmetic:{left}..{right}")));
        }
    }
    output
}

fn detect_operation(text: &str) -> Result<BaseArithmeticOperation, Vec<String>> {
    let lower = text.to_ascii_lowercase();
    let mut operations = Vec::new();
    if lower.contains('+')
        || lower.starts_with("add ")
        || lower.contains(" add ")
        || lower.contains("sum of")
        || lower.contains(" plus ")
    {
        operations.push(BaseArithmeticOperation::Add);
    }
    if lower.contains('−')
        || lower.contains(" - ")
        || lower.starts_with("subtract ")
        || lower.contains(" subtract ")
        || lower.contains("difference")
        || lower.contains(" minus ")
    {
        operations.push(BaseArithmeticOperation::Subtract);
    }
    if lower.contains('*')
        || lower.contains("cdot")
        || lower.contains("times")
        || lower.starts_with("multiply ")
        || lower.contains(" product ")
        || lower.contains("multiply")
    {
        operations.push(BaseArithmeticOperation::Multiply);
    }
    operations.sort_by_key(|operation| format!("{operation:?}"));
    operations.dedup();
    match operations.as_slice() {
        [operation] => Ok(*operation),
        [] => Err(vec![
            "one supported binary arithmetic operation is required".into(),
        ]),
        _ => Err(vec![
            "multiple positional arithmetic operations remain plausible".into(),
        ]),
    }
}

pub fn formalize(text: &str, case_id: &str) -> FrontendResult {
    let normalized = normalize(text);
    let lower = normalized.to_ascii_lowercase();
    let provenance = vec![format!("source-base-arithmetic-frontend:{case_id}")];
    if lower.contains("fraction")
        || lower.contains("decimal point")
        || lower.contains("digit")
        || lower.contains("palindrome")
        || lower.contains("largest base")
        || lower.contains("smallest base")
    {
        return output(
            FrontendStatus::Unsupported,
            None,
            None,
            provenance,
            Vec::new(),
            vec!["fractional, digit-statistic, palindrome, or unknown-base target is outside the binary arithmetic scope".into()],
        );
    }
    let literals = source_literals(&normalized);
    if literals.len() != 2 {
        return output(
            if literals.is_empty() {
                FrontendStatus::Missing
            } else {
                FrontendStatus::Unsupported
            },
            None,
            None,
            provenance,
            Vec::new(),
            vec!["exactly two explicit positional numerals are required".into()],
        );
    }
    let operation = match detect_operation(&normalized) {
        Ok(operation) => operation,
        Err(reasons) => {
            return output(
                if reasons.len() > 1 {
                    FrontendStatus::Ambiguous
                } else {
                    FrontendStatus::Missing
                },
                None,
                None,
                provenance,
                Vec::new(),
                reasons,
            )
        }
    };
    if literals[0].1 != literals[1].1 {
        return output(
            FrontendStatus::Unsupported,
            Some(BaseArithmeticRequest {
                left: literals[0].0.clone(),
                right: literals[1].0.clone(),
                base: literals[0].1,
                target_base: literals[0].1,
                operation,
                domain: DOMAIN.into(),
                ambiguity: Some("operands have different declared bases".into()),
                provenance: provenance.clone(),
            }),
            Some(operation),
            literals.iter().map(|literal| literal.2.clone()).collect(),
            Vec::new(),
            vec!["binary arithmetic requires both operands in the same base".into()],
        );
    }
    if !(MIN_BASE..=MAX_BASE).contains(&literals[0].1) {
        return output(
            FrontendStatus::Unsupported,
            None,
            Some(operation),
            literals.iter().map(|literal| literal.2.clone()).collect(),
            Vec::new(),
            vec![format!(
                "operands must use a base between {MIN_BASE} and {MAX_BASE}"
            )],
        );
    }
    let targets = target_bases(&normalized);
    match targets.as_slice() {
        [] => output(
            FrontendStatus::Missing,
            None,
            Some(operation),
            literals.iter().map(|literal| literal.2.clone()).collect(),
            Vec::new(),
            vec!["one explicit target base is required".into()],
        ),
        [target_base] => {
            if !(MIN_BASE..=MAX_BASE).contains(target_base) {
                return output(
                    FrontendStatus::Unsupported,
                    None,
                    Some(operation),
                    literals.iter().map(|literal| literal.2.clone()).collect(),
                    vec![target_base.to_string()],
                    vec![format!(
                        "target base must be between {MIN_BASE} and {MAX_BASE}"
                    )],
                );
            }
            if *target_base != literals[0].1 {
                return output(
                    FrontendStatus::Unsupported,
                    None,
                    Some(operation),
                    literals.iter().map(|literal| literal.2.clone()).collect(),
                    vec![target_base.to_string()],
                    vec![
                        "cross-base output requires the separate validated conversion bridge"
                            .into(),
                    ],
                );
            }
            let request = BaseArithmeticRequest {
                left: literals[0].0.clone(),
                right: literals[1].0.clone(),
                base: literals[0].1,
                target_base: *target_base,
                operation,
                domain: DOMAIN.into(),
                ambiguity: None,
                provenance: provenance.clone(),
            };
            output(
                FrontendStatus::Complete,
                Some(request),
                Some(operation),
                literals.iter().map(|literal| literal.2.clone()).collect(),
                Vec::new(),
                Vec::new(),
            )
        }
        alternatives => output(
            FrontendStatus::Ambiguous,
            None,
            Some(operation),
            literals.iter().map(|literal| literal.2.clone()).collect(),
            alternatives.iter().map(|base| base.to_string()).collect(),
            vec!["several target bases are stated".into()],
        ),
    }
}

pub fn replay_verified(result: &FrontendResult) -> bool {
    result.replay_hash == digest(&payload(result))
        && !result.source_spans.is_empty()
        && (result.status != FrontendStatus::Complete || result.request.is_some())
}

pub fn downstream_replay(result: &FrontendResult) -> bool {
    let Some(request) = result.request.as_ref() else {
        return result.status != FrontendStatus::Complete;
    };
    let evaluated: BaseArithmeticResult = evaluate(request);
    replay_verified(result)
        && replay_verified_result(&evaluated)
        && (result.status != FrontendStatus::Complete
            || evaluated.status == BaseArithmeticStatus::Complete)
}

fn replay_verified_result(result: &BaseArithmeticResult) -> bool {
    crate::source_base_arithmetic_pack::replay_verified(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_explicit_base_arithmetic_and_replays() {
        let parsed = formalize(
            "Find the product of 1011_2 and 101_2. Express your answer in base 2.",
            "test",
        );
        assert_eq!(parsed.status, FrontendStatus::Complete);
        assert_eq!(parsed.operation, Some(BaseArithmeticOperation::Multiply));
        assert!(downstream_replay(&parsed));
    }

    #[test]
    fn rejects_derived_digit_and_mixed_base_questions() {
        let digits = formalize(
            "Find the number of even digits in the base-7 representation of 403_10.",
            "digits",
        );
        assert_eq!(digits.status, FrontendStatus::Unsupported);
        let mixed = formalize("Compute 43210_6 - 3210_7 in base 10.", "mixed");
        assert_eq!(mixed.status, FrontendStatus::Unsupported);
        assert!(replay_verified(&digits));
        assert!(replay_verified(&mixed));
    }

    #[test]
    fn preserves_missing_target_and_tamper() {
        let missing = formalize("Add 101_2 and 11_2.", "missing");
        assert_eq!(missing.status, FrontendStatus::Missing);
        let complete = formalize("Add 101_2 and 11_2; express in base 10.", "tamper");
        let mut tampered = complete.clone();
        tampered.replay_hash.push('x');
        assert!(!replay_verified(&tampered));
    }
}
