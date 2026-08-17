//! Narrow technical-language frontend for the source-derived Möbius pack.
//!
//! The frontend accepts only explicit finite sequence literals and explicit
//! one-based indexing evidence.  It does not infer a sequence from prose,
//! choose among asymptotic interpretations, or treat a divisor sum as an
//! inversion request without an operation phrase.

use crate::mobius_inversion_pack::{MobiusOperation, MobiusRequest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MobiusFrontendStatus { Complete, Ambiguous, Unsupported, Missing }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MobiusFrontendResult {
    pub status: MobiusFrontendStatus,
    pub request: Option<MobiusRequest>,
    pub provenance_spans: Vec<String>,
    pub alternatives: Vec<String>,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String { format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap())) }

fn payload(result: &MobiusFrontendResult) -> impl Serialize + '_ {
    (&result.status, &result.request, &result.provenance_spans, &result.alternatives, &result.reasons)
}

fn output(status: MobiusFrontendStatus, request: Option<MobiusRequest>, spans: Vec<String>, alternatives: Vec<String>, reasons: Vec<String>) -> MobiusFrontendResult {
    let replay_hash = digest(&(&status, &request, &spans, &alternatives, &reasons));
    MobiusFrontendResult { status, request, provenance_spans: spans, alternatives, reasons, replay_hash }
}

fn integers_in_brackets(text: &str) -> Option<Vec<i128>> {
    let start = text.find('[')?;
    let end = text[start + 1..].find(']')? + start + 1;
    let body = &text[start + 1..end];
    let values = body.split(',').map(|part| part.trim().parse::<i128>().ok()).collect::<Option<Vec<_>>>()?;
    (!values.is_empty()).then_some(values)
}

/// Formalize one bounded Möbius technical report.
pub fn formalize_mobius_text(text: &str) -> MobiusFrontendResult {
    let lower = text.to_ascii_lowercase();
    let provenance = vec!["mobius-operation-span".into(), "mobius-sequence-span".into()];
    if ["asymptotic", "infinite", "analytic continuation", "dirichlet series"].iter().any(|marker| lower.contains(marker)) {
        return output(MobiusFrontendStatus::Unsupported, None, provenance, Vec::new(), vec!["asymptotic or infinite number-theory semantics are outside the finite pack".into()]);
    }
    let operation = if lower.contains("mobius inversion") || lower.contains("möbius inversion") {
        MobiusOperation::InvertFiniteSequence
    } else if lower.contains("divisor convolution") || lower.contains("dirichlet convolution") {
        MobiusOperation::DivisorConvolution
    } else {
        return output(MobiusFrontendStatus::Missing, None, provenance, Vec::new(), vec!["a supported finite Möbius operation is not stated".into()]);
    };
    let Some(values) = integers_in_brackets(text) else {
        return output(MobiusFrontendStatus::Missing, None, provenance, Vec::new(), vec!["an explicit finite sequence literal is required".into()]);
    };
    if values.len() > 32 {
        return output(MobiusFrontendStatus::Unsupported, None, provenance, Vec::new(), vec!["sequence exceeds the finite length bound".into()]);
    }
    if !(lower.contains("f(1)") || lower.contains("f[1]") || lower.contains("indexed from 1") || lower.contains("indexed at 1")) {
        return output(MobiusFrontendStatus::Ambiguous, None, provenance, vec!["one-based sequence indexing is unresolved".into()], vec!["the literal lacks explicit f(1)..f(n) indexing evidence".into()]);
    }
    if lower.contains("unclear") || lower.contains("either") || lower.contains(" or ") {
        return output(MobiusFrontendStatus::Ambiguous, None, provenance, vec!["operation or divisor convention has competing readings".into()], vec!["multiple semantic readings remain".into()]);
    }
    let second_values = if operation == MobiusOperation::DivisorConvolution {
        if let Some(second_start) = text.rfind('[') {
            let tail = &text[second_start..];
            integers_in_brackets(tail).unwrap_or_default()
        } else { Vec::new() }
    } else { Vec::new() };
    if operation == MobiusOperation::DivisorConvolution && second_values.len() != values.len() {
        return output(MobiusFrontendStatus::Ambiguous, None, provenance, vec!["second sequence binding is unresolved".into()], vec!["divisor convolution requires two equally indexed sequences".into()]);
    }
    let request = MobiusRequest { operation, values: Some(values), second_values: (operation == MobiusOperation::DivisorConvolution).then_some(second_values), domain: "bounded_source_mobius_inversion".into(), indexing_declared: true, ambiguity: None, provenance: vec!["mobius-frontend".into(), text.to_string()] };
    output(MobiusFrontendStatus::Complete, Some(request), provenance, Vec::new(), Vec::new())
}

impl MobiusFrontendResult {
    pub fn replay_verified(&self) -> bool { self.replay_hash == digest(&payload(self)) && !self.provenance_spans.is_empty() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_inversion_is_typed_and_replayable() {
        let result = formalize_mobius_text("Apply Mobius inversion to f(1)..f(n) indexed from 1: [1,2,3,4].");
        assert_eq!(result.status, MobiusFrontendStatus::Complete);
        assert!(result.replay_verified());
    }

    #[test]
    fn missing_indexing_and_asymptotics_fail_closed() {
        let ambiguous = formalize_mobius_text("Apply Mobius inversion to [1,2,3,4].");
        assert_eq!(ambiguous.status, MobiusFrontendStatus::Ambiguous);
        let unsupported = formalize_mobius_text("Find the asymptotic Mobius inversion of f(1)..f(n) indexed from 1: [1,2].");
        assert_eq!(unsupported.status, MobiusFrontendStatus::Unsupported);
    }
}
