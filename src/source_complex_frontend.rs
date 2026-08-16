//! Fail-closed technical-language frontend for source-derived complex
//! arithmetic.  It accepts only explicit rectangular literals such as
//! `(3-4i)` and explicit operation evidence; it never infers polar or
//! analytic semantics from a nearby word.

use crate::probability_pack::Rational;
use crate::source_complex_pack::{ComplexOperation, ComplexRequest, DOMAIN};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrontendStatus {
    Complete,
    Ambiguous,
    Unsupported,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComplexFrontendResult {
    pub status: FrontendStatus,
    pub operation: Option<ComplexOperation>,
    pub request: Option<ComplexRequest>,
    pub provenance_spans: Vec<String>,
    pub alternatives: Vec<String>,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn payload(result: &ComplexFrontendResult) -> impl Serialize + '_ {
    (
        result.status,
        result.operation,
        &result.request,
        &result.provenance_spans,
        &result.alternatives,
        &result.reasons,
    )
}

fn result(
    status: FrontendStatus,
    operation: Option<ComplexOperation>,
    request: Option<ComplexRequest>,
    spans: Vec<String>,
    alternatives: Vec<String>,
    reasons: Vec<String>,
) -> ComplexFrontendResult {
    let mut output = ComplexFrontendResult {
        status,
        operation,
        request,
        provenance_spans: spans,
        alternatives,
        reasons,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&output));
    output.replay_hash = replay_hash;
    output
}

fn parse_rational(text: &str) -> Option<Rational> {
    let text = text.trim();
    if let Some((numerator, denominator)) = text.split_once('/') {
        return Rational::new(numerator.parse().ok()?, denominator.parse().ok()?);
    }
    Rational::new(text.parse().ok()?, 1)
}

fn parse_complex_literal(text: &str) -> Option<(Rational, Rational)> {
    let compact: String = text
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let compact = compact.strip_suffix('i').unwrap_or(&compact);
    if compact.is_empty() {
        return None;
    }
    let split = compact
        .char_indices()
        .skip(1)
        .find(|(_, character)| *character == '+' || *character == '-')
        .map(|(index, _)| index);
    match split {
        Some(index) => {
            let real = parse_rational(&compact[..index])?;
            let mut imaginary = compact[index..].to_string();
            if imaginary == "+" || imaginary == "-" {
                imaginary.push('1');
            }
            Some((real, parse_rational(&imaginary)?))
        }
        None if compact == "1" => Some((Rational::zero(), Rational::one())),
        None if compact == "-1" => Some((Rational::zero(), Rational::new(-1, 1)?)),
        None => Some((parse_rational(compact)?, Rational::zero())),
    }
}

fn parenthesized_literals(text: &str) -> Vec<String> {
    let mut spans = Vec::new();
    let mut start = None;
    for (index, character) in text.char_indices() {
        match (character, start) {
            ('(', None) => start = Some(index),
            (')', Some(begin)) => {
                spans.push(text[begin + 1..index].to_string());
                start = None;
            }
            _ => {}
        }
    }
    spans
}

fn detect_operation(lower: &str) -> Result<ComplexOperation, Vec<String>> {
    if lower.contains("polar") || lower.contains("argument") || lower.contains("analytic") {
        return Ok(ComplexOperation::PolarConversion);
    }
    let mut candidates = Vec::new();
    if lower.contains("conjugate") {
        candidates.push(ComplexOperation::Conjugate);
    }
    if lower.contains("squared magnitude")
        || lower.contains("magnitude squared")
        || lower.contains("norm squared")
        || lower.contains("modulus squared")
    {
        candidates.push(ComplexOperation::NormSquared);
    }
    if lower.contains("divide") || lower.contains("quotient") {
        candidates.push(ComplexOperation::Divide);
    }
    if lower.contains("multiply") || lower.contains("product") {
        candidates.push(ComplexOperation::Multiply);
    }
    if lower.contains("subtract") || lower.contains("difference") {
        candidates.push(ComplexOperation::Subtract);
    }
    if lower.contains("add") || lower.contains("sum") {
        candidates.push(ComplexOperation::Add);
    }
    candidates.sort_by_key(|operation| format!("{operation:?}"));
    candidates.dedup();
    match candidates.as_slice() {
        [operation] => Ok(*operation),
        [] => Err(vec![
            "no explicit supported complex operation was identified".into(),
        ]),
        _ => Err(candidates
            .iter()
            .map(|operation| format!("{operation:?}"))
            .collect()),
    }
}

/// Parse a deliberately bounded natural-language complex arithmetic request.
pub fn formalize_complex_text(text: &str) -> ComplexFrontendResult {
    let lower = text.to_ascii_lowercase();
    if lower.contains("decimal")
        || lower.contains("approx")
        || lower.contains("limit")
        || lower.contains("exponential")
    {
        return result(
            FrontendStatus::Unsupported,
            None,
            None,
            vec![text.into()],
            Vec::new(),
            vec!["request requires semantics outside exact rectangular complex arithmetic".into()],
        );
    }
    let operation = match detect_operation(&lower) {
        Ok(operation) => operation,
        Err(alternatives) if alternatives.len() > 1 => {
            return result(
                FrontendStatus::Ambiguous,
                None,
                None,
                vec![text.into()],
                alternatives,
                vec!["multiple operation interpretations remain plausible".into()],
            )
        }
        Err(reasons) => {
            return result(
                FrontendStatus::Missing,
                None,
                None,
                vec![text.into()],
                Vec::new(),
                reasons,
            )
        }
    };
    if operation == ComplexOperation::PolarConversion {
        return result(
            FrontendStatus::Unsupported,
            Some(operation),
            None,
            vec![text.into()],
            Vec::new(),
            vec!["polar and branch semantics are outside the bounded source pack".into()],
        );
    }
    let spans = parenthesized_literals(text);
    let parsed: Vec<_> = spans
        .iter()
        .filter_map(|span| parse_complex_literal(span))
        .collect();
    let required = match operation {
        ComplexOperation::Conjugate | ComplexOperation::NormSquared => 1,
        _ => 2,
    };
    if parsed.len() != required || spans.len() != required {
        let provenance_spans = if spans.is_empty() {
            vec![text.into()]
        } else {
            spans.clone()
        };
        return result(
            FrontendStatus::Missing,
            Some(operation),
            None,
            provenance_spans,
            Vec::new(),
            vec![format!(
                "expected {required} explicit parenthesized complex literal(s)"
            )],
        );
    }
    let (a, b) = parsed[0].clone();
    let (c, d) = if required == 2 {
        let (real, imag) = parsed[1].clone();
        (Some(real), Some(imag))
    } else {
        (None, None)
    };
    let request = ComplexRequest {
        operation,
        a: Some(a),
        b: Some(b),
        c,
        d,
        domain: DOMAIN.into(),
        ambiguity: None,
        provenance: spans.clone(),
    };
    result(
        FrontendStatus::Complete,
        Some(operation),
        Some(request),
        spans,
        Vec::new(),
        Vec::new(),
    )
}

impl ComplexFrontendResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance_spans.is_empty()
            && (self.status != FrontendStatus::Complete || self.request.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_product_formalizes_and_replays() {
        let parsed = formalize_complex_text("Find the product of (3-4i) and (2+5i).");
        assert_eq!(parsed.status, FrontendStatus::Complete);
        assert_eq!(parsed.operation, Some(ComplexOperation::Multiply));
        assert!(parsed.replay_verified());
    }

    #[test]
    fn missing_operation_and_polar_request_fail_closed() {
        assert_eq!(
            formalize_complex_text("Evaluate (3-4i) and (2+5i).").status,
            FrontendStatus::Missing
        );
        assert_eq!(
            formalize_complex_text("Convert (3-4i) to polar form.").status,
            FrontendStatus::Unsupported
        );
    }
}
