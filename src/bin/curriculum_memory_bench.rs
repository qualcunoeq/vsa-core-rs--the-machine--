//! 10,000-record curriculum-memory scale and tamper campaign.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn record(index: usize) -> MemoryRecord {
    let domain = match index % 4 {
        0 => "combinatorics",
        1 => "source_formula",
        2 => "cross_domain",
        _ => "technical_language",
    };
    MemoryRecord {
        record_id: format!("curriculum-{index:05}"),
        domain: domain.into(),
        artifact_type: if index % 2 == 0 { "scalar" } else { "receipt" }.into(),
        payload: format!("artifact-payload-{index}"),
        provenance: vec![format!("source-{}", index % 7), "curriculum-shadow".into()],
        content_hash: String::new(),
    }
}

fn main() {
    let mut memory = CurriculumMemory::new();
    for index in 0..10_000 {
        assert_eq!(memory.append(record(index)), AppendStatus::Appended);
    }
    assert_eq!(memory.len(), 10_000);
    assert_eq!(memory.segment_count(), 40);
    let duplicate = memory.append(record(42));
    assert_eq!(duplicate, AppendStatus::Duplicate);
    let invalid = memory.append(MemoryRecord {
        record_id: "bad".into(),
        domain: "combinatorics".into(),
        artifact_type: "scalar".into(),
        payload: "tampered".into(),
        provenance: vec!["source".into()],
        content_hash: "wrong-hash".into(),
    });
    assert_eq!(invalid, AppendStatus::Invalid);
    let replay_verified = memory
        .all_records()
        .filter(|record| memory.replay_verified(record))
        .count();
    assert_eq!(replay_verified, 10_000);
    let retrieval_counts = [
        memory.retrieve_domain("combinatorics").len(),
        memory.retrieve_domain("source_formula").len(),
        memory.retrieve_domain("cross_domain").len(),
        memory.retrieve_domain("technical_language").len(),
    ];
    assert_eq!(retrieval_counts, [2500, 2500, 2500, 2500]);
    let tamper_rejected = (0..10_000)
        .filter(|index| {
            let mut tampered = memory
                .get(&format!("curriculum-{index:05}"))
                .unwrap()
                .clone();
            tampered.payload.push('x');
            !memory.replay_verified(&tampered)
        })
        .count();
    assert_eq!(tamper_rejected, 10_000);
    let manifest_hash = digest(&(
        memory.len(),
        memory.segment_count(),
        retrieval_counts,
        replay_verified,
        tamper_rejected,
    ));
    let report = serde_json::json!({
        "schema": "stage-e-curriculum-memory-v1",
        "records": 10_000,
        "segments": memory.segment_count(),
        "segment_capacity": the_machine::curriculum_memory::SEGMENT_CAPACITY,
        "duplicate_rejections": 1,
        "invalid_rejections": 1,
        "retrieval_counts": retrieval_counts,
        "replay_verified": replay_verified,
        "tamper_rejected": tamper_rejected,
        "live_mutation": false,
        "manifest_hash": manifest_hash,
    });
    let serialized = serde_json::to_string_pretty(&report).unwrap();
    std::fs::write(
        "docs/stage_e_curriculum_memory.json",
        format!("{serialized}\n"),
    )
    .unwrap();
    println!("{serialized}");
}
