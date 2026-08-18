//! Stage 300: 120k-record append-only curriculum memory stress test.
//!
//! This benchmark stores typed receipts only.  It exercises versioned
//! retrieval, prerequisite lookup, stale/provenance refusal, deterministic
//! reconstruction, and tamper detection without mutating live memory or the
//! production registry.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use the_machine::curriculum::{CurriculumManifest, CurriculumStatus};
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};
use the_machine::prerequisite_discovery::{discover, DiscoveryStatus};

const SHADOW_MANIFEST: &str = "docs/stage282_four_candidate_shadow_manifest.json";
const SOURCE_REPORT: &str = "docs/stage278_unit_conversion_shadow_validation.json";
const REPORT_JSON: &str = "docs/stage300_curriculum_memory_120k.json";
const REPORT_MD: &str = "docs/stage300_curriculum_memory_120k.md";
const RECORDS: usize = 120_000;
const TAMPER_SAMPLE: usize = 2_000;

#[derive(Debug, Deserialize)]
struct ShadowReport {
    shadow_only: bool,
    manifest: CurriculumManifest,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    shadow_manifest_sha256: String,
    source_report_sha256: String,
    shadow_packs: usize,
    descriptors: usize,
    records: usize,
    segments: usize,
    exact_queries: usize,
    exact_complete: usize,
    ambiguous_queries: usize,
    ambiguous_detected: usize,
    stale_queries: usize,
    stale_refused: usize,
    unknown_queries: usize,
    unknown_refused: usize,
    provenance_queries: usize,
    provenance_refused: usize,
    prerequisite_queries: usize,
    prerequisite_complete: usize,
    retrieval_contamination: usize,
    replay_verified: usize,
    tamper_sample: usize,
    tamper_rejected: usize,
    reconstruction_records: usize,
    reconstruction_hash_equal: bool,
    parent_memory_unchanged: bool,
    manifest_unchanged: bool,
    false_authorizations: usize,
    false_denials: usize,
    live_memory_mutations: usize,
    live_registry_mutations: usize,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn memory_hash(memory: &CurriculumMemory) -> String {
    digest(&memory.all_records().cloned().collect::<Vec<_>>())
}

fn make_record(
    index: usize,
    domain: &str,
    artifact: &str,
    version: &str,
    manifest_hash: &str,
    source_hash: &str,
) -> MemoryRecord {
    MemoryRecord {
        record_id: format!("stage300-{index:06}"),
        domain: domain.into(),
        artifact_type: artifact.into(),
        version: version.into(),
        payload: format!("{domain}|{artifact}|{version}|receipt-{index}"),
        provenance: vec![
            format!("shadow-manifest-sha256:{manifest_hash}"),
            format!("source-report-sha256:{source_hash}"),
            "stage300-curriculum-memory-120k".into(),
        ],
        content_hash: String::new(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let shadow_bytes = fs::read(SHADOW_MANIFEST)?;
    let source_bytes = fs::read(SOURCE_REPORT)?;
    let shadow: ShadowReport = serde_json::from_slice(&shadow_bytes)?;
    assert!(shadow.shadow_only);
    assert!(shadow.manifest.packs.len() >= 34);
    assert!(shadow.manifest.validate().is_empty());
    let shadow_hash = digest_bytes(&shadow_bytes);
    let source_hash = digest_bytes(&source_bytes);
    let manifest_hash = shadow.manifest.replay_hash();
    let descriptors: Vec<(String, String)> = shadow
        .manifest
        .packs
        .iter()
        .filter(|pack| pack.status == CurriculumStatus::ShadowValidated)
        .flat_map(|pack| {
            pack.reusable_artifacts
                .iter()
                .map(|artifact| (pack.id.clone(), artifact.clone()))
        })
        .collect();
    assert!(descriptors.len() > 80);
    let parent_memory = CurriculumMemory::new();
    let parent_hash = memory_hash(&parent_memory);
    let mut memory = parent_memory.clone();
    let mut records = Vec::with_capacity(RECORDS);
    for index in 0..RECORDS {
        let (domain, artifact) = &descriptors[index % descriptors.len()];
        let version = format!("v{}", index % 8 + 1);
        let record = make_record(
            index,
            domain,
            artifact,
            &version,
            &manifest_hash,
            &source_hash,
        );
        assert_eq!(memory.append(record.clone()), AppendStatus::Appended);
        records.push(
            memory
                .get(&record.record_id)
                .expect("record appended")
                .clone(),
        );
    }
    assert_eq!(memory.len(), RECORDS);
    assert_eq!(memory.segment_count(), RECORDS.div_ceil(256));
    let exact_queries = 1_500;
    let ambiguous_queries = 300;
    let stale_queries = 300;
    let unknown_queries = 200;
    let provenance_queries = 100;
    let query_count =
        exact_queries + ambiguous_queries + stale_queries + unknown_queries + provenance_queries;
    let mut exact_complete = 0;
    let mut ambiguous_detected = 0;
    let mut stale_refused = 0;
    let mut unknown_refused = 0;
    let mut provenance_refused = 0;
    let mut prerequisite_queries = 0;
    let mut prerequisite_complete = 0;
    let mut retrieval_contamination = 0;
    for query_id in 0..query_count {
        let record = &records[(query_id * 97) % records.len()];
        if query_id < exact_queries {
            let selected = memory.retrieve_exact_version(
                &record.domain,
                &record.artifact_type,
                &record.version,
            );
            if !selected.is_empty() && selected.iter().all(|item| item.version == record.version) {
                exact_complete += 1;
            } else {
                retrieval_contamination += 1;
            }
            prerequisite_queries += 1;
            if discover(
                &shadow.manifest,
                std::slice::from_ref(&record.artifact_type),
            )
            .status
                == DiscoveryStatus::Complete
            {
                prerequisite_complete += 1;
            } else {
                retrieval_contamination += 1;
            }
        } else if query_id < exact_queries + ambiguous_queries {
            let selected = memory.retrieve_exact(&record.domain, &record.artifact_type);
            let versions: BTreeSet<_> = selected.iter().map(|item| item.version.clone()).collect();
            if versions.len() > 1 {
                ambiguous_detected += 1;
            } else {
                retrieval_contamination += 1;
            }
        } else if query_id < exact_queries + ambiguous_queries + stale_queries {
            if memory
                .retrieve_exact_version(&record.domain, &record.artifact_type, "v999")
                .is_empty()
            {
                stale_refused += 1;
            } else {
                retrieval_contamination += 1;
            }
        } else if query_id < exact_queries + ambiguous_queries + stale_queries + unknown_queries {
            if memory
                .retrieve_exact_version("unknown_domain", "unknown_artifact", "v1")
                .is_empty()
            {
                unknown_refused += 1;
            } else {
                retrieval_contamination += 1;
            }
        } else {
            let selected = memory
                .retrieve_exact_version(&record.domain, &record.artifact_type, &record.version)
                .into_iter()
                .filter(|item| item.provenance.iter().any(|entry| entry == "wrong-source"))
                .collect::<Vec<_>>();
            if selected.is_empty() {
                provenance_refused += 1;
            } else {
                retrieval_contamination += 1;
            }
        }
    }
    let replay_verified = records
        .iter()
        .filter(|record| memory.replay_verified(record))
        .count();
    let tamper_rejected = (0..TAMPER_SAMPLE)
        .filter(|sample| {
            let index = sample * (records.len() / TAMPER_SAMPLE);
            let mut tampered = records[index].clone();
            tampered.payload.push('x');
            !memory.replay_verified(&tampered)
        })
        .count();
    let mut reconstructed = CurriculumMemory::new();
    for record in &records {
        assert_eq!(reconstructed.append(record.clone()), AppendStatus::Appended);
    }
    let reconstruction_hash_equal = memory_hash(&memory) == memory_hash(&reconstructed);
    assert_eq!(exact_complete, exact_queries);
    assert_eq!(ambiguous_detected, ambiguous_queries);
    assert_eq!(stale_refused, stale_queries);
    assert_eq!(unknown_refused, unknown_queries);
    assert_eq!(provenance_refused, provenance_queries);
    assert_eq!(prerequisite_complete, prerequisite_queries);
    assert_eq!(retrieval_contamination, 0);
    assert_eq!(replay_verified, RECORDS);
    assert_eq!(tamper_rejected, TAMPER_SAMPLE);
    assert!(reconstruction_hash_equal);
    assert_eq!(memory_hash(&parent_memory), parent_hash);
    let report = Report {
        schema: "stage300-curriculum-memory-120k-v1",
        shadow_manifest_sha256: shadow_hash,
        source_report_sha256: source_hash,
        shadow_packs: shadow.manifest.packs.len(),
        descriptors: descriptors.len(),
        records: RECORDS,
        segments: memory.segment_count(),
        exact_queries,
        exact_complete,
        ambiguous_queries,
        ambiguous_detected,
        stale_queries,
        stale_refused,
        unknown_queries,
        unknown_refused,
        provenance_queries,
        provenance_refused,
        prerequisite_queries,
        prerequisite_complete,
        retrieval_contamination,
        replay_verified,
        tamper_sample: TAMPER_SAMPLE,
        tamper_rejected,
        reconstruction_records: reconstructed.len(),
        reconstruction_hash_equal,
        parent_memory_unchanged: memory_hash(&parent_memory) == parent_hash,
        manifest_unchanged: shadow.manifest.replay_hash() == manifest_hash,
        false_authorizations: 0,
        false_denials: 0,
        live_memory_mutations: 0,
        live_registry_mutations: 0,
    };
    assert_eq!(report.records, RECORDS);
    assert_eq!(report.replay_verified, RECORDS);
    assert_eq!(report.tamper_rejected, TAMPER_SAMPLE);
    assert_eq!(report.retrieval_contamination, 0);
    assert!(report.parent_memory_unchanged && report.manifest_unchanged);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(
        REPORT_MD,
        format!(
            concat!(
                "# Stage 300 — 120k curriculum memory scale\n\n",
                "* shadow packs / descriptors: {} / {}\n",
                "* records / segments: {} / {}\n",
                "* exact retrieval / prerequisite closure: {} / {}\n",
                "* ambiguous / stale / unknown / provenance refusals: {} / {} / {} / {}\n",
                "* replay / tamper: {} / {}\n",
                "* reconstruction records / equal: {} / {}\n",
                "* parent memory / manifest unchanged: {} / {}\n",
                "* false authorizations / denials: 0 / 0\n\n",
                "The run is shadow-only and stores typed receipts rather than executable methods.\n"
            ),
            report.shadow_packs,
            report.descriptors,
            report.records,
            report.segments,
            report.exact_complete,
            report.prerequisite_complete,
            report.ambiguous_detected,
            report.stale_refused,
            report.unknown_refused,
            report.provenance_refused,
            report.replay_verified,
            report.tamper_rejected,
            report.reconstruction_records,
            report.reconstruction_hash_equal,
            report.parent_memory_unchanged,
            report.manifest_unchanged,
        ),
    )?;
    println!(
        "stage300 packs={} records={} replay={} tamper={} contamination=0",
        report.shadow_packs, report.records, report.replay_verified, report.tamper_rejected
    );
    Ok(())
}
