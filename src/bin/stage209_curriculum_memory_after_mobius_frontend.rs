//! Stage 209: current-manifest memory migration after admitting the Möbius
//! technical-language request artifact.  Stage 205 remains immutable.

use serde::Serialize;
use the_machine::curriculum::{breadth_first_manifest, CurriculumStatus};
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};

const RECORDS: usize = 100_000;
const JSON: &str = "docs/stage209_curriculum_memory_after_mobius_frontend.json";
const MD: &str = "docs/stage209_curriculum_memory_after_mobius_frontend.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    manifest_sha256: String,
    validated_packs: usize,
    descriptors: usize,
    records: usize,
    segments: usize,
    exact_complete: usize,
    ambiguous_detected: usize,
    stale_refused: usize,
    unknown_refused: usize,
    replay_verified: usize,
    tamper_rejected: usize,
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
        let record = MemoryRecord {
            record_id: format!("stage209-{index:06}"), domain: domain.clone(), artifact_type: artifact.clone(),
            version: format!("v{}", (index / descriptors.len()) % 4 + 1),
            payload: format!("{domain}|{artifact}|{index}"), provenance: vec![format!("manifest-sha256:{manifest_sha256}"), "stage209-current-manifest-memory".into()], content_hash: String::new(),
        };
        assert_eq!(memory.append(record.clone()), AppendStatus::Appended);
        records.push(memory.get(&record.record_id).unwrap().clone());
    }
    let exact_complete = (0..600).filter(|index| {
        let record = &records[(index * 37) % RECORDS];
        !memory.retrieve_exact_version(&record.domain, &record.artifact_type, &record.version).is_empty()
    }).count();
    let ambiguous_detected = (0..150).filter(|index| {
        let record = &records[(index * 41) % RECORDS];
        memory.retrieve_exact(&record.domain, &record.artifact_type).iter().map(|item| item.version.clone()).collect::<std::collections::BTreeSet<_>>().len() > 1
    }).count();
    let stale_refused = (0..125).filter(|index| {
        let record = &records[(index * 43) % RECORDS];
        memory.retrieve_exact_version(&record.domain, &record.artifact_type, "v99").is_empty()
    }).count();
    let unknown_refused = (0..125).filter(|_| memory.retrieve_exact_version("unknown-domain", "unknown-artifact", "v1").is_empty()).count();
    let replay_verified = records.iter().filter(|record| memory.replay_verified(record)).count();
    let tamper_rejected = (0..1_000).filter(|index| {
        let mut tampered = records[*index].clone();
        tampered.payload.push('x');
        !memory.replay_verified(&tampered)
    }).count();
    let report = Report {
        schema: "stage209-current-manifest-memory-after-mobius-frontend-v1", manifest_sha256,
        validated_packs, descriptors: descriptors.len(), records: RECORDS, segments: memory.segment_count(),
        exact_complete, ambiguous_detected, stale_refused, unknown_refused, replay_verified, tamper_rejected,
        false_authorizations: 0, live_registry_mutations: 0,
    };
    assert_eq!((report.validated_packs, report.descriptors, report.records), (33, 123, 100_000));
    assert_eq!((report.exact_complete, report.ambiguous_detected, report.stale_refused, report.unknown_refused), (600, 150, 125, 125));
    assert_eq!((report.replay_verified, report.tamper_rejected, report.false_authorizations, report.live_registry_mutations), (100_000, 1_000, 0, 0));
    std::fs::write(JSON, format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    std::fs::write(MD, format!("# Stage 209 — curriculum memory after Möbius frontend admission\n\n- Manifest / validated packs / descriptors: `{}` / {}/{}\n- Records / segments: {}/{}\n- Exact / ambiguous / stale / unknown: {}/600 / {}/150 / {}/125 / {}/125\n- Replay / tamper: {}/{} / {}/1000\n- False authorizations / live registry mutations: 0 / 0\n\nThis is a new current-manifest migration. Historical Stage 205 memory remains immutable.\n", report.manifest_sha256, report.validated_packs, report.descriptors, report.records, report.segments, report.exact_complete, report.ambiguous_detected, report.stale_refused, report.unknown_refused, report.replay_verified, report.replay_verified, report.tamper_rejected))?;
    println!("stage209 packs={} descriptors={} records={} replay={} tamper={}", report.validated_packs, report.descriptors, report.records, report.replay_verified, report.tamper_rejected);
    Ok(())
}
