//! Source-derived exact positional-numeral conversion.
//!
//! The source record declares the finite positional semantics and its strict
//! boundary.  Execution is a generic base conversion over validated digits;
//! it does not contain benchmark-specific question branches.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const DOMAIN: &str = "source_derived_base_conversion";
pub const SOURCE: &str = include_str!("../docs/sources/openstax_base_conversion_source.txt");

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BaseConversionStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidDomain,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BaseConversionRequest {
    pub numeral: String,
    pub source_base: u32,
    pub target_base: u32,
    pub domain: String,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BaseConversionResult {
    pub status: BaseConversionStatus,
    pub numeral: Option<String>,
    pub decimal_value: Option<u128>,
    pub source_base: u32,
    pub target_base: u32,
    pub source_provenance: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn payload(result: &BaseConversionResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.numeral,
        result.decimal_value,
        result.source_base,
        result.target_base,
        &result.source_provenance,
        &result.reasons,
        &result.provenance,
    )
}

fn result(
    request: &BaseConversionRequest,
    status: BaseConversionStatus,
    numeral: Option<String>,
    decimal_value: Option<u128>,
    source_provenance: Vec<String>,
    reasons: Vec<String>,
) -> BaseConversionResult {
    let mut output = BaseConversionResult {
        status,
        numeral,
        decimal_value,
        source_base: request.source_base,
        target_base: request.target_base,
        source_provenance,
        reasons,
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&output));
    output.replay_hash = replay_hash;
    output
}

fn digit_value(digit: char) -> Option<u32> {
    match digit.to_ascii_uppercase() {
        upper @ '0'..='9' => Some(upper as u32 - '0' as u32),
        upper @ 'A'..='Z' => Some(upper as u32 - 'A' as u32 + 10),
        _ => None,
    }
}

fn digit_char(value: u32) -> char {
    match value {
        0..=9 => (b'0' + value as u8) as char,
        10..=35 => (b'A' + (value as u8 - 10)) as char,
        _ => unreachable!("validated base digit"),
    }
}

fn decode(numeral: &str, base: u32) -> Option<u128> {
    if numeral.is_empty() || base < 2 || base > 36 {
        return None;
    }
    numeral.chars().try_fold(0u128, |value, digit| {
        let digit = digit_value(digit)?;
        (digit < base).then_some(
            value
                .checked_mul(base as u128)?
                .checked_add(digit as u128)?,
        )
    })
}

/// Validate a finite numeral against its declared base without evaluating it.
pub fn numeral_is_valid(numeral: &str, base: u32) -> bool {
    base >= 2
        && base <= 36
        && !numeral.is_empty()
        && numeral
            .chars()
            .all(|digit| digit_value(digit).is_some_and(|value| value < base))
}

fn encode(mut value: u128, base: u32) -> Option<String> {
    if base < 2 || base > 36 {
        return None;
    }
    if value == 0 {
        return Some("0".into());
    }
    let mut digits = Vec::new();
    while value > 0 {
        digits.push(digit_char((value % base as u128) as u32));
        value /= base as u128;
    }
    digits.reverse();
    Some(digits.into_iter().collect())
}

fn source_provenance() -> Vec<String> {
    SOURCE
        .lines()
        .filter_map(|line| line.strip_prefix("SOURCE_ID: ").map(str::to_owned))
        .chain(
            SOURCE
                .lines()
                .filter_map(|line| line.strip_prefix("EVIDENCE: ").map(str::to_owned)),
        )
        .collect()
}

pub fn source_valid() -> bool {
    [
        "SOURCE_ID:",
        "TITLE:",
        "SECTION:",
        "URL:",
        "LICENSE:",
        "EVIDENCE:",
        "RULE:",
    ]
    .iter()
    .all(|field| SOURCE.contains(field))
        && SOURCE.contains("bases 2 through 36")
}

pub fn evaluate_base_conversion(request: &BaseConversionRequest) -> BaseConversionResult {
    let source = source_provenance();
    if request.domain != DOMAIN {
        return result(
            request,
            BaseConversionStatus::InvalidDomain,
            None,
            None,
            source,
            vec!["request domain is outside the source-derived conversion record".into()],
        );
    }
    if request.ambiguity.is_some() {
        return result(
            request,
            BaseConversionStatus::Ambiguous,
            None,
            None,
            source,
            vec![request.ambiguity.clone().unwrap()],
        );
    }
    if !source_valid() || request.provenance.is_empty() {
        return result(
            request,
            BaseConversionStatus::Missing,
            None,
            None,
            source,
            vec!["source record or request provenance is incomplete".into()],
        );
    }
    let Some(decimal_value) = decode(&request.numeral, request.source_base) else {
        return result(
            request,
            BaseConversionStatus::Unsupported,
            None,
            None,
            source,
            vec!["numeral contains a digit outside the declared source base".into()],
        );
    };
    let Some(converted) = encode(decimal_value, request.target_base) else {
        return result(
            request,
            BaseConversionStatus::Unsupported,
            None,
            None,
            source,
            vec!["source and target bases must be integers from 2 through 36".into()],
        );
    };
    result(
        request,
        BaseConversionStatus::Complete,
        Some(converted),
        Some(decimal_value),
        source,
        Vec::new(),
    )
}

pub fn replay_verified(result: &BaseConversionResult) -> bool {
    result.replay_hash == digest(&payload(result)) && !result.provenance.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(numeral: &str, source_base: u32, target_base: u32) -> BaseConversionRequest {
        BaseConversionRequest {
            numeral: numeral.into(),
            source_base,
            target_base,
            domain: DOMAIN.into(),
            ambiguity: None,
            provenance: vec!["unit-test:base-conversion".into()],
        }
    }

    #[test]
    fn exact_integer_conversion_is_replayable() {
        let output = evaluate_base_conversion(&request("10101", 3, 10));
        assert_eq!(output.status, BaseConversionStatus::Complete);
        assert_eq!(output.decimal_value, Some(91));
        assert_eq!(output.numeral.as_deref(), Some("91"));
        assert!(replay_verified(&output));
    }

    #[test]
    fn hexadecimal_digits_are_case_insensitive_and_canonicalized() {
        let output = evaluate_base_conversion(&request("a03", 16, 10));
        assert_eq!(output.numeral.as_deref(), Some("2563"));
        assert!(replay_verified(&output));
    }

    #[test]
    fn invalid_digits_and_ambiguous_requests_fail_closed() {
        let invalid = evaluate_base_conversion(&request("29", 2, 10));
        assert_eq!(invalid.status, BaseConversionStatus::Unsupported);
        let mut ambiguous = request("101", 2, 10);
        ambiguous.ambiguity = Some("two target bases remain".into());
        let ambiguous = evaluate_base_conversion(&ambiguous);
        assert_eq!(ambiguous.status, BaseConversionStatus::Ambiguous);
        assert!(replay_verified(&invalid));
        assert!(replay_verified(&ambiguous));
    }

    #[test]
    fn tampered_receipt_is_rejected() {
        let output = evaluate_base_conversion(&request("222", 10, 13));
        let mut tampered = output.clone();
        tampered.replay_hash.push('x');
        assert!(!replay_verified(&tampered));
    }
}
