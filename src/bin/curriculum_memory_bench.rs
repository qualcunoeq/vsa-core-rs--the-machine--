//! 10,000-record curriculum-memory scale and tamper campaign.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};

const RECORDS: usize = 100_000;

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
        version: format!("v{}", index % 3),
        payload: format!("artifact-payload-{index}"),
        provenance: vec![format!("source-{}", index % 7), "curriculum-shadow".into()],
        content_hash: String::new(),
    }
}

fn main() {
    let mut memory = CurriculumMemory::new();
    for index in 0..RECORDS {
        assert_eq!(memory.append(record(index)), AppendStatus::Appended);
    }
    assert_eq!(memory.len(), RECORDS);
    assert_eq!(memory.segment_count(), 391);
    let duplicate = memory.append(record(42));
    assert_eq!(duplicate, AppendStatus::Duplicate);
    let invalid = memory.append(MemoryRecord {
        record_id: "bad".into(),
        domain: "combinatorics".into(),
        artifact_type: "scalar".into(),
        version: "v1".into(),
        payload: "tampered".into(),
        provenance: vec!["source".into()],
        content_hash: "wrong-hash".into(),
    });
    assert_eq!(invalid, AppendStatus::Invalid);
    let replay_verified = memory
        .all_records()
        .filter(|record| memory.replay_verified(record))
        .count();
    assert_eq!(replay_verified, RECORDS);
    let retrieval_counts = [
        memory.retrieve_domain("combinatorics").len(),
        memory.retrieve_domain("source_formula").len(),
        memory.retrieve_domain("cross_domain").len(),
        memory.retrieve_domain("technical_language").len(),
    ];
    assert_eq!(retrieval_counts, [25000, 25000, 25000, 25000]);
    let exact_retrieval = memory.retrieve_exact("combinatorics", "scalar");
    assert_eq!(exact_retrieval.len(), 25000);
    let exact_contamination = exact_retrieval
        .iter()
        .filter(|record| record.domain != "combinatorics" || record.artifact_type != "scalar")
        .count();
    assert_eq!(exact_contamination, 0);
    let empty_exact_query = memory.retrieve_exact("combinatorics", "theorem").len();
    assert_eq!(empty_exact_query, 0);
    let exact_v1 = memory.retrieve_exact_version("combinatorics", "scalar", "v1");
    let version_contamination = exact_v1
        .iter()
        .filter(|record| record.version != "v1")
        .count();
    assert_eq!(version_contamination, 0);
    let explicit_stale_v0 = memory
        .retrieve_exact_version("combinatorics", "scalar", "v0")
        .len();
    assert!(explicit_stale_v0 > 0);
    let tamper_rejected = (0..RECORDS)
        .filter(|index| {
            let mut tampered = memory
                .get(&format!("curriculum-{index:05}"))
                .unwrap()
                .clone();
            tampered.payload.push('x');
            !memory.replay_verified(&tampered)
        })
        .count();
    assert_eq!(tamper_rejected, RECORDS);
    let manifest_hash = digest(&(
        memory.len(),
        memory.segment_count(),
        retrieval_counts,
        exact_retrieval.len(),
        exact_contamination,
        empty_exact_query,
        exact_v1.len(),
        version_contamination,
        explicit_stale_v0,
        replay_verified,
        tamper_rejected,
    ));
    let report = serde_json::json!({
        "schema": "stage-e-curriculum-memory-v1",
        "records": RECORDS,
        "segments": memory.segment_count(),
        "segment_capacity": the_machine::curriculum_memory::SEGMENT_CAPACITY,
        "duplicate_rejections": 1,
        "invalid_rejections": 1,
        "retrieval_counts": retrieval_counts,
        "exact_retrieval_count": exact_retrieval.len(),
        "exact_retrieval_contamination": exact_contamination,
        "empty_exact_query": empty_exact_query,
        "exact_v1_retrieval_count": exact_v1.len(),
        "version_contamination": version_contamination,
        "explicit_stale_v0_count": explicit_stale_v0,
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
