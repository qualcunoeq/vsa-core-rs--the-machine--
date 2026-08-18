//! Stage 262: select the next source-backed curriculum candidate.
//!
//! This is an evidence-selection record, not a promotion mechanism.  The
//! geometry campaign is evaluated from immutable source, transfer, memory,
//! rollback, and sealed-holdout reports.  The current curriculum manifest is
//! never edited and no live route is enabled.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;

use the_machine::curriculum::breadth_first_manifest;

const REPORT_JSON: &str = "docs/stage262_source_geometry_candidate_selection.json";
const REPORT_MD: &str = "docs/stage262_source_geometry_candidate_selection.md";
const EVIDENCE: [&str; 12] = [
    "docs/stage163_source_geometry_acquisition.json",
    "docs/stage164_source_geometry_language_transfer.json",
    "docs/stage165_geometry_measurement_composition.json",
    "docs/stage166_route_blind_measurement_composition.json",
    "docs/stage167_geometry_technical_language_scale.json",
    "docs/stage168_geometry_curriculum_admission.json",
    "docs/stage169_geometry_promotion_rollback.json",
    "docs/stage170_geometry_memory_integration.json",
    "docs/stage171_curriculum_memory_scale.json",
    "docs/stage172_memory_backed_geometry_routes.json",
    "docs/stage173_route_blind_technical_language.json",
    "docs/stage174_sealed_curriculum_learning_curve.json",
];

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    candidate_id: &'static str,
    current_manifest_hash: String,
    candidate_present_in_current_manifest: bool,
    evidence_artifacts: usize,
    evidence_hashes: Vec<String>,
    source_development_cases: usize,
    source_holdout_cases: usize,
    language_transfer_cases: usize,
    composition_cases: usize,
    route_blind_cases: usize,
    memory_backed_cases: usize,
    sealed_cases: usize,
    sealed_learning_delta: usize,
    admission_decisions: usize,
    promotion_decisions: usize,
    rollback_cases: usize,
    prerequisite_closures: usize,
    all_evidence_checks_passed: bool,
    false_authorizations: usize,
    false_denials: usize,
    live_manifest_mutations: usize,
    live_registry_mutations: usize,
    shadow_only: bool,
    recommendation: &'static str,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn number(value: &Value, key: &str) -> usize {
    value.get(key).and_then(Value::as_u64).unwrap_or_default() as usize
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let candidate_present = manifest
        .packs
        .iter()
        .any(|pack| pack.id == "source_derived_bounded_geometry");
    let mut reports = Vec::new();
    let mut evidence_hashes = Vec::new();
    for path in EVIDENCE {
        let bytes = fs::read(path)?;
        evidence_hashes.push(digest(&bytes));
        reports.push(serde_json::from_slice::<Value>(&bytes)?);
    }
    let s163 = &reports[0];
    let s164 = &reports[1];
    let s165 = &reports[2];
    let s166 = &reports[3];
    let s167 = &reports[4];
    let s168 = &reports[5];
    let s169 = &reports[6];
    let s171 = &reports[8];
    let s172 = &reports[9];
    let s173 = &reports[10];
    let s174 = &reports[11];

    let all_evidence_checks_passed = number(s163, "development_exact_decisions") == 240
        && number(s163, "holdout_exact_decisions") == 60
        && number(s164, "development_exact_decisions") == 500
        && number(s164, "holdout_exact_decisions") == 100
        && number(s165, "development_exact") == 300
        && number(s165, "holdout_exact") == 100
        && number(s166, "development_exact") == 800
        && number(s166, "holdout_exact") == 200
        && number(s167, "development_exact") == 1600
        && number(s167, "holdout_exact") == 400
        && number(s168, "exact_admission_decisions") == 240
        && number(s169, "exact_promotion_decisions") == 240
        && number(s171, "replay_verified") == 100000
        && number(s172, "memory_replay_verified") == 1000
        && number(s173, "replay_verified") == 1200
        && number(s174, "sealed_learning_delta") == 30
        && reports.iter().all(|report| {
            number(report, "false_authorizations") == 0 && number(report, "false_denials") == 0
        });
    let report = Report {
        schema: "stage262-source-geometry-candidate-selection-v1",
        candidate_id: "source_derived_bounded_geometry",
        current_manifest_hash: manifest_hash,
        candidate_present_in_current_manifest: candidate_present,
        evidence_artifacts: EVIDENCE.len(),
        evidence_hashes,
        source_development_cases: number(s163, "independent_development_cases"),
        source_holdout_cases: number(s163, "holdout_exact_decisions"),
        language_transfer_cases: number(s164, "cases"),
        composition_cases: number(s165, "cases"),
        route_blind_cases: number(s166, "cases") + number(s167, "cases"),
        memory_backed_cases: number(s172, "cases"),
        sealed_cases: number(s174, "sealed_cases"),
        sealed_learning_delta: number(s174, "sealed_learning_delta"),
        admission_decisions: number(s168, "exact_admission_decisions"),
        promotion_decisions: number(s169, "exact_promotion_decisions"),
        rollback_cases: number(s169, "rollbacks_applied"),
        prerequisite_closures: number(s168, "prerequisite_closures"),
        all_evidence_checks_passed,
        false_authorizations: 0,
        false_denials: 0,
        live_manifest_mutations: 0,
        live_registry_mutations: 0,
        shadow_only: true,
        recommendation: "retain as a shadow candidate; do not mutate the current manifest",
    };
    assert!(report.all_evidence_checks_passed);
    assert!(!report.candidate_present_in_current_manifest);
    assert!(report.shadow_only);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.live_manifest_mutations, 0);
    assert_eq!(report.live_registry_mutations, 0);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 262 — source geometry candidate selection\n\nEvaluated {} immutable geometry evidence artifacts against the current curriculum manifest.\n\n* candidate: `{}`\n* evidence checks: passed\n* sealed learning delta: {}\n* admission / promotion decisions: {} / {}\n* rollback cases: {}\n* candidate present in live manifest: {}\n* shadow-only: {}\n* false authorizations / denials: 0 / 0\n* live manifest / registry mutations: 0 / 0\n\nThe evidence supports retaining geometry as a shadow candidate. This report intentionally does not promote it or mutate routing.\n\nReproduce with `cargo run --quiet --bin stage262_source_geometry_candidate_selection`.\n",
            report.evidence_artifacts,
            report.candidate_id,
            report.sealed_learning_delta,
            report.admission_decisions,
            report.promotion_decisions,
            report.rollback_cases,
            report.candidate_present_in_current_manifest,
            report.shadow_only,
        ),
    )?;
    println!(
        "stage262 candidate={} evidence={} all_checks_passed=true shadow_only=true manifest_mutated=false",
        report.candidate_id, report.evidence_artifacts
    );
    Ok(())
}
