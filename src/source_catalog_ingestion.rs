//! Generic provenance-preserving ingestion for bounded source catalogs.

use crate::source_formula_pack::SourceCitation;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceCatalog {
    pub citation: SourceCitation,
    pub operations: Vec<String>,
    pub document_sha256: String,
    pub replay_hash: String,
}
fn digest<T: Serialize>(v: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(v).unwrap()))
}
fn line(document: &str, key: &str) -> Option<String> {
    document
        .lines()
        .find_map(|l| l.strip_prefix(key).map(|x| x.trim().to_string()))
}
pub fn ingest(document: &str) -> Result<SourceCatalog, String> {
    let id = line(document, "SOURCE_ID: ").ok_or("missing source id")?;
    let title = line(document, "TITLE: ").ok_or("missing title")?;
    let section = line(document, "SECTION: ").ok_or("missing section")?;
    let url = line(document, "URL: ").ok_or("missing url")?;
    let license = line(document, "LICENSE: ").ok_or("missing license")?;
    let retrieved = line(document, "RETRIEVED_UTC: ").ok_or("missing retrieval date")?;
    let evidence = line(document, "EVIDENCE: ").ok_or("missing evidence span")?;
    let ops = line(document, "OPERATIONS: ")
        .ok_or("missing operation declarations")?
        .split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>();
    if !url.starts_with("https://") || ops.is_empty() {
        return Err("source url or operations are invalid".into());
    }
    let citation = SourceCitation {
        source_id: id,
        title,
        section,
        url,
        license,
        retrieved_utc: retrieved,
        evidence_span: evidence,
    };
    let doc_hash = digest(&document);
    let mut c = SourceCatalog {
        citation,
        operations: ops,
        document_sha256: doc_hash,
        replay_hash: String::new(),
    };
    c.replay_hash = digest(&(&c.citation, &c.operations, &c.document_sha256));
    Ok(c)
}
pub fn replay_verified(c: &SourceCatalog) -> bool {
    c.replay_hash == digest(&(&c.citation, &c.operations, &c.document_sha256))
        && !c.operations.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ingests_metadata() {
        let d="SOURCE_ID: x\nTITLE: T\nSECTION: S\nURL: https://x\nLICENSE: L\nRETRIEVED_UTC: d\nEVIDENCE: e\nOPERATIONS: a, b\n";
        let c = ingest(d).unwrap();
        assert!(replay_verified(&c));
    }
}
