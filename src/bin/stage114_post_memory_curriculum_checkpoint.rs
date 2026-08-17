//! Stage 114: post-memory sealed curriculum checkpoint.
//!
//! This report composes prior sealed corpus results without merging unlike
//! units. Curriculum cases, memory records, retrieval queries, and the frozen
//! HLE baseline remain separate denominators.

use serde::Serialize;
use sha2::{Digest, Sha256};

const STAGE103: &str = include_str!("../../docs/stage103_curriculum_checkpoint.json");
const STAGE106_HLE: &str = include_str!("../../docs/stage106_hle_curriculum_checkpoint.json");
const STAGE107: &str = include_str!("../../docs/stage107_source_logic_bench.json");
const STAGE108: &str = include_str!("../../docs/stage108_cross_domain_synthesis.json");
const STAGE110: &str = include_str!("../../docs/stage110_technical_language_2000.json");
const STAGE111: &str = include_str!("../../docs/stage111_source_catalog_ingestion.json");
const STAGE112: &str = include_str!("../../docs/stage112_curriculum_memory_source_extension.json");
const STAGE113: &str = include_str!("../../docs/stage113_curriculum_retrieval_prerequisites.json");

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

fn boolean_field(text: &str, name: &str) -> bool {
    let needle = format!("\"{name}\":");
    text.split(&needle)
        .nth(1)
        .is_some_and(|tail| tail.trim_start().starts_with("true"))
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    parent_report_sha256: Vec<String>,
    curriculum_cases: usize,
    curriculum_authorized: usize,
    curriculum_safe_refusals_or_ambiguities: usize,
    curriculum_replay_verified: usize,
    curriculum_tamper_rejections: usize,
    curriculum_false_authorizations: usize,
    curriculum_false_denials: usize,
    source_catalogs: usize,
    memory_records: usize,
    memory_replay_verified: usize,
    memory_tamper_rejected: usize,
    retrieval_queries: usize,
    retrieval_receipt_replays: usize,
    retrieval_receipt_tamper_rejections: usize,
    retrieval_contamination: usize,
    manifest_unchanged: bool,
    frozen_hle_cases: usize,
    frozen_hle_correct_authorized: usize,
    frozen_hle_false_authorizations: usize,
}

fn main() {
    let parents = [
        STAGE103,
        STAGE106_HLE,
        STAGE107,
        STAGE108,
        STAGE110,
        STAGE111,
        STAGE112,
        STAGE113,
    ];
    let curriculum_cases = field(STAGE103, "cases")
        + field(STAGE107, "cases")
        + field(STAGE108, "cases")
        + field(STAGE110, "cases");
    let curriculum_authorized = field(STAGE103, "authorized")
        + field(STAGE107, "authorized")
        + field(STAGE108, "authorized")
        + field(STAGE110, "authorized");
    let curriculum_replay_verified = field(STAGE103, "replay_verified")
        + field(STAGE107, "replay_verified")
        + field(STAGE108, "replay_verified")
        + field(STAGE110, "replay_verified");
    let curriculum_tamper_rejections = field(STAGE103, "tamper_rejections")
        + field(STAGE107, "tamper_rejections")
        + field(STAGE108, "tamper_rejections")
        + field(STAGE110, "tamper_rejections");
    let curriculum_false_authorizations = field(STAGE103, "false_authorizations")
        + field(STAGE107, "false_authorizations")
        + field(STAGE108, "false_authorizations")
        + field(STAGE110, "false_authorizations");
    let curriculum_false_denials = field(STAGE103, "false_denials")
        + field(STAGE107, "false_denials")
        + field(STAGE108, "false_denials")
        + field(STAGE110, "false_denials");
    assert_eq!(curriculum_cases, 10_400);
    assert_eq!(curriculum_authorized, 6_204);
    assert_eq!(curriculum_replay_verified, curriculum_cases);
    assert_eq!(curriculum_tamper_rejections, curriculum_cases);
    assert_eq!(curriculum_false_authorizations, 0);
    assert_eq!(curriculum_false_denials, 0);
    assert_eq!(field(STAGE112, "records"), 100_000);
    assert_eq!(field(STAGE112, "replay_verified"), 100_000);
    assert_eq!(field(STAGE112, "tamper_rejected"), 100_000);
    assert_eq!(field(STAGE113, "queries"), 2_000);
    assert_eq!(field(STAGE113, "receipt_replays"), 2_000);
    assert_eq!(field(STAGE113, "receipt_tamper_rejections"), 2_000);
    assert_eq!(field(STAGE113, "retrieval_contamination"), 0);
    assert!(boolean_field(STAGE113, "manifest_unchanged"));

    let report = Report {
        schema: "stage114-post-memory-curriculum-checkpoint-v1",
        parent_report_sha256: parents.iter().map(|parent| digest(parent)).collect(),
        curriculum_cases,
        curriculum_authorized,
        curriculum_safe_refusals_or_ambiguities: curriculum_cases - curriculum_authorized,
        curriculum_replay_verified,
        curriculum_tamper_rejections,
        curriculum_false_authorizations,
        curriculum_false_denials,
        source_catalogs: field(STAGE111, "catalogs"),
        memory_records: field(STAGE112, "records"),
        memory_replay_verified: field(STAGE112, "replay_verified"),
        memory_tamper_rejected: field(STAGE112, "tamper_rejected"),
        retrieval_queries: field(STAGE113, "queries"),
        retrieval_receipt_replays: field(STAGE113, "receipt_replays"),
        retrieval_receipt_tamper_rejections: field(STAGE113, "receipt_tamper_rejections"),
        retrieval_contamination: field(STAGE113, "retrieval_contamination"),
        manifest_unchanged: boolean_field(STAGE113, "manifest_unchanged"),
        frozen_hle_cases: field(STAGE106_HLE, "cases"),
        frozen_hle_correct_authorized: field(STAGE106_HLE, "correct_authorized"),
        frozen_hle_false_authorizations: field(STAGE106_HLE, "incorrect_authorized"),
    };
    assert_eq!(report.source_catalogs, 3);
    assert_eq!(report.frozen_hle_cases, 2_500);
    assert_eq!(report.frozen_hle_correct_authorized, 2);
    assert_eq!(report.frozen_hle_false_authorizations, 0);
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
