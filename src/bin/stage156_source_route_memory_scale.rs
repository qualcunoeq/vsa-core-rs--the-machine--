//! Stage 156: long-term memory scale for promoted source/multimodal receipts.
//!
//! The memory layer stores immutable receipt records, not executable methods.
//! This campaign seeds it from the sealed Stage 155 report, exercises exact
//! route/version retrieval at scale, reconstructs an equivalent memory, and
//! verifies duplicate and tamper rejection without changing any registry.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::curriculum_memory::{record_hash, AppendStatus, CurriculumMemory, MemoryRecord};

const SOURCE_REPORT: &str = "docs/stage155_sealed_source_multimodal_exam.json";
const RECORDS: usize = 100_000;

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn field(report: &Value, name: &str) -> usize {
    report.get(name).and_then(Value::as_u64).unwrap_or_default() as usize
}

fn record(index: usize, source_hash: &str) -> MemoryRecord {
    let family = match index % 6 {
        0 => "statistics",
        1 => "biology",
        2 => "chemistry",
        3 => "visual_statistics",
        4 => "visual_biology",
        _ => "visual_chemistry",
    };
    let partition = match index % 5 {
        0..=2 => "development",
        3 => "validation",
        _ => "sealed",
    };
    MemoryRecord {
        record_id: format!("source-route-{index:06}"),
        domain: format!("source_route/{family}"),
        artifact_type: if (index / 6) % 2 == 0 {
            "typed_receipt"
        } else {
            "replay_receipt"
        }
        .into(),
        version: format!("v{}", index % 3),
        payload: format!("stage155|family={family}|partition={partition}|index={index}"),
        provenance: vec![
            format!("stage155-report-sha256:{source_hash}"),
            "stage154-cloned-curriculum-admission".into(),
        ],
        content_hash: String::new(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_bytes = fs::read(SOURCE_REPORT)?;
    let source: Value = serde_json::from_slice(&source_bytes)?;
    assert_eq!(field(&source, "cases"), 2400);
    assert_eq!(field(&source, "exact_decisions"), 2400);
    assert_eq!(field(&source, "false_authorizations"), 0);
    assert_eq!(field(&source, "false_denials"), 0);
    let source_hash = format!("{:x}", Sha256::digest(&source_bytes));

    let mut memory = CurriculumMemory::new();
    let mut source_records = Vec::with_capacity(RECORDS);
    for index in 0..RECORDS {
        let item = record(index, &source_hash);
        assert_eq!(memory.append(item.clone()), AppendStatus::Appended);
        source_records.push(item);
    }
    assert_eq!(memory.len(), RECORDS);
    assert_eq!(memory.segment_count(), 391);

    let duplicate = memory.append(source_records[42].clone());
    assert_eq!(duplicate, AppendStatus::Duplicate);
    let invalid = memory.append(MemoryRecord {
        record_id: "source-route-invalid".into(),
        domain: "source_route/statistics".into(),
        artifact_type: "typed_receipt".into(),
        version: "v1".into(),
        payload: "tampered".into(),
        provenance: vec![format!("stage155-report-sha256:{source_hash}")],
        content_hash: "wrong-hash".into(),
    });
    assert_eq!(invalid, AppendStatus::Invalid);

    let replay_verified = memory
        .all_records()
        .filter(|item| memory.replay_verified(item))
        .count();
    assert_eq!(replay_verified, RECORDS);

    let route_counts: BTreeMap<_, _> = [
        "statistics",
        "biology",
        "chemistry",
        "visual_statistics",
        "visual_biology",
        "visual_chemistry",
    ]
    .into_iter()
    .map(|family| {
        (
            family.to_string(),
            memory
                .retrieve_domain(&format!("source_route/{family}"))
                .len(),
        )
    })
    .collect();
    assert_eq!(route_counts.values().sum::<usize>(), RECORDS);
    let exact_visual = memory.retrieve_exact("source_route/visual_statistics", "typed_receipt");
    let exact_contamination = exact_visual
        .iter()
        .filter(|item| {
            item.domain != "source_route/visual_statistics" || item.artifact_type != "typed_receipt"
        })
        .count();
    assert_eq!(exact_contamination, 0);
    assert_eq!(memory.retrieve_domain("source_route/unknown").len(), 0);

    let tamper_sample = (0..RECORDS).step_by(100).count();
    let tamper_rejected = (0..RECORDS)
        .step_by(100)
        .filter(|index| {
            let mut tampered = memory
                .get(&format!("source-route-{index:06}"))
                .unwrap()
                .clone();
            tampered.payload.push('x');
            !memory.replay_verified(&tampered)
        })
        .count();
    assert_eq!(tamper_rejected, tamper_sample);

    let mut reconstructed = CurriculumMemory::new();
    for item in &source_records {
        let mut stored = item.clone();
        stored.content_hash = record_hash(&stored);
        assert_eq!(reconstructed.append(stored), AppendStatus::Appended);
    }
    let original_hash = digest(&memory.all_records().cloned().collect::<Vec<_>>());
    let reconstructed_hash = digest(&reconstructed.all_records().cloned().collect::<Vec<_>>());
    assert_eq!(original_hash, reconstructed_hash);
    assert_eq!(reconstructed.len(), RECORDS);
    assert_eq!(reconstructed.segment_count(), memory.segment_count());

    let report = serde_json::json!({
        "schema": "stage156-source-route-memory-scale-v1",
        "source_report": SOURCE_REPORT,
        "source_report_sha256": source_hash,
        "records": RECORDS,
        "segments": memory.segment_count(),
        "segment_capacity": the_machine::curriculum_memory::SEGMENT_CAPACITY,
        "duplicate_rejections": usize::from(duplicate == AppendStatus::Duplicate),
        "invalid_rejections": usize::from(invalid == AppendStatus::Invalid),
        "route_counts": route_counts,
        "exact_visual_typed_receipts": exact_visual.len(),
        "exact_retrieval_contamination": exact_contamination,
        "unknown_route_records": memory.retrieve_domain("source_route/unknown").len(),
        "replay_verified": replay_verified,
        "tamper_sample": tamper_sample,
        "tamper_rejected": tamper_rejected,
        "reconstructed_records": reconstructed.len(),
        "reconstruction_hash_equal": original_hash == reconstructed_hash,
        "live_registry_mutations": 0,
        "live_curriculum_mutations": 0,
    });
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(
        "docs/stage156_source_route_memory_scale.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
