//! Source-derived exact arithmetic on finite positional numerals.
//!
//! The positional representation rule is attributed to the accompanying
//! OpenStax source transcription.  Arithmetic is deliberately bounded to two
//! nonnegative numerals in one explicitly declared base; conversion and
//! operation semantics are kept in the typed request rather than inferred
//! from a subject label.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const DOMAIN: &str = "source_derived_base_arithmetic";
pub const SOURCE: &str = include_str!("../docs/sources/openstax_base_arithmetic_source.txt");
pub const MIN_BASE: u32 = 2;
pub const MAX_BASE: u32 = 12;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BaseArithmeticOperation {
    Add,
    Subtract,
    Multiply,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BaseArithmeticRequest {
    pub left: String,
    pub right: String,
    pub base: u32,
    pub target_base: u32,
    pub operation: BaseArithmeticOperation,
    pub domain: String,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BaseArithmeticStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidDomain,
    Inconsistent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BaseArithmeticResult {
    pub status: BaseArithmeticStatus,
    pub numeral: Option<String>,
    pub decimal_value: Option<u128>,
    pub operation: BaseArithmeticOperation,
    pub base: u32,
    pub target_base: u32,
    pub source_provenance: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn payload(result: &BaseArithmeticResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.numeral,
        result.decimal_value,
        result.operation,
        result.base,
        result.target_base,
        &result.source_provenance,
        &result.reasons,
        &result.provenance,
    )
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

fn output(
    request: &BaseArithmeticRequest,
    status: BaseArithmeticStatus,
    numeral: Option<String>,
    decimal_value: Option<u128>,
    reasons: Vec<String>,
) -> BaseArithmeticResult {
    let mut result = BaseArithmeticResult {
        status,
        numeral,
        decimal_value,
        operation: request.operation,
        base: request.base,
        target_base: request.target_base,
        source_provenance: source_provenance(),
        reasons,
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn digit_value(digit: char) -> Option<u32> {
    match digit.to_ascii_uppercase() {
        value @ '0'..='9' => Some(value as u32 - '0' as u32),
        value @ 'A'..='Z' => Some(value as u32 - 'A' as u32 + 10),
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
    if base < MIN_BASE || base > MAX_BASE || numeral.is_empty() {
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

fn encode(mut value: u128, base: u32) -> Option<String> {
    if base < MIN_BASE || base > MAX_BASE {
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

pub fn evaluate(request: &BaseArithmeticRequest) -> BaseArithmeticResult {
    if request.domain != DOMAIN {
        return output(
            request,
            BaseArithmeticStatus::InvalidDomain,
            None,
            None,
            vec!["request domain is outside source-derived base arithmetic".into()],
        );
    }
    if request.ambiguity.is_some() {
        return output(
            request,
            BaseArithmeticStatus::Ambiguous,
            None,
            None,
            vec![request.ambiguity.clone().unwrap()],
        );
    }
    if request.provenance.is_empty() {
        return output(
            request,
            BaseArithmeticStatus::Missing,
            None,
            None,
            vec!["source provenance is required".into()],
        );
    }
    if request.target_base != request.base {
        return output(
            request,
            BaseArithmeticStatus::Unsupported,
            None,
            None,
            vec![
                "source-derived arithmetic requires the output to remain in the operand base; use the validated conversion bridge separately".into(),
            ],
        );
    }
    let Some(left) = decode(&request.left, request.base) else {
        return output(
            request,
            BaseArithmeticStatus::Unsupported,
            None,
            None,
            vec!["left numeral is invalid for its declared base".into()],
        );
    };
    let Some(right) = decode(&request.right, request.base) else {
        return output(
            request,
            BaseArithmeticStatus::Unsupported,
            None,
            None,
            vec!["right numeral is invalid for its declared base".into()],
        );
    };
    let Some(value) = (match request.operation {
        BaseArithmeticOperation::Add => left.checked_add(right),
        BaseArithmeticOperation::Subtract => left.checked_sub(right),
        BaseArithmeticOperation::Multiply => left.checked_mul(right),
    }) else {
        return output(
            request,
            BaseArithmeticStatus::Inconsistent,
            None,
            None,
            vec!["operation overflows or produces a negative result".into()],
        );
    };
    let Some(numeral) = encode(value, request.target_base) else {
        return output(
            request,
            BaseArithmeticStatus::Unsupported,
            None,
            None,
            vec![format!("base must be between {MIN_BASE} and {MAX_BASE}")],
        );
    };
    output(
        request,
        BaseArithmeticStatus::Complete,
        Some(numeral),
        Some(value),
        Vec::new(),
    )
}

pub fn replay_verified(result: &BaseArithmeticResult) -> bool {
    result.replay_hash == digest(&payload(result))
        && !result.provenance.is_empty()
        && !result.source_provenance.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(
        left: &str,
        right: &str,
        base: u32,
        target_base: u32,
        operation: BaseArithmeticOperation,
    ) -> BaseArithmeticRequest {
        BaseArithmeticRequest {
            left: left.into(),
            right: right.into(),
            base,
            target_base,
            operation,
            domain: DOMAIN.into(),
            ambiguity: None,
            provenance: vec!["source-test".into()],
        }
    }

    #[test]
    fn exact_addition_and_multiplication_replay() {
        let add = evaluate(&request("1011", "101", 2, 2, BaseArithmeticOperation::Add));
        assert_eq!(add.numeral.as_deref(), Some("10000"));
        assert!(replay_verified(&add));
        let product = evaluate(&request(
            "315",
            "4",
            6,
            6,
            BaseArithmeticOperation::Multiply,
        ));
        assert_eq!(product.numeral.as_deref(), Some("2112"));
        assert!(replay_verified(&product));
    }

    #[test]
    fn subtraction_refuses_negative_and_invalid_digit() {
        let negative = evaluate(&request("1", "10", 2, 2, BaseArithmeticOperation::Subtract));
        assert_eq!(negative.status, BaseArithmeticStatus::Inconsistent);
        let invalid = evaluate(&request("29", "1", 2, 10, BaseArithmeticOperation::Add));
        assert_eq!(invalid.status, BaseArithmeticStatus::Unsupported);
        assert!(replay_verified(&negative));
        assert!(replay_verified(&invalid));
    }

    #[test]
    fn cross_base_output_requires_conversion_bridge() {
        let result = evaluate(&request("101", "11", 2, 10, BaseArithmeticOperation::Add));
        assert_eq!(result.status, BaseArithmeticStatus::Unsupported);
        assert!(replay_verified(&result));
    }

    #[test]
    fn tampered_receipt_is_rejected() {
        let result = evaluate(&request("101", "11", 2, 10, BaseArithmeticOperation::Add));
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        assert!(!replay_verified(&tampered));
    }
}
