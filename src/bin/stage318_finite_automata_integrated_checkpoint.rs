//! Stage 318: immutable integrated checkpoint for the automata curriculum.
//!
//! This audit consumes the independently committed Stage 314--317 reports.
//! It does not regenerate a corpus or alter routing; it verifies that source
//! validation, composition, lifecycle governance, and holdout evidence all
//! remain mutually consistent.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;

const REPORT_JSON: &str = "docs/stage318_finite_automata_integrated_checkpoint.json";
const REPORT_MD: &str = "docs/stage318_finite_automata_integrated_checkpoint.md";

#[derive(Debug, Serialize)]
struct Evidence {
    report: String,
    report_sha256: String,
    cases: usize,
    exact: usize,
    replay: usize,
    tamper: usize,
    false_authorizations: usize,
    false_denials: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    reports: Vec<Evidence>,
    aggregate_cases: usize,
    aggregate_exact_decisions: usize,
    aggregate_replay_verified: usize,
    aggregate_replay_not_applicable: usize,
    aggregate_tamper_rejections: usize,
    aggregate_false_authorizations: usize,
    aggregate_false_denials: usize,
    aggregate_supported_or_promoted: usize,
    live_registry_mutations: usize,
    curriculum_manifest_mutations: usize,
    hle_questions_read: usize,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn number(value: &Value, key: &str) -> usize {
    value[key]
        .as_u64()
        .unwrap_or_else(|| panic!("missing {key}")) as usize
}

fn evidence(
    path: &str,
    exact_key: &str,
    replay_key: &str,
    tamper_key: &str,
) -> Result<Evidence, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    let value: Value = serde_json::from_slice(&bytes)?;
    Ok(Evidence {
        report: path.into(),
        report_sha256: digest(&bytes),
        cases: number(&value, "cases"),
        exact: number(&value, exact_key),
        replay: number(&value, replay_key),
        tamper: number(&value, tamper_key),
        false_authorizations: number(&value, "false_authorizations"),
        false_denials: number(&value, "false_denials"),
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let reports = vec![
        evidence(
            "docs/stage314_finite_automata_source_pack.json",
            "exact_decisions",
            "replay_verified",
            "tamper_rejections",
        )?,
        evidence(
            "docs/stage315_finite_automata_composition.json",
            "exact_decisions",
            "replay_verified",
            "tamper_rejections",
        )?,
        evidence(
            "docs/stage316_finite_automata_promotion_rollback.json",
            "exact_lifecycle_decisions",
            "registry_replays",
            "tamper_rejections",
        )?,
        evidence(
            "docs/stage317_finite_automata_independent_holdout.json",
            "exact_decisions",
            "replay_verified",
            "tamper_rejections",
        )?,
    ];
    let aggregate_cases = reports.iter().map(|item| item.cases).sum::<usize>();
    let aggregate_exact_decisions = reports.iter().map(|item| item.exact).sum::<usize>();
    let aggregate_replay_verified = reports.iter().map(|item| item.replay).sum::<usize>();
    let aggregate_tamper_rejections = reports.iter().map(|item| item.tamper).sum::<usize>();
    let aggregate_false_authorizations = reports
        .iter()
        .map(|item| item.false_authorizations)
        .sum::<usize>();
    let aggregate_false_denials = reports.iter().map(|item| item.false_denials).sum::<usize>();
    let stage314: Value =
        serde_json::from_slice(&fs::read("docs/stage314_finite_automata_source_pack.json")?)?;
    let stage315: Value =
        serde_json::from_slice(&fs::read("docs/stage315_finite_automata_composition.json")?)?;
    let stage316: Value = serde_json::from_slice(&fs::read(
        "docs/stage316_finite_automata_promotion_rollback.json",
    )?)?;
    let stage317: Value = serde_json::from_slice(&fs::read(
        "docs/stage317_finite_automata_independent_holdout.json",
    )?)?;
    let report = Report {
        schema: "stage318-finite-automata-integrated-checkpoint-v1",
        reports,
        aggregate_cases,
        aggregate_exact_decisions,
        aggregate_replay_verified,
        aggregate_replay_not_applicable: number(&stage317, "cases")
            - number(&stage317, "replay_verified"),
        aggregate_tamper_rejections,
        aggregate_false_authorizations,
        aggregate_false_denials,
        aggregate_supported_or_promoted: number(&stage314, "supported")
            + number(&stage315, "supported")
            + number(&stage316, "promotions")
            + number(&stage317, "supported"),
        live_registry_mutations: number(&stage314, "live_registry_mutations")
            + number(&stage315, "live_registry_mutations")
            + number(&stage316, "production_registry_mutations")
            + number(&stage317, "live_registry_mutations"),
        curriculum_manifest_mutations: number(&stage316, "curriculum_manifest_mutations"),
        hle_questions_read: number(&stage314, "hle_questions_read")
            + number(&stage315, "hle_questions_read")
            + number(&stage317, "hle_questions_read"),
    };
    assert_eq!(report.aggregate_cases, 820);
    assert_eq!(report.aggregate_exact_decisions, 820);
    assert_eq!(report.aggregate_replay_verified, 780);
    assert_eq!(report.aggregate_replay_not_applicable, 40);
    assert_eq!(report.aggregate_tamper_rejections, 780);
    assert_eq!(report.aggregate_false_authorizations, 0);
    assert_eq!(report.aggregate_false_denials, 0);
    assert_eq!(report.aggregate_supported_or_promoted, 400);
    assert_eq!(report.live_registry_mutations, 0);
    assert_eq!(report.curriculum_manifest_mutations, 0);
    assert_eq!(report.hle_questions_read, 0);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 318 — integrated finite-automata curriculum checkpoint\n\n- Reports: {}\n- Aggregate cases / exact decisions: {} / {}\n- Supported or promoted artifacts: {}\n- Replay verified / not applicable: {} / {}\n- Tamper rejections: {}\n- False authorizations / denials: {} / {}\n- Live registry / curriculum mutations: {} / {}\n- HLE questions read: {}\n\nThe checkpoint covers source-derived validation, graph/trace composition, cloned-registry lifecycle governance, and an independently authored holdout. All component reports remain immutable inputs.\n",
            report.reports.len(), report.aggregate_cases, report.aggregate_exact_decisions,
            report.aggregate_supported_or_promoted, report.aggregate_replay_verified,
            report.aggregate_replay_not_applicable, report.aggregate_tamper_rejections,
            report.aggregate_false_authorizations, report.aggregate_false_denials,
            report.live_registry_mutations, report.curriculum_manifest_mutations,
            report.hle_questions_read,
        ),
    )?;
    println!(
        "stage318 cases={} exact={} supported_or_promoted={} replay={} tamper={} false_auth={}",
        report.aggregate_cases,
        report.aggregate_exact_decisions,
        report.aggregate_supported_or_promoted,
        report.aggregate_replay_verified,
        report.aggregate_tamper_rejections,
        report.aggregate_false_authorizations
    );
    Ok(())
}
