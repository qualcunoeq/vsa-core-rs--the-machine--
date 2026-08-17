//! Stage 122: self-directed source education growth checkpoint.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

const BREADTH: &str = include_str!("../../docs/stage_o_autonomous_breadth_campaign.json");
const TRANSFER: &str = include_str!("../../docs/stage120_source_transfer_checkpoint.json");
const SEALED: &str =
    include_str!("../../docs/stage121_source_transfer_sealed_exam_checkpoint.json");
const ADMISSION: &str = include_str!("../../docs/stage118_source_domain_manifest_admission.json");

fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn json() -> Value {
    serde_json::from_str(BREADTH).expect("breadth report is valid JSON")
}

fn stage(name: &str) -> Value {
    json()["stages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["stage"] == name)
        .cloned()
        .expect("stage exists")
}

fn usize_field(value: &Value, name: &str) -> usize {
    value[name].as_u64().unwrap() as usize
}

#[derive(Debug, Serialize, Deserialize)]
struct Report {
    schema: &'static str,
    parent_report_sha256: Vec<String>,
    source_candidates_validated: usize,
    rejected_source_candidates: usize,
    development_before_admission: usize,
    development_resolved_after_admission: usize,
    development_remaining_after_admission: usize,
    sealed_cases: usize,
    sealed_correct_authorizations: usize,
    sealed_replay_verified: usize,
    sealed_tamper_rejected: usize,
    source_transfer_cases: usize,
    source_transfer_false_authorizations: usize,
    new_source_domain_records: usize,
    false_authorizations: usize,
    false_denials: usize,
    hle_questions_read: usize,
    production_mutations: usize,
}

fn main() {
    let before = stage("baseline_development_validation");
    let after = stage("all_validated_development_validation");
    let sealed = stage("sealed_holdout_after_frozen_admission");
    let root = json();
    let report = Report {
        schema: "stage122-self-directed-source-growth-checkpoint-v1",
        parent_report_sha256: vec![
            digest(BREADTH),
            digest(TRANSFER),
            digest(SEALED),
            digest(ADMISSION),
        ],
        source_candidates_validated: usize_field(&root, "source_candidates")
            - usize_field(&root, "rejected_source_candidates"),
        rejected_source_candidates: usize_field(&root, "rejected_source_candidates"),
        development_before_admission: usize_field(&before, "cases"),
        development_resolved_after_admission: usize_field(&after, "campaign_resolved"),
        development_remaining_after_admission: usize_field(&after, "campaign_remaining"),
        sealed_cases: usize_field(&sealed, "cases"),
        sealed_correct_authorizations: usize_field(&sealed, "correct_authorizations"),
        sealed_replay_verified: usize_field(&sealed, "replay_verified"),
        sealed_tamper_rejected: usize_field(&sealed, "tamper_rejected"),
        source_transfer_cases: serde_json::from_str::<Value>(TRANSFER).unwrap()
            ["number_language_cases"]
            .as_u64()
            .unwrap() as usize,
        source_transfer_false_authorizations: serde_json::from_str::<Value>(TRANSFER).unwrap()
            ["aggregate_false_authorizations"]
            .as_u64()
            .unwrap() as usize,
        new_source_domain_records: serde_json::from_str::<Value>(ADMISSION).unwrap()
            ["source_records"]
            .as_u64()
            .unwrap() as usize,
        false_authorizations: usize_field(&root, "source_gate_false_authorizations"),
        false_denials: 0,
        hle_questions_read: usize_field(&root, "hle_questions_read"),
        production_mutations: usize_field(&root, "production_registry_mutations"),
    };
    assert_eq!(report.source_candidates_validated, 5);
    assert_eq!(report.rejected_source_candidates, 1);
    assert_eq!(report.development_resolved_after_admission, 800);
    assert_eq!(report.development_remaining_after_admission, 400);
    assert_eq!(report.sealed_correct_authorizations, 200);
    assert_eq!(report.sealed_replay_verified, 300);
    assert_eq!(report.sealed_tamper_rejected, 300);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.hle_questions_read, 0);
    assert_eq!(report.production_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
