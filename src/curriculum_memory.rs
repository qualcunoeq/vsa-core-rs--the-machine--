//! Append-only curriculum-scale memory substrate.
//!
//! Memory stores immutable artifact receipts rather than executable methods.
//! Records are segmented for bounded access, indexed by domain, and protected
//! by deterministic content hashes.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const SEGMENT_CAPACITY: usize = 256;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryRecord {
    pub record_id: String,
    pub domain: String,
    pub artifact_type: String,
    pub version: String,
    pub payload: String,
    pub provenance: Vec<String>,
    pub content_hash: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AppendStatus {
    Appended,
    Duplicate,
    Invalid,
}

#[derive(Debug, Clone, Default)]
pub struct CurriculumMemory {
    segments: Vec<Vec<MemoryRecord>>,
    ids: BTreeSet<String>,
    domain_index: BTreeMap<String, Vec<String>>,
    record_index: BTreeMap<String, (usize, usize)>,
}

fn payload(record: &MemoryRecord) -> impl Serialize + '_ {
    (
        &record.record_id,
        &record.domain,
        &record.artifact_type,
        &record.version,
        &record.payload,
        &record.provenance,
    )
}

pub fn record_hash(record: &MemoryRecord) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&payload(record)).unwrap())
    )
}

impl CurriculumMemory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn append(&mut self, mut record: MemoryRecord) -> AppendStatus {
        if record.record_id.is_empty()
            || record.domain.is_empty()
            || record.artifact_type.is_empty()
            || record.version.is_empty()
            || record.provenance.is_empty()
        {
            return AppendStatus::Invalid;
        }
        if self.ids.contains(&record.record_id) {
            return AppendStatus::Duplicate;
        }
        let expected_hash = record_hash(&record);
        if !record.content_hash.is_empty() && record.content_hash != expected_hash {
            return AppendStatus::Invalid;
        }
        record.content_hash = expected_hash;
        if self
            .segments
            .last()
            .is_none_or(|segment| segment.len() >= SEGMENT_CAPACITY)
        {
            self.segments.push(Vec::with_capacity(SEGMENT_CAPACITY));
        }
        self.ids.insert(record.record_id.clone());
        self.domain_index
            .entry(record.domain.clone())
            .or_default()
            .push(record.record_id.clone());
        let segment_index = self.segments.len() - 1;
        let entry_index = self.segments[segment_index].len();
        self.segments[segment_index].push(record);
        self.record_index.insert(
            self.segments[segment_index][entry_index].record_id.clone(),
            (segment_index, entry_index),
        );
        AppendStatus::Appended
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    pub fn retrieve_domain(&self, domain: &str) -> Vec<&MemoryRecord> {
        let Some(ids) = self.domain_index.get(domain) else {
            return Vec::new();
        };
        ids.iter().filter_map(|id| self.get(id)).collect()
    }

    /// Retrieve only records matching both semantic dimensions.  Exact
    /// filtering prevents a broad domain hit from contaminating a typed
    /// planner with unrelated artifacts.
    pub fn retrieve_exact(&self, domain: &str, artifact_type: &str) -> Vec<&MemoryRecord> {
        self.retrieve_domain(domain)
            .into_iter()
            .filter(|record| record.artifact_type == artifact_type)
            .collect()
    }

    /// Retrieve an exact typed artifact at an explicit immutable version.
    pub fn retrieve_exact_version(
        &self,
        domain: &str,
        artifact_type: &str,
        version: &str,
    ) -> Vec<&MemoryRecord> {
        self.retrieve_exact(domain, artifact_type)
            .into_iter()
            .filter(|record| record.version == version)
            .collect()
    }

    pub fn get(&self, record_id: &str) -> Option<&MemoryRecord> {
        let (segment_index, entry_index) = self.record_index.get(record_id).copied()?;
        self.segments
            .get(segment_index)
            .and_then(|segment| segment.get(entry_index))
    }

    pub fn replay_verified(&self, record: &MemoryRecord) -> bool {
        self.get(&record.record_id)
            .is_some_and(|stored| stored == record && record_hash(record) == record.content_hash)
    }

    pub fn all_records(&self) -> impl Iterator<Item = &MemoryRecord> {
        self.segments.iter().flat_map(|segment| segment.iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str) -> MemoryRecord {
        MemoryRecord {
            record_id: id.into(),
            domain: "test".into(),
            artifact_type: "scalar".into(),
            version: "v1".into(),
            payload: "42".into(),
            provenance: vec!["test-source".into()],
            content_hash: String::new(),
        }
    }

    #[test]
    fn append_retrieve_and_tamper_are_deterministic() {
        let mut memory = CurriculumMemory::new();
        assert_eq!(memory.append(record("a")), AppendStatus::Appended);
        assert_eq!(memory.append(record("a")), AppendStatus::Duplicate);
        let stored = memory.get("a").unwrap().clone();
        assert!(memory.replay_verified(&stored));
        let mut tampered = stored.clone();
        tampered.payload = "43".into();
        assert!(!memory.replay_verified(&tampered));
    }
}
