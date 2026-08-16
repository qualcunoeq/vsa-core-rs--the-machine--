//! Narrow technical-language frontend for the bounded DNA biology pack.
//!
//! Only explicit sequence cues are accepted. The frontend never infers DNA
//! from arbitrary biological vocabulary and never supplies a strand
//! orientation that the report did not state.

use super::{BiologyOperation, BiologyRequest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BiologyFrontendStatus {
    Complete,
    Ambiguous,
    Unsupported,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BiologyFrontendResult {
    pub status: BiologyFrontendStatus,
    pub request: Option<BiologyRequest>,
    pub candidate_spans: Vec<String>,
    pub unresolved_alternatives: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("biology frontend serializes"))
    )
}

fn payload(result: &BiologyFrontendResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.request,
        &result.candidate_spans,
        &result.unresolved_alternatives,
        &result.reasons,
        &result.provenance,
    )
}

fn output(
    status: BiologyFrontendStatus,
    request: Option<BiologyRequest>,
    candidates: Vec<String>,
    alternatives: Vec<String>,
    reasons: Vec<String>,
    text: &str,
) -> BiologyFrontendResult {
    let mut result = BiologyFrontendResult {
        status,
        request,
        candidate_spans: candidates,
        unresolved_alternatives: alternatives,
        reasons,
        provenance: vec![format!(
            "biology-frontend-text-sha256:{:x}",
            Sha256::digest(text.as_bytes())
        )],
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn sequence_candidates(text: &str) -> Vec<String> {
    let lower = text.to_ascii_lowercase();
    let mut candidates = Vec::new();
    for marker in ["sequence:", "sequence is", "dna sequence ", "strand:"] {
        let mut start = 0;
        while let Some(offset) = lower[start..].find(marker) {
            let index = start + offset + marker.len();
            let token: String = text[index..]
                .chars()
                .skip_while(|value| value.is_whitespace())
                .take_while(|value| value.is_ascii_alphabetic())
                .collect();
            let normalized = token.to_ascii_uppercase();
            if normalized.len() > 1
                && normalized.chars().all(|value| matches!(value, 'A' | 'T' | 'C' | 'G'))
                && !candidates.contains(&normalized)
            {
                candidates.push(normalized);
            }
            start = index;
        }
    }
    candidates
}

fn request(operation: BiologyOperation, sequence: String, orientation: Option<String>, text: &str) -> BiologyRequest {
    BiologyRequest {
        operation,
        sequence: Some(sequence),
        orientation,
        domain: "source_derived_bounded_dna".into(),
        ambiguity: None,
        provenance: vec![format!(
            "biology-frontend-text:{:x}",
            Sha256::digest(text.as_bytes())
        )],
    }
}

/// Formalize explicit, bounded DNA language into a typed biology request.
pub fn formalize_biology_text(text: &str) -> BiologyFrontendResult {
    let lower = text.to_ascii_lowercase();
    let unsupported_terms = [
        "rna",
        "mrna",
        "codon",
        "translation",
        "protein",
        "mutation",
        "phenotype",
        "gene expression",
    ];
    if unsupported_terms.iter().any(|term| lower.contains(term)) {
        return output(
            BiologyFrontendStatus::Unsupported,
            None,
            Vec::new(),
            Vec::new(),
            vec!["requested semantics exceed bounded DNA representation".into()],
            text,
        );
    }
    let candidates = sequence_candidates(text);
    if candidates.len() > 1 {
        return output(
            BiologyFrontendStatus::Ambiguous,
            None,
            candidates.clone(),
            candidates,
            vec!["multiple explicit sequence spans require a target declaration".into()],
            text,
        );
    }
    let Some(sequence) = candidates.first().cloned() else {
        return output(
            BiologyFrontendStatus::Missing,
            None,
            Vec::new(),
            Vec::new(),
            vec!["no explicit DNA sequence cue was found".into()],
            text,
        );
    };
    let operation = if lower.contains("reverse complement") || lower.contains("reverse-complement") {
        BiologyOperation::ReverseComplement
    } else if lower.contains("complement") {
        BiologyOperation::Complement
    } else if lower.contains("base composition") || lower.contains("gc content") {
        BiologyOperation::BaseComposition
    } else {
        BiologyOperation::ValidateDna
    };
    let orientation = if lower.contains("5' to 3'")
        || lower.contains("5’ to 3’")
        || lower.contains("5_to_3")
    {
        Some("5_to_3".into())
    } else {
        None
    };
    if matches!(operation, BiologyOperation::Complement | BiologyOperation::ReverseComplement)
        && orientation.is_none()
    {
        return output(
            BiologyFrontendStatus::Ambiguous,
            None,
            vec![sequence],
            Vec::new(),
            vec!["complement operation requires explicit 5-to-3 orientation".into()],
            text,
        );
    }
    let request = request(operation, sequence.clone(), orientation, text);
    output(
        BiologyFrontendStatus::Complete,
        Some(request),
        vec![sequence],
        Vec::new(),
        Vec::new(),
        text,
    )
}

impl BiologyFrontendResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != BiologyFrontendStatus::Complete || self.request.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_sequence_and_orientation_are_typed() {
        let result = formalize_biology_text("Find the reverse complement of DNA sequence: AATTGGCC, 5' to 3'.");
        assert_eq!(result.status, BiologyFrontendStatus::Complete);
        assert_eq!(result.request.as_ref().unwrap().operation, BiologyOperation::ReverseComplement);
        assert!(result.replay_verified());
    }

    #[test]
    fn missing_orientation_and_rna_fail_closed() {
        let ambiguous = formalize_biology_text("Find the complement of sequence: AATTGGCC.");
        assert_eq!(ambiguous.status, BiologyFrontendStatus::Ambiguous);
        let unsupported = formalize_biology_text("Translate the codon sequence: AUG.");
        assert_eq!(unsupported.status, BiologyFrontendStatus::Unsupported);
    }
}
