//! Stage 112: curriculum-memory stress with the expanded source curriculum.
use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};
const RECORDS: usize = 100_000;
fn digest<T: Serialize + ?Sized>(v: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(v).unwrap()))
}
fn domain(i: usize) -> &'static str {
    match i % 8 {
        0 => "finite_set",
        1 => "bounded_counting",
        2 => "truth_tables",
        3 => "bayes_rule",
        4 => "linear_algebra",
        5 => "probability",
        6 => "graph_theory",
        _ => "classical_mechanics",
    }
}
fn record(i: usize) -> MemoryRecord {
    MemoryRecord {
        record_id: format!("source-curriculum-{i:06}"),
        domain: domain(i).into(),
        artifact_type: if i % 3 == 0 {
            "theorem"
        } else if i % 3 == 1 {
            "receipt"
        } else {
            "problem"
        }
        .into(),
        version: format!("v{}", i % 4),
        payload: format!("typed-source-payload-{i}"),
        provenance: vec![
            format!("openstax-source-{}", i % 8),
            "stage112-shadow".into(),
        ],
        content_hash: String::new(),
    }
}
#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    records: usize,
    segments: usize,
    domain_counts: [usize; 8],
    replay_verified: usize,
    exact_retrieval: usize,
    exact_contamination: usize,
    version_contamination: usize,
    duplicate_rejections: usize,
    invalid_rejections: usize,
    tamper_rejected: usize,
    live_mutation: bool,
    manifest_hash: String,
}
fn main() {
    let mut memory = CurriculumMemory::new();
    for i in 0..RECORDS {
        assert_eq!(memory.append(record(i)), AppendStatus::Appended);
    }
    assert_eq!(memory.len(), RECORDS);
    let mut counts = [0; 8];
    for i in 0..RECORDS {
        counts[i % 8] += 1;
    }
    let replay = memory
        .all_records()
        .filter(|r| memory.replay_verified(r))
        .count();
    assert_eq!(replay, RECORDS);
    let exact = memory.retrieve_exact("truth_tables", "theorem");
    let contamination = exact
        .iter()
        .filter(|r| r.domain != "truth_tables" || r.artifact_type != "theorem")
        .count();
    let exact_retrieval = exact.len();
    assert_eq!(contamination, 0);
    drop(exact);
    let version = memory.retrieve_exact_version("finite_set", "receipt", "v2");
    let version_bad = version
        .iter()
        .filter(|r| r.domain != "finite_set" || r.artifact_type != "receipt" || r.version != "v2")
        .count();
    drop(version);
    assert_eq!(version_bad, 0);
    let duplicate = memory.append(record(42));
    assert_eq!(duplicate, AppendStatus::Duplicate);
    let invalid = memory.append(MemoryRecord {
        record_id: "bad-source".into(),
        domain: "truth_tables".into(),
        artifact_type: "theorem".into(),
        version: "v1".into(),
        payload: "tampered".into(),
        provenance: vec!["source".into()],
        content_hash: "bad".into(),
    });
    assert_eq!(invalid, AppendStatus::Invalid);
    let tamper = (0..RECORDS)
        .filter(|i| {
            let mut r = memory
                .get(&format!("source-curriculum-{i:06}"))
                .unwrap()
                .clone();
            r.payload.push('x');
            !memory.replay_verified(&r)
        })
        .count();
    assert_eq!(tamper, RECORDS);
    let report = Report {
        schema: "stage112-curriculum-memory-source-extension-v1",
        records: RECORDS,
        segments: memory.segment_count(),
        domain_counts: counts,
        replay_verified: replay,
        exact_retrieval,
        exact_contamination: contamination,
        version_contamination: version_bad,
        duplicate_rejections: 1,
        invalid_rejections: 1,
        tamper_rejected: tamper,
        live_mutation: false,
        manifest_hash: digest(include_str!("../../docs/curriculum_manifest.json")),
    };
    assert_eq!(report.exact_contamination, 0);
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
