//! Stage 312: integrated checkpoint over independent curriculum evaluations.
//!
//! This is a release-level audit, not a replacement benchmark.  It verifies
//! the immutable reports from technical-language, source-composition,
//! multimodal, and sealed-learning evaluations, keeping conditional replay
//! denominators explicit and requiring the shared zero-authorization policy.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;

const REPORT_JSON: &str = "docs/stage312_integrated_curriculum_checkpoint.json";
const REPORT_MD: &str = "docs/stage312_integrated_curriculum_checkpoint.md";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExamAudit {
    report: String,
    report_sha256: String,
    cases: usize,
    exact_decisions: usize,
    authorized: usize,
    ambiguous: usize,
    refused: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    hle_questions_read: usize,
    live_mutations: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    independent_exam_reports: Vec<ExamAudit>,
    aggregate_cases: usize,
    aggregate_exact_decisions: usize,
    aggregate_authorized: usize,
    aggregate_ambiguous: usize,
    aggregate_refused: usize,
    aggregate_replay_verified: usize,
    aggregate_tamper_rejected: usize,
    aggregate_false_authorizations: usize,
    aggregate_false_denials: usize,
    sealed_learning_baseline: usize,
    sealed_learning_post: usize,
    sealed_learning_delta: usize,
    prerequisite_proposals: usize,
    prerequisite_closures: usize,
    prerequisite_cycle_rejections: usize,
    all_zero_authorization_checks: bool,
    all_replay_checks: bool,
    all_tamper_checks: bool,
    all_hle_exclusions: bool,
    all_live_mutation_checks: bool,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn number(value: &serde_json::Value, key: &str) -> usize {
    value[key]
        .as_u64()
        .unwrap_or_else(|| panic!("missing numeric field {key}")) as usize
}

fn audit(
    path: &str,
    mapping: (&str, &str, &str),
    replay: &str,
    tamper: &str,
) -> Result<ExamAudit, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let cases = number(&value, "cases");
    let exact_decisions = number(&value, mapping.0);
    let authorized = number(&value, mapping.1);
    let ambiguous = number(&value, mapping.2);
    let refused = if value.get("unsupported_refusals").is_some() {
        number(&value, "unsupported_refusals")
    } else if value.get("refused").is_some() {
        number(&value, "refused")
    } else {
        number(&value, "unsupported_cases")
    };
    let replay_verified = number(&value, replay);
    let tamper_rejected = number(&value, tamper);
    let false_authorizations = number(&value, "false_authorizations");
    let false_denials = number(&value, "false_denials");
    let hle_questions_read = number(&value, "hle_questions_read");
    let live_mutations = if value.get("production_mutations").is_some() {
        number(&value, "production_mutations")
    } else if value.get("production_registry_mutations").is_some() {
        number(&value, "production_registry_mutations")
    } else {
        number(&value, "production_mutations")
    };
    Ok(ExamAudit {
        report: path.into(),
        report_sha256: digest(&bytes),
        cases,
        exact_decisions,
        authorized,
        ambiguous,
        refused,
        replay_verified,
        tamper_rejected,
        false_authorizations,
        false_denials,
        hle_questions_read,
        live_mutations,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let exams = vec![
        audit(
            "docs/stage284_curriculum_technical_language_benchmark.json",
            ("exact_decisions", "authorized", "ambiguity_preserved"),
            "replay_verified",
            "tamper_rejected",
        )?,
        audit(
            "docs/stage_v_source_composition_1000.json",
            ("exact_decisions", "supported", "ambiguous"),
            "replay_verified",
            "tamper_rejections",
        )?,
        audit(
            "docs/stage_x_multimodal_curriculum_1000.json",
            (
                "exact_decisions",
                "authorized_supported",
                "ambiguities_preserved",
            ),
            "table_replay_verified",
            "frontend_tamper_rejected",
        )?,
        audit(
            "docs/stage311_sealed_curriculum_learning_curve.json",
            (
                "post_exact_decisions",
                "post_authorized",
                "post_ambiguous_preserved",
            ),
            "post_replays",
            "post_tamper_rejections",
        )?,
    ];
    // Stage 311's replay/tamper fields are split between post and sealed
    // partitions; replace its case-level values with the complete totals.
    let stage311_bytes = fs::read("docs/stage311_sealed_curriculum_learning_curve.json")?;
    let stage311: serde_json::Value = serde_json::from_slice(&stage311_bytes)?;
    let mut exams = exams;
    let last = exams.last_mut().unwrap();
    last.replay_verified = number(&stage311, "post_replays") + number(&stage311, "sealed_replays");
    last.tamper_rejected =
        number(&stage311, "post_tamper_rejections") + number(&stage311, "sealed_tamper_rejections");
    assert_eq!(last.replay_verified, last.cases);
    assert_eq!(last.tamper_rejected, last.cases);
    for audit in exams.iter().take(3) {
        assert_eq!(audit.exact_decisions, audit.cases);
        assert_eq!(audit.replay_verified, audit.cases);
        assert_eq!(audit.tamper_rejected, audit.cases);
    }

    let prerequisite_bytes = fs::read("docs/stage310_prerequisite_discovery_campaign.json")?;
    let prerequisite: serde_json::Value = serde_json::from_slice(&prerequisite_bytes)?;
    let aggregate_cases = exams.iter().map(|audit| audit.cases).sum();
    let aggregate_exact_decisions = exams.iter().map(|audit| audit.exact_decisions).sum();
    let aggregate_authorized = exams.iter().map(|audit| audit.authorized).sum();
    let aggregate_ambiguous = exams.iter().map(|audit| audit.ambiguous).sum();
    let aggregate_refused = exams.iter().map(|audit| audit.refused).sum();
    let aggregate_replay_verified = exams.iter().map(|audit| audit.replay_verified).sum();
    let aggregate_tamper_rejected = exams.iter().map(|audit| audit.tamper_rejected).sum();
    let aggregate_false_authorizations = exams.iter().map(|audit| audit.false_authorizations).sum();
    let aggregate_false_denials = exams.iter().map(|audit| audit.false_denials).sum();
    let all_zero_authorization_checks =
        aggregate_false_authorizations == 0 && aggregate_false_denials == 0;
    let all_replay_checks = aggregate_replay_verified == aggregate_cases;
    let all_tamper_checks = aggregate_tamper_rejected == aggregate_cases;
    let all_hle_exclusions = exams.iter().all(|audit| audit.hle_questions_read == 0);
    let all_live_mutation_checks = exams.iter().all(|audit| audit.live_mutations == 0);
    let report = Report {
        schema: "stage312-integrated-curriculum-checkpoint-v1",
        independent_exam_reports: exams,
        aggregate_cases,
        aggregate_exact_decisions,
        aggregate_authorized,
        aggregate_ambiguous,
        aggregate_refused,
        aggregate_replay_verified,
        aggregate_tamper_rejected,
        aggregate_false_authorizations,
        aggregate_false_denials,
        sealed_learning_baseline: number(&stage311, "baseline_sealed_authorized"),
        sealed_learning_post: number(&stage311, "sealed_authorized"),
        sealed_learning_delta: number(&stage311, "sealed_authorized")
            - number(&stage311, "baseline_sealed_authorized"),
        prerequisite_proposals: number(&prerequisite, "proposal_count"),
        prerequisite_closures: number(&prerequisite, "complete_discoveries"),
        prerequisite_cycle_rejections: number(&prerequisite, "self_cycle_rejections"),
        all_zero_authorization_checks,
        all_replay_checks,
        all_tamper_checks,
        all_hle_exclusions,
        all_live_mutation_checks,
    };
    assert_eq!(report.aggregate_cases, 4_500);
    assert_eq!(report.aggregate_exact_decisions, report.aggregate_cases);
    assert_eq!(report.aggregate_replay_verified, report.aggregate_cases);
    assert_eq!(report.aggregate_tamper_rejected, report.aggregate_cases);
    assert!(report.all_zero_authorization_checks);
    assert!(report.all_hle_exclusions && report.all_live_mutation_checks);
    assert_eq!(report.sealed_learning_delta, 60);
    assert_eq!(report.prerequisite_proposals, 6);
    assert_eq!(report.prerequisite_closures, 6);
    assert_eq!(report.prerequisite_cycle_rejections, 6);

    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 312 — integrated curriculum checkpoint\n\n* independent exam cases: {}\n* exact decisions: {} / {}\n* authorized / ambiguous / refused: {} / {} / {}\n* replay / tamper: {} / {}\n* false authorizations / denials: {} / {}\n* sealed learning baseline / post / delta: {} / {} / {}\n* prerequisite proposals / closures / cycle rejections: {} / {} / {}\n* all zero-authorization checks: {}\n* all replay / tamper checks: {} / {}\n* HLE exclusions / live-mutation checks: {} / {}\n\nThis checkpoint audits four independently generated evaluation artifacts without merging conditional denominators. It is evidence aggregation, not a new training corpus or HLE exposure.\n",
            report.aggregate_cases,
            report.aggregate_exact_decisions,
            report.aggregate_cases,
            report.aggregate_authorized,
            report.aggregate_ambiguous,
            report.aggregate_refused,
            report.aggregate_replay_verified,
            report.aggregate_tamper_rejected,
            report.aggregate_false_authorizations,
            report.aggregate_false_denials,
            report.sealed_learning_baseline,
            report.sealed_learning_post,
            report.sealed_learning_delta,
            report.prerequisite_proposals,
            report.prerequisite_closures,
            report.prerequisite_cycle_rejections,
            report.all_zero_authorization_checks,
            report.all_replay_checks,
            report.all_tamper_checks,
            report.all_hle_exclusions,
            report.all_live_mutation_checks,
        ),
    )?;
    println!(
        "stage312 cases={} exact={} authorized={} sealed_delta={} false_auth=0",
        report.aggregate_cases,
        report.aggregate_exact_decisions,
        report.aggregate_authorized,
        report.sealed_learning_delta
    );
    Ok(())
}
