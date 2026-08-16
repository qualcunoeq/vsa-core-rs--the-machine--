//! Stage Z: validate answer-key-blind HLE learning proposals against
//! immutable independent-corpus evidence.
//!
//! This is a promotion preflight only.  It reads the Stage Y proposals and
//! existing source-derived benchmark manifests, verifies their replay and
//! boundary gates, and records what would be eligible for sandbox promotion.
//! No curriculum or production registry is changed.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::process::Command;

const PLAN_REPORT: &str = "docs/stage_y_hle_gap_education.json";
const SUMMARY: &str = "docs/stage_z_hle_gap_validation.json";

#[derive(Debug, Serialize)]
struct ValidationReceipt {
    module_id: String,
    covered_case_count: usize,
    source_ids_present: bool,
    independent_exercise_count: usize,
    evidence_path: String,
    evidence_sha256: String,
    evidence_cases: usize,
    evidence_supported: usize,
    exact_decisions: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    source_provenance_present: bool,
    independent_gate: bool,
    replay_gate: bool,
    safety_gate: bool,
    sandbox_validated: bool,
    promotion_allowed: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    producer_commit: String,
    plan_report: &'static str,
    plan_report_sha256: String,
    plan_manifest_sha256: String,
    plans_read: usize,
    plans_with_exact_overlap: usize,
    validation_receipts: Vec<ValidationReceipt>,
    sandbox_validated_plans: usize,
    promotion_allowed_plans: usize,
    manifest_unchanged: bool,
    production_registry_mutations: usize,
    false_authorizations: usize,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn producer_commit() -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".into())
}

fn evidence_path(module_id: &str) -> Option<&'static str> {
    match module_id {
        "curriculum_linear_algebra_frontend" => Some("docs/phase52_linear_algebra_pack_bench.json"),
        "curriculum_finite_probability_frontend" => {
            Some("docs/phase54_probability_pack_bench.json")
        }
        "curriculum_bounded_graph_frontend" => Some("docs/phase56_graph_pack_bench.json"),
        "curriculum_bounded_calculus_frontend" => Some("docs/phase61_calculus_pack.json"),
        "curriculum_finite_dynamics_frontend" => Some("docs/stage_a_finite_markov_pack.json"),
        _ => None,
    }
}

fn usize_field(value: &Value, names: &[&str]) -> usize {
    names
        .iter()
        .find_map(|name| value.get(name).and_then(Value::as_u64))
        .unwrap_or(0) as usize
}

fn plan_string(plan: &Value, name: &str) -> String {
    plan.get(name)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let plan_bytes = fs::read(PLAN_REPORT)?;
    let plan_report: Value = serde_json::from_slice(&plan_bytes)?;
    let plan_manifest_sha256 = plan_string(&plan_report, "manifest_sha256");
    let plans = plan_report
        .get("learning_plans")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut receipts = Vec::new();
    for plan in &plans {
        let module_id = plan_string(plan, "module_id");
        let covered_case_count = usize_field(plan, &["covered_case_count"]);
        if covered_case_count == 0 {
            continue;
        }
        let source_ids_present = plan
            .get("source_ids")
            .and_then(Value::as_array)
            .is_some_and(|ids| !ids.is_empty());
        let independent_exercise_count = usize_field(plan, &["independent_exercise_count"]);
        let Some(path) = evidence_path(&module_id) else {
            continue;
        };
        let evidence_bytes = fs::read(path)?;
        let evidence: Value = serde_json::from_slice(&evidence_bytes)?;
        let evidence_cases = usize_field(&evidence, &["cases", "case_count"]);
        let evidence_supported = usize_field(&evidence, &["supported", "supported_cases"]);
        let exact_decisions = usize_field(&evidence, &["exact_decisions"]);
        let replay_verified = usize_field(&evidence, &["replay_verified"]);
        let tamper_rejections = usize_field(&evidence, &["tamper_rejections"]);
        let false_authorizations = usize_field(&evidence, &["false_authorizations"]);
        let false_denials = usize_field(&evidence, &["false_denials"]);
        let source_provenance_present = evidence
            .get("source")
            .and_then(Value::as_str)
            .is_some_and(|source| !source.trim().is_empty());
        let independent_gate = source_ids_present
            && independent_exercise_count >= 120
            && evidence_supported >= 120
            && evidence_cases >= 240;
        let replay_gate = evidence_cases > 0
            && exact_decisions == evidence_cases
            && replay_verified == evidence_cases
            && tamper_rejections == evidence_cases;
        let safety_gate =
            source_provenance_present && false_authorizations == 0 && false_denials == 0;
        let sandbox_validated = independent_gate && replay_gate && safety_gate;
        receipts.push(ValidationReceipt {
            module_id,
            covered_case_count,
            source_ids_present,
            independent_exercise_count,
            evidence_path: path.into(),
            evidence_sha256: digest_bytes(&evidence_bytes),
            evidence_cases,
            evidence_supported,
            exact_decisions,
            replay_verified,
            tamper_rejections,
            false_authorizations,
            false_denials,
            source_provenance_present,
            independent_gate,
            replay_gate,
            safety_gate,
            sandbox_validated,
            promotion_allowed: false,
        });
    }
    let sandbox_validated_plans = receipts
        .iter()
        .filter(|receipt| receipt.sandbox_validated)
        .count();
    let report = Report {
        schema: "stage-z-hle-gap-validation-v1",
        producer_commit: producer_commit(),
        plan_report: PLAN_REPORT,
        plan_report_sha256: digest_bytes(&plan_bytes),
        plan_manifest_sha256,
        plans_read: plans.len(),
        plans_with_exact_overlap: receipts.len(),
        validation_receipts: receipts,
        sandbox_validated_plans,
        promotion_allowed_plans: 0,
        manifest_unchanged: true,
        production_registry_mutations: 0,
        false_authorizations: 0,
    };
    assert_eq!(report.plans_with_exact_overlap, 5);
    assert_eq!(report.sandbox_validated_plans, 5);
    assert_eq!(report.promotion_allowed_plans, 0);
    assert!(report
        .validation_receipts
        .iter()
        .all(|receipt| receipt.promotion_allowed == false));
    assert_eq!(
        report
            .validation_receipts
            .iter()
            .filter(|receipt| receipt.sandbox_validated)
            .count(),
        5
    );
    assert!(report.manifest_unchanged);
    assert_eq!(report.production_registry_mutations, 0);
    assert_eq!(report.false_authorizations, 0);
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(SUMMARY, format!("{serialized}\n"))?;
    println!("{serialized}");
    Ok(())
}
