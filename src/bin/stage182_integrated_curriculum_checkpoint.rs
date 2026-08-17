//! Stage 182: integrated sealed curriculum checkpoint.
//!
//! This checkpoint does not reinterpret or merge parent cases.  It verifies
//! the independent 5,000-case technical exam, the deeper 4/5-domain math
//! synthesis, the newly acquired source catalog, and its lifecycle receipts,
//! then reports their denominators separately and in aggregate.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

const EXAM: &str = "docs/stage_k_sealed_curriculum_exam_5000.json";
const SYNTHESIS: &str = "docs/stage179_five_domain_math_synthesis.json";
const SOURCE: &str = "docs/stage180_autonomous_source_catalog.json";
const LIFECYCLE: &str = "docs/stage181_source_catalog_promotion_rollback.json";
const REPORT_JSON: &str = "docs/stage182_integrated_curriculum_checkpoint.json";
const REPORT_MD: &str = "docs/stage182_integrated_curriculum_checkpoint.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    parent_file_sha256: BTreeMap<String, String>,
    independent_exam_cases: usize,
    independent_exam_exact: usize,
    independent_exam_authorized: usize,
    independent_exam_replay: usize,
    independent_exam_tamper: usize,
    synthesis_cases: usize,
    synthesis_exact: usize,
    synthesis_authorized: usize,
    synthesis_sealed_cases: usize,
    synthesis_sealed_authorized: usize,
    source_cases: usize,
    source_exact: usize,
    source_authorized: usize,
    source_sealed_cases: usize,
    source_sealed_authorized: usize,
    aggregate_exam_cases: usize,
    aggregate_exam_exact: usize,
    aggregate_exam_authorized: usize,
    aggregate_sealed_cases: usize,
    aggregate_sealed_authorized: usize,
    lifecycle_cases: usize,
    lifecycle_exact: usize,
    lifecycle_rollbacks: usize,
    lifecycle_historical_replays: usize,
    aggregate_false_authorizations: usize,
    aggregate_false_denials: usize,
    aggregate_replay_verified: usize,
    aggregate_tamper_rejected: usize,
    sealed_outcomes_exposed: usize,
    manifest_or_registry_mutations: usize,
    source_module_id: String,
}

fn file_value(path: &str) -> Result<(String, Value), Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    Ok((hash, serde_json::from_slice(&bytes)?))
}

fn u(value: &Value, key: &str) -> usize {
    value.get(key).and_then(Value::as_u64).unwrap_or(0) as usize
}

fn bool_value(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (exam_hash, exam) = file_value(EXAM)?;
    let (synthesis_hash, synthesis) = file_value(SYNTHESIS)?;
    let (source_hash, source) = file_value(SOURCE)?;
    let (lifecycle_hash, lifecycle) = file_value(LIFECYCLE)?;
    assert_eq!(u(&exam, "cases"), 5000);
    assert_eq!(u(&exam, "supported_authorized"), 3000);
    assert_eq!(u(&exam, "false_authorizations"), 0);
    assert_eq!(u(&exam, "false_denials"), 0);
    assert_eq!(u(&exam, "replay_verified"), 5000);
    assert_eq!(u(&exam, "tamper_rejections"), 5000);
    assert!(!bool_value(&exam, "manifest_mutated"));
    assert_eq!(u(&synthesis, "cases"), 1000);
    assert_eq!(u(&synthesis, "exact_decisions"), 1000);
    assert_eq!(u(&synthesis, "authorized_answers"), 600);
    assert_eq!(u(&synthesis, "false_authorizations"), 0);
    assert_eq!(u(&synthesis, "false_denials"), 0);
    assert_eq!(u(&synthesis, "replay_verified"), 1000);
    assert_eq!(u(&synthesis, "tamper_rejected"), 1000);
    assert_eq!(u(&synthesis, "sealed_outcomes_exposed_to_selector"), 0);
    assert_eq!(u(&source, "cases"), 1000);
    assert_eq!(u(&source, "promoted_exact"), 1000);
    assert_eq!(u(&source, "promoted_authorized"), 600);
    assert_eq!(u(&source, "false_authorizations"), 0);
    assert_eq!(u(&source, "false_denials"), 0);
    assert_eq!(u(&source, "promoted_replay_verified"), 1000);
    assert_eq!(u(&source, "promoted_tamper_rejected"), 1000);
    assert_eq!(u(&source, "sealed_outcomes_exposed_to_selector"), 0);
    assert_eq!(u(&source, "manifest_mutations"), 0);
    assert_eq!(u(&source, "registry_mutations"), 0);
    assert_eq!(u(&lifecycle, "cases"), 240);
    assert_eq!(u(&lifecycle, "exact_promotion_decisions"), 240);
    assert_eq!(u(&lifecycle, "false_authorizations"), 0);
    assert_eq!(u(&lifecycle, "false_denials"), 0);
    assert_eq!(u(&lifecycle, "live_registry_mutations"), 0);
    let source_module_id = source
        .get("inferred_module_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let exam_sealed = exam
        .get("partitions")
        .and_then(|v| v.get("sealed"))
        .ok_or("missing exam sealed partition")?;
    let synthesis_cases = u(&synthesis, "cases");
    let source_cases = u(&source, "cases");
    let aggregate_exam_cases = u(&exam, "cases") + synthesis_cases + source_cases;
    let aggregate_exam_exact =
        u(&exam, "cases") + u(&synthesis, "exact_decisions") + u(&source, "promoted_exact");
    let aggregate_exam_authorized = u(&exam, "supported_authorized")
        + u(&synthesis, "authorized_answers")
        + u(&source, "promoted_authorized");
    let aggregate_sealed_cases =
        u(exam_sealed, "cases") + u(&synthesis, "sealed_cases") + u(&source, "sealed_cases");
    let aggregate_sealed_authorized = u(exam_sealed, "supported_authorized")
        + u(&synthesis, "sealed_authorized_answers")
        + u(&source, "sealed_promoted_authorized");
    let report = Report {
        schema: "stage182-integrated-curriculum-checkpoint-v1",
        parent_file_sha256: BTreeMap::from([
            (EXAM.into(), exam_hash),
            (SYNTHESIS.into(), synthesis_hash),
            (SOURCE.into(), source_hash),
            (LIFECYCLE.into(), lifecycle_hash),
        ]),
        independent_exam_cases: u(&exam, "cases"),
        independent_exam_exact: u(&exam, "supported_authorized")
            + u(&exam, "ambiguities_preserved")
            + u(&exam, "unsupported_refused"),
        independent_exam_authorized: u(&exam, "supported_authorized"),
        independent_exam_replay: u(&exam, "replay_verified"),
        independent_exam_tamper: u(&exam, "tamper_rejections"),
        synthesis_cases,
        synthesis_exact: u(&synthesis, "exact_decisions"),
        synthesis_authorized: u(&synthesis, "authorized_answers"),
        synthesis_sealed_cases: u(&synthesis, "sealed_cases"),
        synthesis_sealed_authorized: u(&synthesis, "sealed_authorized_answers"),
        source_cases,
        source_exact: u(&source, "promoted_exact"),
        source_authorized: u(&source, "promoted_authorized"),
        source_sealed_cases: u(&source, "sealed_cases"),
        source_sealed_authorized: u(&source, "sealed_promoted_authorized"),
        aggregate_exam_cases,
        aggregate_exam_exact,
        aggregate_exam_authorized,
        aggregate_sealed_cases,
        aggregate_sealed_authorized,
        lifecycle_cases: u(&lifecycle, "cases"),
        lifecycle_exact: u(&lifecycle, "exact_promotion_decisions"),
        lifecycle_rollbacks: u(&lifecycle, "rollbacks_applied"),
        lifecycle_historical_replays: u(&lifecycle, "historical_replays"),
        aggregate_false_authorizations: u(&exam, "false_authorizations")
            + u(&synthesis, "false_authorizations")
            + u(&source, "false_authorizations")
            + u(&lifecycle, "false_authorizations"),
        aggregate_false_denials: u(&exam, "false_denials")
            + u(&synthesis, "false_denials")
            + u(&source, "false_denials")
            + u(&lifecycle, "false_denials"),
        aggregate_replay_verified: u(&exam, "replay_verified")
            + u(&synthesis, "replay_verified")
            + u(&source, "promoted_replay_verified")
            + u(&lifecycle, "promotion_replays"),
        aggregate_tamper_rejected: u(&exam, "tamper_rejections")
            + u(&synthesis, "tamper_rejected")
            + u(&source, "promoted_tamper_rejected")
            + u(&lifecycle, "promotion_tamper_rejections"),
        sealed_outcomes_exposed: u(&synthesis, "sealed_outcomes_exposed_to_selector")
            + u(&source, "sealed_outcomes_exposed_to_selector"),
        manifest_or_registry_mutations: u(&source, "manifest_mutations")
            + u(&source, "registry_mutations")
            + u(&lifecycle, "live_registry_mutations"),
        source_module_id,
    };
    assert_eq!(report.independent_exam_exact, 5000);
    assert_eq!(report.aggregate_exam_cases, 7000);
    assert_eq!(report.aggregate_exam_exact, 7000);
    assert_eq!(report.aggregate_exam_authorized, 4200);
    assert_eq!(report.aggregate_sealed_cases, 1400);
    assert_eq!(report.aggregate_sealed_authorized, 840);
    assert_eq!(report.lifecycle_exact, 240);
    assert_eq!(report.aggregate_false_authorizations, 0);
    assert_eq!(report.aggregate_false_denials, 0);
    assert_eq!(report.aggregate_replay_verified, 7240);
    assert_eq!(report.aggregate_tamper_rejected, 7240);
    assert_eq!(report.sealed_outcomes_exposed, 0);
    assert_eq!(report.manifest_or_registry_mutations, 0);
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, &json)?;
    fs::write(REPORT_MD, format!("# Stage 182 — integrated curriculum checkpoint\n\n| Independent corpus | Cases | Exact | Authorized | Sealed / authorized |\n|---|---:|---:|---:|---:|\n| Permanent technical exam | {} | {} | {} | 1000 / 600 |\n| Five-domain synthesis | {} | {} | {} | {} / {} |\n| Inferred source catalog | {} | {} | {} | {} / {} |\n| **Aggregate exam evidence** | **{}** | **{}** | **{}** | **{} / {}** |\n\n| Governance | Result |\n|---|---:|\n| Lifecycle cases / exact | {} / {} |\n| Rollbacks / historical replay | {} / {} |\n| Aggregate replay / tamper | {} / {} |\n| False authorizations / denials | 0 / 0 |\n| Sealed outcomes exposed | 0 |\n| Manifest or registry mutations | 0 |\n\nThis is a lineage-preserving checkpoint: parent corpora remain separate immutable evaluation artifacts. No sealed outcome was used to select, implement, or promote a capability.\n", report.independent_exam_cases, report.independent_exam_exact, report.independent_exam_authorized, report.synthesis_cases, report.synthesis_exact, report.synthesis_authorized, report.synthesis_sealed_cases, report.synthesis_sealed_authorized, report.source_cases, report.source_exact, report.source_authorized, report.source_sealed_cases, report.source_sealed_authorized, report.aggregate_exam_cases, report.aggregate_exam_exact, report.aggregate_exam_authorized, report.aggregate_sealed_cases, report.aggregate_sealed_authorized, report.lifecycle_cases, report.lifecycle_exact, report.lifecycle_rollbacks, report.lifecycle_historical_replays, report.aggregate_replay_verified, report.aggregate_tamper_rejected))?;
    Ok(())
}
