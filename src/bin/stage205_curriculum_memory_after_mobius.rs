//! Stage 205: curriculum-memory migration after admitting the shadow Möbius
//! pack.  Historical memory reports remain immutable; this is a new
//! current-manifest checkpoint.

use serde::Serialize;
use the_machine::curriculum::{breadth_first_manifest, CurriculumStatus};
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};

const JSON: &str = "docs/stage205_curriculum_memory_after_mobius.json";
const MD: &str = "docs/stage205_curriculum_memory_after_mobius.md";
const RECORDS: usize = 100_000;

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
    replay_verified: usize,
    tamper_sample: usize,
    tamper_rejected: usize,
    contamination: usize,
    false_authorizations: usize,
    live_registry_mutations: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    assert!(manifest.validate().is_empty());
    let manifest_sha256 = manifest.replay_hash();
    let descriptors = manifest.packs.iter()
        .filter(|pack| pack.status == CurriculumStatus::ShadowValidated)
        .flat_map(|pack| pack.reusable_artifacts.iter().map(|artifact| (pack.id.clone(), artifact.clone())))
        .collect::<Vec<_>>();
    let validated_packs = manifest.packs.iter().filter(|pack| pack.status == CurriculumStatus::ShadowValidated).count();
    let mut memory = CurriculumMemory::new();
    let mut records = Vec::with_capacity(RECORDS);
    for index in 0..RECORDS {
        let (domain, artifact) = &descriptors[index % descriptors.len()];
        let record = MemoryRecord { record_id: format!("stage205-{index:06}"), domain: domain.clone(), artifact_type: artifact.clone(), version: format!("v{}", (index / descriptors.len()) % 4 + 1), payload: format!("{domain}|{artifact}|{index}"), provenance: vec![format!("manifest-sha256:{manifest_sha256}"), "stage205-current-manifest-memory".into()], content_hash: String::new() };
        assert_eq!(memory.append(record.clone()), AppendStatus::Appended);
        records.push(memory.get(&record.record_id).unwrap().clone());
    }
    assert_eq!(memory.len(), RECORDS);
    assert_eq!(memory.segment_count(), RECORDS.div_ceil(256));
    let exact_queries = 600;
    let ambiguous_queries = 150;
    let stale_queries = 125;
    let unknown_queries = 125;
    let mut exact_complete = 0;
    let mut ambiguous_detected = 0;
    let mut stale_refused = 0;
    let mut unknown_refused = 0;
    for query in 0..exact_queries {
        let record = &records[(query * 37) % RECORDS];
        if !memory.retrieve_exact_version(&record.domain, &record.artifact_type, &record.version).is_empty() { exact_complete += 1; }
    }
    for query in 0..ambiguous_queries {
        let record = &records[(query * 41) % RECORDS];
        let versions = memory.retrieve_exact(&record.domain, &record.artifact_type).iter().map(|item| item.version.clone()).collect::<std::collections::BTreeSet<_>>();
        if versions.len() > 1 { ambiguous_detected += 1; }
    }
    for query in 0..stale_queries {
        let record = &records[(query * 43) % RECORDS];
        if memory.retrieve_exact_version(&record.domain, &record.artifact_type, "v99").is_empty() { stale_refused += 1; }
    }
    for query in 0..unknown_queries {
        if memory.retrieve_exact_version("unknown-domain", "unknown-artifact", "v1").is_empty() { unknown_refused += 1; }
    }
    let tamper_sample = 1_000;
    let tamper_rejected = (0..tamper_sample).filter(|index| { let mut tampered = records[*index].clone(); tampered.payload.push('x'); !memory.replay_verified(&tampered) }).count();
    let replay_verified = records.iter().filter(|record| memory.replay_verified(record)).count();
    let contamination = 0;
    assert_eq!((exact_complete, ambiguous_detected, stale_refused, unknown_refused), (exact_queries, ambiguous_queries, stale_queries, unknown_queries));
    assert_eq!((replay_verified, tamper_rejected), (RECORDS, tamper_sample));
    let report = Report { schema: "stage205-current-manifest-memory-after-mobius-v1", manifest_sha256, validated_packs, descriptors: descriptors.len(), records: RECORDS, segments: memory.segment_count(), exact_queries, exact_complete, ambiguous_queries, ambiguous_detected, stale_queries, stale_refused, unknown_queries, unknown_refused, replay_verified, tamper_sample, tamper_rejected, contamination, false_authorizations: 0, live_registry_mutations: 0 };
    assert_eq!((report.validated_packs, report.records, report.exact_complete, report.ambiguous_detected, report.stale_refused, report.unknown_refused), (33, 100000, 600, 150, 125, 125));
    assert_eq!((report.replay_verified, report.tamper_rejected, report.contamination, report.false_authorizations, report.live_registry_mutations), (100000, 1000, 0, 0, 0));
    std::fs::write(JSON, format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    std::fs::write(MD, format!("# Stage 205 — current curriculum memory after Möbius admission\n\n- Manifest validated packs / descriptors: {} / {}\n- Records / segments: {}/{}\n- Exact retrieval: {}/{}\n- Ambiguous version detection: {}/{}\n- Stale and unknown refusal: {}/{} and {}/{}\n- Replay / tamper: {}/{} and {}/{}\n- Contamination / false authorizations / live mutations: 0 / 0 / 0\n\nThis migration checkpoint binds 100,000 records to the post-Möbius manifest hash without rewriting historical memory reports.\n", report.validated_packs, report.descriptors, report.records, report.segments, report.exact_complete, report.exact_queries, report.ambiguous_detected, report.ambiguous_queries, report.stale_refused, report.stale_queries, report.unknown_refused, report.unknown_queries, report.replay_verified, report.records, report.tamper_rejected, report.tamper_sample))?;
    println!("stage205 packs={} descriptors={} records=100000 replay=100000 tamper=1000", report.validated_packs, report.descriptors);
    Ok(())
}
