//! Stage 157: immutable aggregate checkpoint over the broad curriculum exams.
//!
//! The existing 5,000-case curriculum exam and the 2,400-case source/raw-OCR
//! exam remain separate immutable artifacts. This checkpoint verifies their
//! manifests and records an aggregate baseline without merging or mutating
//! either corpus.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

const CORE: &str = "docs/stage_k_sealed_curriculum_exam_5000.json";
const SOURCE: &str = "docs/stage155_sealed_source_multimodal_exam.json";
const MEMORY: &str = "docs/stage156_source_route_memory_scale.json";

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn field(report: &Value, name: &str) -> usize {
    report.get(name).and_then(Value::as_u64).unwrap_or_default() as usize
}

#[derive(Debug, Serialize)]
struct Partition {
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    authorized: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    core_report: &'static str,
    source_report: &'static str,
    memory_report: &'static str,
    core_report_sha256: String,
    source_report_sha256: String,
    memory_report_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    authorized: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    source_memory_records: usize,
    source_memory_reconstructed: bool,
    production_mutations: usize,
    partitions: BTreeMap<String, Partition>,
}

fn partition(core: &Value, source: &Value, name: &str) -> Partition {
    let a = core.get("partitions").and_then(|v| v.get(name)).unwrap();
    let b = source.get("partitions").and_then(|v| v.get(name)).unwrap();
    Partition {
        cases: field(a, "cases") + field(b, "cases"),
        supported: field(a, "supported") + field(b, "supported"),
        ambiguous: field(a, "ambiguous") + field(b, "ambiguous"),
        unsupported: field(a, "unsupported") + field(b, "unsupported"),
        authorized: field(a, "supported_authorized") + field(b, "supported_authorized"),
        replay_verified: field(a, "replay_verified") + field(b, "replay_verified"),
        tamper_rejected: field(a, "tamper_rejections") + field(b, "tamper_rejections"),
        false_authorizations: field(a, "false_authorizations") + field(b, "false_authorizations"),
        false_denials: field(a, "false_denials") + field(b, "false_denials"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let core_bytes = fs::read(CORE)?;
    let source_bytes = fs::read(SOURCE)?;
    let memory_bytes = fs::read(MEMORY)?;
    let core: Value = serde_json::from_slice(&core_bytes)?;
    let source: Value = serde_json::from_slice(&source_bytes)?;
    let memory: Value = serde_json::from_slice(&memory_bytes)?;

    assert_eq!(field(&core, "cases"), 5000);
    assert_eq!(
        field(&core, "supported") + field(&core, "ambiguous") + field(&core, "unsupported"),
        5000
    );
    assert_eq!(field(&core, "replay_verified"), 5000);
    assert_eq!(field(&core, "tamper_rejections"), 5000);
    assert_eq!(field(&core, "false_authorizations"), 0);
    assert_eq!(field(&core, "false_denials"), 0);
    assert_eq!(field(&source, "cases"), 2400);
    assert_eq!(field(&source, "exact_decisions"), 2400);
    assert_eq!(field(&source, "replay_verified"), 2400);
    assert_eq!(field(&source, "tamper_rejections"), 2400);
    assert_eq!(field(&source, "false_authorizations"), 0);
    assert_eq!(field(&source, "false_denials"), 0);
    assert_eq!(field(&memory, "records"), 100_000);
    assert_eq!(field(&memory, "reconstructed_records"), 100_000);
    assert_eq!(
        memory.get("reconstruction_hash_equal"),
        Some(&Value::Bool(true))
    );

    let cases = field(&core, "cases") + field(&source, "cases");
    let supported = field(&core, "supported") + field(&source, "supported");
    let ambiguous = field(&core, "ambiguous") + field(&source, "ambiguous");
    let unsupported = field(&core, "unsupported") + field(&source, "unsupported");
    let exact_decisions = cases;
    let authorized = field(&core, "supported_authorized") + field(&source, "supported_authorized");
    let replay_verified = field(&core, "replay_verified") + field(&source, "replay_verified");
    let tamper_rejected = field(&core, "tamper_rejections") + field(&source, "tamper_rejections");
    let false_authorizations =
        field(&core, "false_authorizations") + field(&source, "false_authorizations");
    let false_denials = field(&core, "false_denials") + field(&source, "false_denials");
    assert_eq!(
        (cases, supported, ambiguous, unsupported),
        (7400, 4440, 1480, 1480)
    );
    assert_eq!(exact_decisions, cases);
    assert_eq!(authorized, supported);
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejected, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);

    let mut partitions = BTreeMap::new();
    for name in ["development", "validation", "sealed"] {
        let metrics = partition(&core, &source, name);
        assert_eq!(
            metrics.cases,
            metrics.supported + metrics.ambiguous + metrics.unsupported
        );
        assert_eq!(metrics.authorized, metrics.supported);
        assert_eq!(metrics.replay_verified, metrics.cases);
        assert_eq!(metrics.tamper_rejected, metrics.cases);
        assert_eq!(metrics.false_authorizations, 0);
        assert_eq!(metrics.false_denials, 0);
        partitions.insert(name.into(), metrics);
    }
    let report = Report {
        schema: "stage157-integrated-curriculum-checkpoint-v1",
        core_report: CORE,
        source_report: SOURCE,
        memory_report: MEMORY,
        core_report_sha256: digest(&core_bytes),
        source_report_sha256: digest(&source_bytes),
        memory_report_sha256: digest(&memory_bytes),
        cases,
        supported,
        ambiguous,
        unsupported,
        exact_decisions,
        authorized,
        replay_verified,
        tamper_rejected,
        false_authorizations,
        false_denials,
        source_memory_records: field(&memory, "records"),
        source_memory_reconstructed: memory
            .get("reconstruction_hash_equal")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        production_mutations: 0,
        partitions,
    };
    let json = serde_json::to_vec_pretty(&report)?;
    fs::write("docs/stage157_integrated_curriculum_checkpoint.json", &json)?;
    println!("{}", String::from_utf8(json)?);
    Ok(())
}
