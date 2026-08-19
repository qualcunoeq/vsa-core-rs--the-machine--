//! Exact-version immutable storage for discovered multi-format catalogs.
//!
//! This is a storage bridge, not an executor.  Formula, relation, and finite
//! topology catalogs retain their typed record variant, source lineage, and
//! discovery version when appended to a cloned curriculum memory.  Retrieval
//! never falls through to a nearby version or a different catalog kind.

use crate::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};
use crate::source_multiformat_discovery::{
    DiscoveredSourceCatalog, SourceCatalogKind, SourceCatalogRecords,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CatalogMemoryStatus {
    Unique,
    Missing,
    Ambiguous,
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatalogMemoryResult {
    pub status: CatalogMemoryStatus,
    pub kind: Option<SourceCatalogKind>,
    pub domain: String,
    pub version: String,
    pub records: Option<SourceCatalogRecords>,
    pub memory_record_ids: Vec<String>,
    pub provenance: Vec<String>,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

fn artifact_type(kind: SourceCatalogKind) -> String {
    format!("source_catalog::{:?}", kind).to_ascii_lowercase()
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("catalog memory serializes"))
    )
}

fn payload(result: &CatalogMemoryResult) -> impl Serialize + '_ {
    (
        result.status,
        result.kind,
        &result.domain,
        &result.version,
        &result.records,
        &result.memory_record_ids,
        &result.provenance,
        &result.reasons,
    )
}

fn finish(
    status: CatalogMemoryStatus,
    kind: Option<SourceCatalogKind>,
    domain: &str,
    version: &str,
    records: Option<SourceCatalogRecords>,
    memory_record_ids: Vec<String>,
    provenance: Vec<String>,
    reasons: Vec<String>,
) -> CatalogMemoryResult {
    let mut result = CatalogMemoryResult {
        status,
        kind,
        domain: domain.into(),
        version: version.into(),
        records,
        memory_record_ids,
        provenance,
        reasons,
        replay_hash: String::new(),
    };
    let replay_hash = {
        let unsigned = payload(&result);
        digest(&unsigned)
    };
    result.replay_hash = replay_hash;
    result
}

/// Append one discovered catalog to an immutable memory clone.
pub fn append_catalog(
    memory: &mut CurriculumMemory,
    catalog: &DiscoveredSourceCatalog,
) -> AppendStatus {
    if catalog.records.is_empty()
        || catalog.candidate.domain.is_empty()
        || catalog.source_hash.is_empty()
        || catalog.candidate.source_ids.is_empty()
        || !super::source_multiformat_discovery::replay_verified(catalog)
    {
        return AppendStatus::Invalid;
    }
    let Ok(payload) = serde_json::to_string(&catalog.records) else {
        return AppendStatus::Invalid;
    };
    memory.append(MemoryRecord {
        record_id: format!("multiformat-catalog::{}", catalog.source_hash),
        domain: catalog.candidate.domain.clone(),
        artifact_type: artifact_type(catalog.kind),
        version: catalog.source_hash.clone(),
        payload,
        provenance: catalog.candidate.source_ids.clone(),
        content_hash: String::new(),
    })
}

/// Retrieve exactly one catalog kind and version from immutable memory.
pub fn retrieve_catalog(
    memory: &CurriculumMemory,
    kind: SourceCatalogKind,
    domain: &str,
    version: &str,
) -> CatalogMemoryResult {
    let matches = memory.retrieve_exact_version(domain, &artifact_type(kind), version);
    if matches.is_empty() {
        return finish(
            CatalogMemoryStatus::Missing,
            Some(kind),
            domain,
            version,
            None,
            Vec::new(),
            Vec::new(),
            vec!["exact catalog kind and version are absent".into()],
        );
    }
    if matches.len() != 1 {
        return finish(
            CatalogMemoryStatus::Ambiguous,
            Some(kind),
            domain,
            version,
            None,
            matches
                .iter()
                .map(|record| record.record_id.clone())
                .collect(),
            matches
                .iter()
                .flat_map(|record| record.provenance.clone())
                .collect(),
            vec!["more than one exact catalog matches".into()],
        );
    }
    let record = matches[0];
    if !memory.replay_verified(record) {
        return finish(
            CatalogMemoryStatus::Invalid,
            Some(kind),
            domain,
            version,
            None,
            vec![record.record_id.clone()],
            record.provenance.clone(),
            vec!["stored catalog receipt failed replay verification".into()],
        );
    }
    let Ok(records) = serde_json::from_str::<SourceCatalogRecords>(&record.payload) else {
        return finish(
            CatalogMemoryStatus::Invalid,
            Some(kind),
            domain,
            version,
            None,
            vec![record.record_id.clone()],
            record.provenance.clone(),
            vec!["stored catalog payload is not typed data".into()],
        );
    };
    if records.kind() != kind || records.is_empty() {
        return finish(
            CatalogMemoryStatus::Invalid,
            Some(kind),
            domain,
            version,
            None,
            vec![record.record_id.clone()],
            record.provenance.clone(),
            vec!["stored catalog kind or record set is invalid".into()],
        );
    }
    finish(
        CatalogMemoryStatus::Unique,
        Some(kind),
        domain,
        version,
        Some(records),
        vec![record.record_id.clone()],
        record.provenance.clone(),
        Vec::new(),
    )
}

pub fn replay_verified(result: &CatalogMemoryResult) -> bool {
    result.replay_hash == digest(&payload(result))
        && !result.domain.is_empty()
        && !result.version.is_empty()
        && (!matches!(result.status, CatalogMemoryStatus::Unique)
            || (result.records.is_some() && !result.memory_record_ids.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_multiformat_discovery::{discover_source_catalog, SourceCatalogDocument};

    const SOURCE: &str = "BEGIN RELATION\nRELATION_ID: pair\nALIASES: pair\nDOMAIN: test\nPAIRS: a=b\nASSUMPTIONS: explicit\nSOURCE_ID: source:test\nTITLE: Test\nSECTION: 1\nURL: https://example.invalid\nLICENSE: test\nRETRIEVED: 2026-08-19\nEVIDENCE: explicit\nEND RELATION";

    #[test]
    fn exact_kind_and_version_retrieval_is_replayable() {
        let catalog = discover_source_catalog(SourceCatalogDocument {
            path: "test.txt",
            document: SOURCE,
        })
        .unwrap();
        let mut memory = CurriculumMemory::new();
        assert_eq!(
            append_catalog(&mut memory, &catalog),
            AppendStatus::Appended
        );
        assert_eq!(
            append_catalog(&mut memory, &catalog),
            AppendStatus::Duplicate
        );
        let result = retrieve_catalog(
            &memory,
            catalog.kind,
            &catalog.candidate.domain,
            &catalog.source_hash,
        );
        assert_eq!(result.status, CatalogMemoryStatus::Unique);
        assert!(replay_verified(&result));
        assert_eq!(result.records, Some(catalog.records));
        let missing = retrieve_catalog(
            &memory,
            catalog.kind,
            &catalog.candidate.domain,
            "missing-version",
        );
        assert_eq!(missing.status, CatalogMemoryStatus::Missing);
        assert!(replay_verified(&missing));
    }

    #[test]
    fn tampering_with_memory_result_breaks_replay() {
        let catalog = discover_source_catalog(SourceCatalogDocument {
            path: "test.txt",
            document: SOURCE,
        })
        .unwrap();
        let mut memory = CurriculumMemory::new();
        assert_eq!(
            append_catalog(&mut memory, &catalog),
            AppendStatus::Appended
        );
        let mut result = retrieve_catalog(
            &memory,
            catalog.kind,
            &catalog.candidate.domain,
            &catalog.source_hash,
        );
        result.version.push('x');
        assert!(!replay_verified(&result));
    }
}
