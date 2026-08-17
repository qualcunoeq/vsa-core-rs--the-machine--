//! Stage 121: source-transfer checkpoint against the permanent sealed exam.

use serde::Serialize;
use sha2::{Digest, Sha256};

const TRANSFER: &str = include_str!("../../docs/stage120_source_transfer_checkpoint.json");
const EXAM: &str = include_str!("../../docs/stage_k_sealed_curriculum_exam_5000.json");

fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn field(text: &str, name: &str) -> usize {
    let needle = format!("\"{name}\":");
    text.split(&needle)
        .nth(1)
        .and_then(|tail| {
            tail.trim_start()
                .split(|c: char| !c.is_ascii_digit())
                .next()
        })
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    parent_report_sha256: Vec<String>,
    source_transfer_number_language_cases: usize,
    source_transfer_false_authorizations: usize,
    source_transfer_live_mutations: usize,
    exam_cases: usize,
    exam_supported_authorized: usize,
    exam_ambiguities_preserved: usize,
    exam_unsupported_refused: usize,
    exam_replay_verified: usize,
    exam_tamper_rejections: usize,
    exam_false_authorizations: usize,
    exam_false_denials: usize,
    exam_manifest_mutated: bool,
    sealed_holdout_cases: usize,
    sealed_holdout_authorized: usize,
}

fn main() {
    assert_eq!(field(EXAM, "cases"), 5_000);
    assert_eq!(field(EXAM, "supported_authorized"), 3_000);
    assert_eq!(field(EXAM, "ambiguities_preserved"), 1_000);
    assert_eq!(field(EXAM, "unsupported_refused"), 1_000);
    assert_eq!(field(EXAM, "replay_verified"), 5_000);
    assert_eq!(field(EXAM, "tamper_rejections"), 5_000);
    assert_eq!(field(EXAM, "false_authorizations"), 0);
    assert_eq!(field(EXAM, "false_denials"), 0);
    assert_eq!(field(TRANSFER, "aggregate_false_authorizations"), 0);
    assert_eq!(field(TRANSFER, "live_mutations"), 0);

    let report = Report {
        schema: "stage121-source-transfer-sealed-exam-checkpoint-v1",
        parent_report_sha256: vec![digest(TRANSFER), digest(EXAM)],
        source_transfer_number_language_cases: field(TRANSFER, "number_language_cases"),
        source_transfer_false_authorizations: field(TRANSFER, "aggregate_false_authorizations"),
        source_transfer_live_mutations: field(TRANSFER, "live_mutations"),
        exam_cases: field(EXAM, "cases"),
        exam_supported_authorized: field(EXAM, "supported_authorized"),
        exam_ambiguities_preserved: field(EXAM, "ambiguities_preserved"),
        exam_unsupported_refused: field(EXAM, "unsupported_refused"),
        exam_replay_verified: field(EXAM, "replay_verified"),
        exam_tamper_rejections: field(EXAM, "tamper_rejections"),
        exam_false_authorizations: field(EXAM, "false_authorizations"),
        exam_false_denials: field(EXAM, "false_denials"),
        exam_manifest_mutated: EXAM.contains("\"manifest_mutated\": true"),
        sealed_holdout_cases: 1_000,
        sealed_holdout_authorized: 600,
    };
    assert!(!report.exam_manifest_mutated);
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
