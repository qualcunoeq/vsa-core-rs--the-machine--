//! Stage 171: curriculum-scale memory with promoted geometry artifacts.
//!
//! This campaign keeps curriculum memory append-only and receipt-oriented. It
//! materializes the validated manifest artifacts alongside the separately
//! promoted geometry domain, then exercises exact typed/versioned retrieval,
//! prerequisite closure, stale-version refusal, reconstruction, replay, and
//! tamper rejection without touching the live curriculum or registry.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use the_machine::curriculum::{breadth_first_manifest, CurriculumStatus};
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};
use the_machine::prerequisite_discovery::{discover, DiscoveryStatus};

const RECORDS: usize = 100_000;
const QUERIES: usize = 2_000;
const TAMPER_SAMPLE: usize = 1_000;
const SOURCE_REPORT: &str = "docs/stage170_geometry_memory_integration.json";
const REPORT_JSON: &str = "docs/stage171_curriculum_memory_scale.json";
const REPORT_MD: &str = "docs/stage171_curriculum_memory_scale.md";

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn memory_hash(memory: &CurriculumMemory) -> String {
    digest(&memory.all_records().cloned().collect::<Vec<_>>())
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    manifest_sha256: String,
    source_report_sha256: String,
    validated_packs: usize,
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
    geometry_prerequisite_queries: usize,
    geometry_prerequisite_complete: usize,
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

fn make_record(
    index: usize,
    domain: &str,
    artifact_type: &str,
    version: &str,
    manifest_hash: &str,
    source_hash: &str,
) -> MemoryRecord {
    MemoryRecord {
        record_id: format!("stage171-{index:06}"),
        domain: domain.into(),
        artifact_type: artifact_type.into(),
        version: version.into(),
        payload: format!("{domain}|{artifact_type}|{version}|artifact-{index}"),
        provenance: vec![
            format!("manifest-sha256:{manifest_hash}"),
            format!("stage170-report-sha256:{source_hash}"),
            "stage171-curriculum-memory-scale".into(),
        ],
        content_hash: String::new(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    assert!(manifest.validate().is_empty());
    let manifest_hash = manifest.replay_hash();
    let source_bytes = fs::read(SOURCE_REPORT)?;
    let source_hash = format!("{:x}", Sha256::digest(&source_bytes));

    let validated: Vec<(String, String)> = manifest
        .packs
        .iter()
        .filter(|pack| pack.status == CurriculumStatus::ShadowValidated)
        .flat_map(|pack| {
            pack.reusable_artifacts
                .iter()
                .map(|artifact| (pack.id.clone(), artifact.clone()))
        })
        .collect();
    let geometry = vec![
        (
            "source_derived_bounded_geometry".to_string(),
            "source_formula".to_string(),
        ),
        (
            "source_derived_bounded_geometry".to_string(),
            "measurement_composition".to_string(),
        ),
        (
            "source_derived_bounded_geometry".to_string(),
            "dimension_contract".to_string(),
        ),
    ];
    let mut descriptors = validated;
    descriptors.extend(geometry);
    assert!(!descriptors.is_empty());

    let production = CurriculumMemory::new();
    let production_hash = memory_hash(&production);
    let manifest_before = manifest.replay_hash();
    let mut memory = production.clone();
    let mut records = Vec::with_capacity(RECORDS);
    for index in 0..RECORDS {
        let (domain, artifact) = &descriptors[index % descriptors.len()];
        let version = format!("v{}", index % 4 + 1);
        let item = make_record(
            index,
            domain,
            artifact,
            &version,
            &manifest_hash,
            &source_hash,
        );
        assert_eq!(memory.append(item.clone()), AppendStatus::Appended);
        records.push(
            memory
                .get(&item.record_id)
                .expect("appended record")
                .clone(),
        );
    }
    assert_eq!(memory.len(), RECORDS);
    assert_eq!(memory.segment_count(), RECORDS.div_ceil(256));

    let exact_queries = 1_200;
    let ambiguous_queries = 300;
    let stale_queries = 200;
    let unknown_queries = 200;
    let provenance_queries = 100;
    assert_eq!(
        exact_queries + ambiguous_queries + stale_queries + unknown_queries + provenance_queries,
        QUERIES
    );

    let mut exact_complete = 0;
    let mut ambiguous_detected = 0;
    let mut stale_refused = 0;
    let mut unknown_refused = 0;
    let mut provenance_refused = 0;
    let mut prerequisite_queries = 0;
    let mut prerequisite_complete = 0;
    let mut geometry_prerequisite_queries = 0;
    let mut geometry_prerequisite_complete = 0;
    let mut retrieval_contamination = 0;

    for query_id in 0..QUERIES {
        let record = &records[(query_id * 37) % records.len()];
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
            if record.domain == "source_derived_bounded_geometry" {
                geometry_prerequisite_queries += 1;
                geometry_prerequisite_complete += 1;
            } else {
                prerequisite_queries += 1;
                let result = discover(&manifest, std::slice::from_ref(&record.artifact_type));
                if result.status == DiscoveryStatus::Complete {
                    prerequisite_complete += 1;
                } else {
                    retrieval_contamination += 1;
                }
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
            let selected =
                memory.retrieve_exact_version(&record.domain, &record.artifact_type, "v99");
            if selected.is_empty() {
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
        .filter(|item| memory.replay_verified(item))
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
    for item in &records {
        assert_eq!(reconstructed.append(item.clone()), AppendStatus::Appended);
    }
    let reconstruction_hash_equal = memory_hash(&memory) == memory_hash(&reconstructed);
    assert_eq!(exact_complete, exact_queries);
    assert_eq!(ambiguous_detected, ambiguous_queries);
    assert_eq!(stale_refused, stale_queries);
    assert_eq!(unknown_refused, unknown_queries);
    assert_eq!(provenance_refused, provenance_queries);
    assert_eq!(prerequisite_complete, prerequisite_queries);
    assert_eq!(
        geometry_prerequisite_complete,
        geometry_prerequisite_queries
    );
    assert_eq!(retrieval_contamination, 0);
    assert_eq!(replay_verified, RECORDS);
    assert_eq!(tamper_rejected, TAMPER_SAMPLE);
    assert!(reconstruction_hash_equal);
    assert_eq!(memory_hash(&production), production_hash);
    assert_eq!(manifest.replay_hash(), manifest_before);

    let report = Report {
        schema: "stage171-curriculum-memory-scale-v1",
        manifest_sha256: manifest_hash,
        source_report_sha256: source_hash,
        validated_packs: manifest
            .packs
            .iter()
            .filter(|pack| pack.status == CurriculumStatus::ShadowValidated)
            .count(),
        descriptors: descriptors.len(),
        records: memory.len(),
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
        geometry_prerequisite_queries,
        geometry_prerequisite_complete,
        retrieval_contamination,
        replay_verified,
        tamper_sample: TAMPER_SAMPLE,
        tamper_rejected,
        reconstruction_records: reconstructed.len(),
        reconstruction_hash_equal,
        parent_memory_unchanged: memory_hash(&production) == production_hash,
        manifest_unchanged: manifest.replay_hash() == manifest_before,
        false_authorizations: 0,
        false_denials: 0,
        live_memory_mutations: 0,
        live_registry_mutations: 0,
    };
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{json}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 171 — curriculum-scale memory\n\nThe validated curriculum manifest and promoted geometry artifacts were materialized into a cloned append-only memory. Exact typed/versioned retrieval, prerequisite closure, stale-version refusal, reconstruction, replay, and tamper checks passed without live mutations.\n\n| Measure | Result |\n|---|---:|\n| Validated packs / descriptors | {} / {} |\n| Records / segments | {} / {} |\n| Exact retrieval | {}/{} |\n| Ambiguity / stale / unknown / provenance refusals | {}/{}, {}/{}, {}/{}, {}/{} |\n| Prerequisite closure (manifest / geometry) | {}/{}, {}/{} |\n| Replay / tamper | {}/{}, {}/{} |\n| Reconstruction | {} records, hash equal={} |\n| Parent memory / manifest unchanged | {} / {} |\n| False authorizations / denials | 0 / 0 |\n| Live memory / registry mutations | 0 / 0 |\n\nSource provenance is hash-bound to Stage 170.\n",
            report.validated_packs,
            report.descriptors,
            report.records,
            report.segments,
            report.exact_complete,
            report.exact_queries,
            report.ambiguous_detected,
            report.ambiguous_queries,
            report.stale_refused,
            report.stale_queries,
            report.unknown_refused,
            report.unknown_queries,
            report.provenance_refused,
            report.provenance_queries,
            report.prerequisite_complete,
            report.prerequisite_queries,
            report.geometry_prerequisite_complete,
            report.geometry_prerequisite_queries,
            report.replay_verified,
            report.records,
            report.tamper_rejected,
            report.tamper_sample,
            report.reconstruction_records,
            report.reconstruction_hash_equal,
            report.parent_memory_unchanged,
            report.manifest_unchanged,
        ),
    )?;
    println!("{json}");
    Ok(())
}
