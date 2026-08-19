//! Generic provenance and scope extraction for source documents.
//!
//! This module deliberately extracts only source metadata. It does not infer
//! domain operations or turn prose claims into executable knowledge. A
//! downstream pack must still validate that its source identity and scope
//! match the extracted metadata before execution.

use crate::source_formula_pack::{validate_source_citation, SourceCitation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceMetadata {
    pub citation: SourceCitation,
    pub scope: Option<String>,
    pub unsupported: Option<String>,
    pub document_sha256: String,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("source metadata serializes"))
    )
}

fn payload(metadata: &SourceMetadata) -> impl Serialize + '_ {
    (
        &metadata.citation,
        &metadata.scope,
        &metadata.unsupported,
        &metadata.document_sha256,
    )
}

pub fn replay_verified(metadata: &SourceMetadata) -> bool {
    metadata.replay_hash == digest(&payload(metadata))
        && !metadata.document_sha256.is_empty()
        && validate_source_citation(&metadata.citation).is_ok()
}

fn first(fields: &BTreeMap<String, String>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| fields.get(*key).cloned())
}

/// Extract required citation fields and optional scope boundaries.
pub fn extract_source_metadata(document: &str) -> Result<SourceMetadata, Vec<String>> {
    let mut fields = BTreeMap::new();
    let mut body = Vec::new();
    for raw in document.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim().to_ascii_uppercase();
            let value = value.trim();
            if !key.is_empty() && !value.is_empty() {
                fields.entry(key).or_insert_with(|| value.to_string());
                continue;
            }
        }
        body.push(line);
    }
    let required = |keys: &[&str], label: &str, errors: &mut Vec<String>| match first(&fields, keys)
    {
        Some(value) => Some(value),
        None => {
            errors.push(format!("source metadata lacks {label}"));
            None
        }
    };
    let mut errors = Vec::new();
    let source_id = required(&["SOURCE_ID"], "SOURCE_ID", &mut errors);
    let title = required(&["TITLE"], "TITLE", &mut errors);
    let section = required(&["SECTION", "SECTIONS"], "SECTION", &mut errors);
    let url = required(&["URL"], "URL", &mut errors);
    let license = required(&["LICENSE", "LICENCE"], "LICENSE", &mut errors);
    let retrieved_utc = required(
        &["RETRIEVED_UTC", "RETRIEVED"],
        "RETRIEVED_UTC",
        &mut errors,
    );
    let evidence_span = required(&["EVIDENCE", "EVIDENCE_SPAN"], "EVIDENCE", &mut errors);
    if !errors.is_empty() {
        return Err(errors);
    }
    let citation = SourceCitation {
        source_id: source_id.unwrap(),
        title: title.unwrap(),
        section: section.unwrap(),
        url: url.unwrap(),
        license: license.unwrap(),
        retrieved_utc: retrieved_utc.unwrap(),
        evidence_span: evidence_span.unwrap(),
    };
    if let Err(citation_errors) = validate_source_citation(&citation) {
        errors.extend(citation_errors);
    }
    let scope = first(&fields, &["SCOPE"]);
    let unsupported = first(&fields, &["UNSUPPORTED", "BOUNDARY"]);
    if scope.is_none() && body.is_empty() {
        errors.push("source metadata has no scope or body boundary".into());
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let document_sha256 = format!("{:x}", Sha256::digest(document.as_bytes()));
    let mut metadata = SourceMetadata {
        citation,
        scope: scope.or_else(|| Some(body.join(" "))),
        unsupported,
        document_sha256,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&metadata));
    metadata.replay_hash = replay_hash;
    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "SOURCE_ID: test:source\nTITLE: Test\nSECTION: 1\nURL: https://example.test\nLICENSE: CC BY\nRETRIEVED_UTC: 2026-08-19\nEVIDENCE: exact definition\nSCOPE: finite only";

    #[test]
    fn extracts_and_replays_metadata() {
        let metadata = extract_source_metadata(SOURCE).unwrap();
        assert_eq!(metadata.citation.source_id, "test:source");
        assert_eq!(metadata.scope.as_deref(), Some("finite only"));
        assert!(replay_verified(&metadata));
    }

    #[test]
    fn missing_citation_fails_closed() {
        let error = extract_source_metadata("TITLE: missing").unwrap_err();
        assert!(error.iter().any(|item| item.contains("SOURCE_ID")));
    }
}
