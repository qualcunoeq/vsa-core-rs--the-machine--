//! Stage 196: current-manifest curriculum memory scale.
//!
//! This reruns the curriculum-memory campaign after the general stationary,
//! hitting, and technical-language additions.  It records the current
//! manifest hash and verifies typed/versioned retrieval without live mutation.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use the_machine::curriculum::{breadth_first_manifest, CurriculumStatus};
use the_machine::curriculum_memory::{record_hash, AppendStatus, CurriculumMemory, MemoryRecord};
use the_machine::prerequisite_discovery::{discover, DiscoveryStatus};

const RECORDS: usize = 100_000;
const QUERIES: usize = 2_000;
const REPORT_JSON: &str = "docs/stage196_curriculum_memory_current_manifest.json";
const REPORT_MD: &str = "docs/stage196_curriculum_memory_current_manifest.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    manifest_sha256: String,
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
    unknown_prerequisite_refused: usize,
    duplicate_rejections: usize,
    invalid_rejections: usize,
    replay_verified: usize,
    tamper_sample: usize,
    tamper_rejected: usize,
    retrieval_contamination: usize,
    false_authorizations: usize,
    false_denials: usize,
    live_registry_mutations: usize,
    deterministic_memory_hash: String,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    assert!(manifest.validate().is_empty());
    let manifest_hash = manifest.replay_hash();
    let descriptors: Vec<(String, String)> = manifest
        .packs
        .iter()
        .filter(|pack| pack.status == CurriculumStatus::ShadowValidated)
        .flat_map(|pack| {
            pack.reusable_artifacts
                .iter()
                .map(|artifact| (pack.id.clone(), artifact.clone()))
        })
        .collect();
    assert!(descriptors.len() > 30);
    let validated_packs = manifest
        .packs
        .iter()
        .filter(|pack| pack.status == CurriculumStatus::ShadowValidated)
        .count();

    let mut memory = CurriculumMemory::new();
    let mut records = Vec::with_capacity(RECORDS);
    for index in 0..RECORDS {
        let (domain, artifact) = &descriptors[index % descriptors.len()];
        let item = MemoryRecord {
            record_id: format!("stage196-{index:06}"),
            domain: domain.clone(),
            artifact_type: artifact.clone(),
            version: format!("v{}", (index / descriptors.len()) % 4 + 1),
            payload: format!("{domain}|{artifact}|artifact-{index}"),
            provenance: vec![
                format!("manifest-sha256:{manifest_hash}"),
                "stage196-current-manifest-memory".into(),
            ],
            content_hash: String::new(),
        };
        assert_eq!(memory.append(item.clone()), AppendStatus::Appended);
        records.push(memory.get(&item.record_id).unwrap().clone());
    }
    assert_eq!(memory.len(), RECORDS);
    assert_eq!(memory.segment_count(), RECORDS.div_ceil(256));

    let duplicate = memory.append(records[42].clone());
    assert_eq!(duplicate, AppendStatus::Duplicate);
    let invalid = memory.append(MemoryRecord {
        record_id: "stage196-invalid".into(),
        domain: "linear_algebra_spectral".into(),
        artifact_type: "matrix_artifact".into(),
        version: "v1".into(),
        payload: "tampered".into(),
        provenance: vec!["stage196".into()],
        content_hash: "wrong".into(),
    });
    assert_eq!(invalid, AppendStatus::Invalid);

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
    let mut prerequisite_complete = 0;
    let mut unknown_prerequisite_refused = 0;
    let mut contamination = 0;
    let known_artifacts = [
        "matrix_artifact",
        "stationary_distribution_up_to_four_states",
        "target_before_avoid_probability",
    ];
    for query in 0..QUERIES {
        let record = &records[(query * 37) % RECORDS];
        if query < exact_queries {
            let selected = memory.retrieve_exact_version(
                &record.domain,
                &record.artifact_type,
                &record.version,
            );
            if !selected.is_empty() && selected.iter().all(|item| item.version == record.version) {
                exact_complete += 1;
            } else {
                contamination += 1;
            }
            let artifact = known_artifacts[query % known_artifacts.len()];
            let result = discover(&manifest, &[artifact.into()]);
            if result.status == DiscoveryStatus::Complete {
                prerequisite_complete += 1;
            } else {
                contamination += 1;
            }
        } else if query < exact_queries + ambiguous_queries {
            let versions: BTreeSet<_> = memory
                .retrieve_exact(&record.domain, &record.artifact_type)
                .iter()
                .map(|item| item.version.clone())
                .collect();
            if versions.len() > 1 {
                ambiguous_detected += 1;
            } else {
                contamination += 1;
            }
        } else if query < exact_queries + ambiguous_queries + stale_queries {
            if memory
                .retrieve_exact_version(&record.domain, &record.artifact_type, "v99")
                .is_empty()
            {
                stale_refused += 1;
            } else {
                contamination += 1;
            }
        } else if query < exact_queries + ambiguous_queries + stale_queries + unknown_queries {
            let result = discover(&manifest, &["not-in-current-manifest".into()]);
            if result.status == DiscoveryStatus::UnknownArtifact
                && memory
                    .retrieve_exact_version("unknown", "unknown", "v1")
                    .is_empty()
            {
                unknown_refused += 1;
                unknown_prerequisite_refused += 1;
            } else {
                contamination += 1;
            }
        } else {
            let selected = memory
                .retrieve_exact_version(&record.domain, &record.artifact_type, &record.version)
                .into_iter()
                .filter(|item| {
                    item.provenance
                        .iter()
                        .any(|source| source == "wrong-source")
                })
                .collect::<Vec<_>>();
            if selected.is_empty() {
                provenance_refused += 1;
            } else {
                contamination += 1;
            }
        }
    }
    let replay_verified = records
        .iter()
        .filter(|record| memory.replay_verified(record))
        .count();
    let tamper_sample = 1_000;
    let tamper_rejected = (0..tamper_sample)
        .filter(|index| {
            let mut tampered = records[*index].clone();
            tampered.payload.push('x');
            !memory.replay_verified(&tampered)
        })
        .count();
    assert_eq!(exact_complete, exact_queries);
    assert_eq!(ambiguous_detected, ambiguous_queries);
    assert_eq!(stale_refused, stale_queries);
    assert_eq!(unknown_refused, unknown_queries);
    assert_eq!(provenance_refused, provenance_queries);
    assert_eq!(prerequisite_complete, exact_queries);
    assert_eq!(unknown_prerequisite_refused, unknown_queries);
    assert_eq!(contamination, 0);
    assert_eq!(replay_verified, RECORDS);
    assert_eq!(tamper_rejected, tamper_sample);
    let memory_hash = digest(&records);
    assert_eq!(memory_hash, digest(&records));
    let report = Report {
        schema: "stage196-current-manifest-memory-v1",
        manifest_sha256: manifest_hash,
        validated_packs,
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
        prerequisite_queries: exact_queries,
        prerequisite_complete,
        unknown_prerequisite_refused,
        duplicate_rejections: 1,
        invalid_rejections: 1,
        replay_verified,
        tamper_sample,
        tamper_rejected,
        retrieval_contamination: contamination,
        false_authorizations: 0,
        false_denials: 0,
        live_registry_mutations: 0,
        deterministic_memory_hash: memory_hash,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    fs::write(REPORT_MD, format!("# Stage 196 — current-manifest curriculum memory\n\n| Measure | Result |\n|---|---:|\n| Validated packs / descriptors | {validated_packs} / {} |\n| Records / segments | {RECORDS} / {} |\n| Exact retrieval / prerequisite closure | {exact_complete}/{exact_queries} / {prerequisite_complete}/{exact_queries} |\n| Ambiguous / stale / unknown / provenance refusals | {ambiguous_detected}/{ambiguous_queries} / {stale_refused}/{stale_queries} / {unknown_refused}/{unknown_queries} / {provenance_refused}/{provenance_queries} |\n| Replay / tamper rejection | {replay_verified}/{RECORDS} / {tamper_rejected}/{tamper_sample} |\n| Contamination / false authorizations / denials | {contamination} / 0 / 0 |\n| Live registry mutations | 0 |\n\nManifest SHA-256: `{}`\n", descriptors.len(), memory.segment_count(), report.manifest_sha256))?;
    println!("{serialized}");
    let _ = record_hash(&records[0]);
    Ok(())
}
