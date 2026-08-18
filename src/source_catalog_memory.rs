//! Versioned, replayable storage for source-derived formula catalogs.
//!
//! Catalogs remain immutable data in curriculum memory.  Retrieval requires an
//! exact domain, artifact type, and version; a missing or multiply matching
//! version never falls through to a nearby catalog.

use crate::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};
use crate::source_formula_pack::FormulaRecord;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const ARTIFACT_TYPE: &str = "source_formula_catalog";

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
    pub domain: String,
    pub version: String,
    pub records: Vec<FormulaRecord>,
    pub memory_record_ids: Vec<String>,
    pub provenance: Vec<String>,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn payload(result: &CatalogMemoryResult) -> impl Serialize + '_ {
    (
        result.status,
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
    domain: &str,
    version: &str,
    records: Vec<FormulaRecord>,
    memory_record_ids: Vec<String>,
    provenance: Vec<String>,
    reasons: Vec<String>,
) -> CatalogMemoryResult {
    let replay_hash = digest(&(
        status,
        domain,
        version,
        &records,
        &memory_record_ids,
        &provenance,
        &reasons,
    ));
    CatalogMemoryResult {
        status,
        domain: domain.into(),
        version: version.into(),
        records,
        memory_record_ids,
        provenance,
        reasons,
        replay_hash,
    }
}

/// Append one immutable catalog version to a memory clone.
pub fn append_catalog(
    memory: &mut CurriculumMemory,
    domain: &str,
    version: &str,
    records: &[FormulaRecord],
    provenance: Vec<String>,
) -> AppendStatus {
    if records.is_empty() || provenance.is_empty() || domain.is_empty() || version.is_empty() {
        return AppendStatus::Invalid;
    }
    let Ok(payload) = serde_json::to_string(records) else {
        return AppendStatus::Invalid;
    };
    memory.append(MemoryRecord {
        record_id: format!("formula-catalog::{domain}::{version}"),
        domain: domain.into(),
        artifact_type: ARTIFACT_TYPE.into(),
        version: version.into(),
        payload,
        provenance,
        content_hash: String::new(),
    })
}

/// Retrieve one exact catalog version from immutable curriculum memory.
pub fn retrieve_catalog(
    memory: &CurriculumMemory,
    domain: &str,
    version: &str,
) -> CatalogMemoryResult {
    let candidates = memory.retrieve_exact_version(domain, ARTIFACT_TYPE, version);
    if candidates.is_empty() {
        return finish(
            CatalogMemoryStatus::Missing,
            domain,
            version,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec!["exact source catalog version is absent".into()],
        );
    }
    if candidates.len() != 1 {
        return finish(
            CatalogMemoryStatus::Ambiguous,
            domain,
            version,
            Vec::new(),
            candidates.iter().map(|record| record.record_id.clone()).collect(),
            candidates
                .iter()
                .flat_map(|record| record.provenance.clone())
                .collect(),
            vec!["more than one source catalog matches the exact version".into()],
        );
    }
    let record = candidates[0];
    if !memory.replay_verified(record) {
        return finish(
            CatalogMemoryStatus::Invalid,
            domain,
            version,
            Vec::new(),
            vec![record.record_id.clone()],
            record.provenance.clone(),
            vec!["stored memory receipt failed replay verification".into()],
        );
    }
    let Ok(records) = serde_json::from_str::<Vec<FormulaRecord>>(&record.payload) else {
        return finish(
            CatalogMemoryStatus::Invalid,
            domain,
            version,
            Vec::new(),
            vec![record.record_id.clone()],
            record.provenance.clone(),
            vec!["source catalog payload is not valid typed data".into()],
        );
    };
    if records.is_empty() {
        return finish(
            CatalogMemoryStatus::Invalid,
            domain,
            version,
            Vec::new(),
            vec![record.record_id.clone()],
            record.provenance.clone(),
            vec!["source catalog contains no records".into()],
        );
    }
    finish(
        CatalogMemoryStatus::Unique,
        domain,
        version,
        records,
        vec![record.record_id.clone()],
        record.provenance.clone(),
        Vec::new(),
    )
}

pub fn replay_verified(result: &CatalogMemoryResult) -> bool {
    result.replay_hash == digest(&payload(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_statistics_pack;

    #[test]
    fn exact_version_retrieval_is_replayable_and_fail_closed() {
        let records = source_statistics_pack::records();
        let mut memory = CurriculumMemory::new();
        assert_eq!(
            append_catalog(
                &mut memory,
                source_statistics_pack::DOMAIN,
                "v2",
                &records,
                vec!["openstax-statistics:v2".into()]
            ),
            AppendStatus::Appended
        );
        let found = retrieve_catalog(&memory, source_statistics_pack::DOMAIN, "v2");
        assert_eq!(found.status, CatalogMemoryStatus::Unique);
        assert_eq!(found.records, records);
        assert!(replay_verified(&found));
        let missing = retrieve_catalog(&memory, source_statistics_pack::DOMAIN, "v3");
        assert_eq!(missing.status, CatalogMemoryStatus::Missing);
        assert!(replay_verified(&missing));
        let mut tampered = found.clone();
        tampered.records.clear();
        assert!(!replay_verified(&tampered));
    }
}
