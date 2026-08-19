//! Provenance-only ingestion for source documents outside executable schemas.
//!
//! An evidence envelope preserves what was observed and where it came from,
//! while explicitly refusing to turn unparsed claims into executable
//! knowledge.  This is the safe first step for unknown source formats and
//! gives later ontology/capability acquisition a replayable residual to
//! analyze.

use crate::source_formula_pack::{validate_source_citation, SourceCitation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceEvidenceEnvelope {
    pub path: String,
    pub source: SourceCitation,
    pub document_sha256: String,
    pub residual_sha256: String,
    pub operation_hints: Vec<String>,
    pub executable: bool,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("source evidence serializes"))
    )
}

fn digest_text(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn payload(envelope: &SourceEvidenceEnvelope) -> impl Serialize + '_ {
    (
        &envelope.path,
        &envelope.source,
        &envelope.document_sha256,
        &envelope.residual_sha256,
        &envelope.operation_hints,
        envelope.executable,
    )
}

fn key_values(document: &str) -> BTreeMap<String, String> {
    document
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            let key = key
                .trim()
                .to_ascii_lowercase()
                .replace('-', "_")
                .replace(' ', "_");
            let value = value.trim();
            (!key.is_empty() && !value.is_empty()).then(|| (key, value.to_string()))
        })
        .collect()
}

fn first(fields: &BTreeMap<String, String>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| fields.get(*key).cloned())
}

/// Ingest source metadata while retaining all unparsed material as a
/// residual hash.  No semantic operation is inferred and `executable` is
/// always false.
pub fn ingest_source_evidence(
    path: impl Into<String>,
    document: &str,
) -> Result<SourceEvidenceEnvelope, Vec<String>> {
    let path = path.into();
    if path.trim().is_empty() || document.trim().is_empty() {
        return Err(vec!["source evidence path and document are required".into()]);
    }
    let fields = key_values(document);
    let required = |name: &str, keys: &[&str]| {
        first(&fields, keys).ok_or_else(|| vec![format!("source evidence lacks {name}")])
    };
    let source = SourceCitation {
        source_id: required("source_id", &["source_id"])?,
        title: required("title", &["title"])?,
        section: required("section", &["section", "sections"])?,
        url: required("url", &["url"])?,
        license: required("license", &["license"])?,
        retrieved_utc: required("retrieved timestamp", &["retrieved_utc", "retrieved"])?,
        evidence_span: required("evidence span", &["evidence", "evidence_span"])?,
    };
    let mut errors = match validate_source_citation(&source) {
        Ok(()) => Vec::new(),
        Err(errors) => errors,
    };
    let operation_hints = [
        "operations",
        "scope",
        "boundary",
        "supported",
        "unsupported",
        "governed_claims",
        "axioms",
    ]
    .iter()
    .filter_map(|key| fields.get(*key).cloned())
    .collect::<Vec<_>>();
    if operation_hints.is_empty() {
        errors.push("source evidence has no declared operation or scope hint".into());
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let document_sha256 = digest_text(document);
    let residual_sha256 = digest(&(document_sha256.clone(), &source, &operation_hints));
    let mut envelope = SourceEvidenceEnvelope {
        path,
        source,
        document_sha256,
        residual_sha256,
        operation_hints,
        executable: false,
        replay_hash: String::new(),
    };
    let replay_hash = {
        let unsigned = payload(&envelope);
        digest(&unsigned)
    };
    envelope.replay_hash = replay_hash;
    Ok(envelope)
}

pub fn replay_verified(envelope: &SourceEvidenceEnvelope) -> bool {
    envelope.replay_hash == digest(&payload(envelope))
        && !envelope.path.is_empty()
        && !envelope.document_sha256.is_empty()
        && !envelope.residual_sha256.is_empty()
        && !envelope.executable
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "SOURCE_ID: source:test\nTITLE: Test\nSECTION: Definitions\nURL: https://example.invalid/test\nLICENSE: test\nRETRIEVED_UTC: 2026-08-19\nEVIDENCE_SPAN: explicit finite scope\nSCOPE: bounded exact operations";

    #[test]
    fn metadata_is_replayable_but_never_executable() {
        let envelope = ingest_source_evidence("docs/test.txt", SOURCE).unwrap();
        assert!(!envelope.executable);
        assert!(replay_verified(&envelope));
    }

    #[test]
    fn missing_provenance_or_scope_is_rejected() {
        assert!(ingest_source_evidence("docs/test.txt", "TITLE: Test").is_err());
        let no_scope = SOURCE.replace("SCOPE: bounded exact operations", "");
        assert!(ingest_source_evidence("docs/test.txt", &no_scope).is_err());
    }

    #[test]
    fn tampering_breaks_replay() {
        let mut envelope = ingest_source_evidence("docs/test.txt", SOURCE).unwrap();
        envelope.residual_sha256.push('x');
        assert!(!replay_verified(&envelope));
    }
}
