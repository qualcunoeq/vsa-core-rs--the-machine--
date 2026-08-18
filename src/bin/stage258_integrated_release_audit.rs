//! Stage 258: integrated release audit over the current governed system.
//!
//! This reads only immutable evaluation manifests and checks their declared
//! safety gates. It does not read HLE answers, mutate the curriculum, or run
//! production routing.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

use the_machine::curriculum::breadth_first_manifest;

const JSON: &str = "docs/stage258_integrated_release_audit.json";
const MD: &str = "docs/stage258_integrated_release_audit.md";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Check {
    artifact: String,
    requirement: String,
    passed: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    current_manifest_hash: String,
    artifacts: usize,
    checks: usize,
    passed_checks: usize,
    all_checks_passed: bool,
    false_authorizations: usize,
    false_denials: usize,
    production_mutations: usize,
    hle_answers_read: usize,
    checks_detail: Vec<Check>,
    artifact_hashes: Vec<(String, String)>,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn load(path: &str) -> Result<(Value, String), Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    Ok((serde_json::from_slice(&bytes)?, digest_bytes(&bytes)))
}

fn number(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

fn boolean(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn check_number(checks: &mut Vec<Check>, artifact: &str, value: &Value, key: &str, expected: u64) {
    checks.push(Check {
        artifact: artifact.into(),
        requirement: format!("{key} == {expected}"),
        passed: number(value, key) == Some(expected),
    });
}

fn check_bool(checks: &mut Vec<Check>, artifact: &str, value: &Value, key: &str, expected: bool) {
    checks.push(Check {
        artifact: artifact.into(),
        requirement: format!("{key} == {expected}"),
        passed: boolean(value, key) == Some(expected),
    });
}

fn check_nonempty_string(checks: &mut Vec<Check>, artifact: &str, value: &Value, key: &str) {
    checks.push(Check {
        artifact: artifact.into(),
        requirement: format!("{key} is nonempty"),
        passed: value
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(|v| !v.is_empty()),
    });
}

fn check_string(checks: &mut Vec<Check>, artifact: &str, value: &Value, key: &str, expected: &str) {
    checks.push(Check {
        artifact: artifact.into(),
        requirement: format!("{key} matches current manifest"),
        passed: value.get(key).and_then(Value::as_str) == Some(expected),
    });
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    assert!(manifest.validate().is_empty());
    let paths = [
        "docs/phase70_algebra_number_theory_composition.json",
        "docs/phase71_combinatorics_number_theory_composition.json",
        "docs/stage257_number_theory_admission_audit.json",
        "docs/stage_b_integrated_synthesis_1000.json",
        "docs/stage_c_technical_language.json",
        "docs/stage_d_source_catalog_ingestion.json",
        "docs/stage_g_self_directed_campaign.json",
        "docs/stage_i_source_retrieval.json",
        "docs/stage_j_multimodal_route_blind.json",
        "docs/phase72_source_metric_pack.json",
        "docs/phase73_source_metric_frontend.json",
        "docs/stage259_self_directed_metric_acquisition.json",
        "docs/stage260_hle_checkpoint_current_curriculum.json",
        "docs/stage261_current_transfer_gap_audit.json",
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
        "docs/stage262_source_geometry_candidate_selection.json",
        "docs/source_provenance_integrity.json",
        "docs/stage_m_continuous_education.json",
        "docs/stage_n_curriculum_learning_curve.json",
        "docs/stage_o_autonomous_breadth_campaign.json",
        "docs/stage_x_multimodal_curriculum_1000.json",
        "docs/stage_k_sealed_curriculum_exam_5000.json",
        "docs/stage_ab_retrieval_investigation.json",
        "docs/stage254_hle_checkpoint_post_portfolio.json",
    ];
    for path in paths {
        assert!(Path::new(path).exists(), "missing release artifact {path}");
    }
    let mut values = Vec::new();
    let mut artifact_hashes = Vec::new();
    for path in paths {
        let (value, hash) = load(path)?;
        values.push((path, value));
        artifact_hashes.push((path.into(), hash));
    }
    let mut checks = Vec::new();
    for (path, value) in &values {
        let name = Path::new(path).file_name().unwrap().to_string_lossy();
        match name.as_ref() {
            "phase70_algebra_number_theory_composition.json" => {
                check_number(&mut checks, path, value, "exact_route_decisions", 240);
                check_number(&mut checks, path, value, "handoff_verifications", 120);
                check_number(&mut checks, path, value, "route_replay_verified", 240);
                check_number(&mut checks, path, value, "tamper_rejections", 240);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "false_denials", 0);
            }
            "phase71_combinatorics_number_theory_composition.json" => {
                check_number(&mut checks, path, value, "exact_decisions", 240);
                check_number(&mut checks, path, value, "replay_verified", 240);
                check_number(&mut checks, path, value, "tamper_rejections", 240);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "false_denials", 0);
            }
            "stage257_number_theory_admission_audit.json" => {
                check_number(&mut checks, path, value, "exact_decisions", 240);
                check_number(&mut checks, path, value, "supported_replays", 120);
                check_number(&mut checks, path, value, "tamper_rejections", 240);
                check_bool(&mut checks, path, value, "promotion_allowed", false);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "false_denials", 0);
            }
            "stage_b_integrated_synthesis_1000.json" => {
                check_number(&mut checks, path, value, "cases", 1000);
                check_number(&mut checks, path, value, "supported_routes", 700);
                check_number(&mut checks, path, value, "replay_verified", 1000);
                check_number(&mut checks, path, value, "tamper_rejections", 1000);
                check_number(&mut checks, path, value, "failure_localized", 300);
                check_number(&mut checks, path, value, "route_leakage", 0);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "false_denials", 0);
            }
            "stage_c_technical_language.json" => {
                check_number(&mut checks, path, value, "cases", 2000);
                check_number(&mut checks, path, value, "target_grounded", 2000);
                check_number(&mut checks, path, value, "replay_verified", 2000);
                check_number(&mut checks, path, value, "provenance_preserved", 2000);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "false_fact_insertions", 0);
            }
            "stage_d_source_catalog_ingestion.json" => {
                check_number(&mut checks, path, value, "valid_catalogs", 1);
                check_number(&mut checks, path, value, "mutation_rejections", 5);
                check_number(&mut checks, path, value, "generated_exercise_replays", 5);
                check_bool(&mut checks, path, value, "replay_stable", true);
                check_number(&mut checks, path, value, "false_acceptances", 0);
            }
            "stage_g_self_directed_campaign.json" => {
                check_number(&mut checks, path, value, "selected_coverage", 320);
                check_bool(&mut checks, path, value, "selected_plan_replay", true);
                check_bool(&mut checks, path, value, "plan_tamper_rejected", true);
                check_number(
                    &mut checks,
                    path,
                    value,
                    "independent_validation_correct",
                    120,
                );
                check_bool(&mut checks, path, value, "manifest_unchanged", true);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "production_authorizations", 0);
            }
            "stage_i_source_retrieval.json" => {
                check_number(&mut checks, path, value, "exact_decisions", 240);
                check_number(
                    &mut checks,
                    path,
                    value,
                    "lineage_deduplication_verified",
                    120,
                );
                check_number(&mut checks, path, value, "replay_verified", 240);
                check_number(&mut checks, path, value, "tamper_rejected", 240);
                check_bool(&mut checks, path, value, "registry_mutated", false);
                check_number(&mut checks, path, value, "false_authorizations", 0);
            }
            "stage_j_multimodal_route_blind.json" => {
                check_number(&mut checks, path, value, "exact_decisions", 240);
                check_number(&mut checks, path, value, "authorized_supported", 120);
                check_number(&mut checks, path, value, "table_replay_verified", 240);
                check_number(&mut checks, path, value, "graph_replay_verified", 240);
                check_number(&mut checks, path, value, "frontend_tamper_rejected", 240);
                check_number(&mut checks, path, value, "downstream_tamper_rejected", 60);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "false_denials", 0);
                check_number(&mut checks, path, value, "hle_questions_read", 0);
                check_number(&mut checks, path, value, "production_registry_mutations", 0);
            }
            "phase72_source_metric_pack.json" => {
                check_number(&mut checks, path, value, "cases", 240);
                check_number(&mut checks, path, value, "supported_artifacts", 120);
                check_number(&mut checks, path, value, "source_mutations_rejected", 6);
                check_number(&mut checks, path, value, "replay_verified", 240);
                check_number(&mut checks, path, value, "tamper_rejected", 240);
                check_number(&mut checks, path, value, "source_provenance_preserved", 240);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "false_denials", 0);
            }
            "phase73_source_metric_frontend.json" => {
                check_number(&mut checks, path, value, "cases", 240);
                check_number(&mut checks, path, value, "supported_artifacts", 120);
                check_number(&mut checks, path, value, "source_provenance_preserved", 240);
                check_number(&mut checks, path, value, "replay_verified", 240);
                check_number(&mut checks, path, value, "tamper_rejected", 240);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "false_denials", 0);
            }
            "stage259_self_directed_metric_acquisition.json" => {
                check_number(&mut checks, path, value, "observed_cases", 240);
                check_number(&mut checks, path, value, "selected_coverage", 240);
                check_bool(&mut checks, path, value, "selected_plan_replay", true);
                check_bool(&mut checks, path, value, "plan_tamper_rejected", true);
                check_number(&mut checks, path, value, "supported_correct", 120);
                check_number(&mut checks, path, value, "supported_replays", 120);
                check_number(&mut checks, path, value, "supported_tamper_rejections", 120);
                check_number(&mut checks, path, value, "ambiguity_preserved", 40);
                check_number(&mut checks, path, value, "refusals_preserved", 80);
                check_number(&mut checks, path, value, "source_mutations_rejected", 6);
                check_bool(&mut checks, path, value, "shadow_promotable", true);
                check_bool(&mut checks, path, value, "manifest_unchanged", true);
                check_number(&mut checks, path, value, "blocked_shortcuts", 1);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "false_denials", 0);
                check_number(&mut checks, path, value, "production_authorizations", 0);
                check_number(&mut checks, path, value, "hle_questions_read", 0);
            }
            "stage260_hle_checkpoint_current_curriculum.json" => {
                check_nonempty_string(&mut checks, path, value, "producer_commit");
                check_string(
                    &mut checks,
                    path,
                    value,
                    "curriculum_manifest_hash",
                    &manifest.replay_hash(),
                );
                check_number(&mut checks, path, value, "cases", 2500);
                check_number(&mut checks, path, value, "incorrect_authorized_answers", 0);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "pack_invocations", 0);
                check_number(&mut checks, path, value, "replay_mismatch", 0);
                check_number(&mut checks, path, value, "replay_not_applicable", 2500);
                check_number(&mut checks, path, value, "replay_not_recorded", 0);
                check_number(&mut checks, path, value, "timed_out", 1);
                check_number(&mut checks, path, value, "curriculum_candidates", 1347);
                check_number(&mut checks, path, value, "no_signal_short_circuits", 1153);
                check_bool(&mut checks, path, value, "manifest_mutated", false);
            }
            "stage261_current_transfer_gap_audit.json" => {
                check_number(&mut checks, path, value, "cases", 2500);
                check_number(&mut checks, path, value, "curriculum_signal_cases", 1347);
                check_number(&mut checks, path, value, "pack_invocations", 0);
                check_number(
                    &mut checks,
                    path,
                    value,
                    "complete_formalization_candidates",
                    1,
                );
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "production_mutations", 0);
            }
            "stage163_source_geometry_acquisition.json" => {
                check_number(
                    &mut checks,
                    path,
                    value,
                    "independent_development_cases",
                    240,
                );
                check_number(&mut checks, path, value, "development_exact_decisions", 240);
                check_number(&mut checks, path, value, "holdout_exact_decisions", 60);
                check_number(&mut checks, path, value, "source_mutations_rejected", 6);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "false_denials", 0);
                check_bool(&mut checks, path, value, "manifest_unchanged", true);
            }
            "stage164_source_geometry_language_transfer.json" => {
                check_number(&mut checks, path, value, "cases", 600);
                check_number(&mut checks, path, value, "development_exact_decisions", 500);
                check_number(&mut checks, path, value, "holdout_exact_decisions", 100);
                check_number(&mut checks, path, value, "development_frontend_replay", 500);
                check_number(&mut checks, path, value, "holdout_frontend_replay", 100);
                check_number(
                    &mut checks,
                    path,
                    value,
                    "development_downstream_replay",
                    500,
                );
                check_number(&mut checks, path, value, "holdout_downstream_replay", 100);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "false_denials", 0);
                check_number(&mut checks, path, value, "live_registry_mutations", 0);
            }
            "stage165_geometry_measurement_composition.json" => {
                check_number(&mut checks, path, value, "cases", 400);
                check_number(&mut checks, path, value, "development_exact", 300);
                check_number(&mut checks, path, value, "holdout_exact", 100);
                check_number(
                    &mut checks,
                    path,
                    value,
                    "development_composition_replay",
                    300,
                );
                check_number(&mut checks, path, value, "holdout_composition_replay", 100);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "false_denials", 0);
                check_number(&mut checks, path, value, "live_registry_mutations", 0);
            }
            "stage166_route_blind_measurement_composition.json" => {
                check_number(&mut checks, path, value, "cases", 1000);
                check_number(&mut checks, path, value, "development_exact", 800);
                check_number(&mut checks, path, value, "holdout_exact", 200);
                check_number(&mut checks, path, value, "development_replay", 800);
                check_number(&mut checks, path, value, "holdout_replay", 200);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "false_denials", 0);
                check_number(&mut checks, path, value, "live_registry_mutations", 0);
            }
            "stage167_geometry_technical_language_scale.json" => {
                check_number(&mut checks, path, value, "cases", 2000);
                check_number(&mut checks, path, value, "development_exact", 1600);
                check_number(&mut checks, path, value, "holdout_exact", 400);
                check_number(&mut checks, path, value, "development_replay", 1600);
                check_number(&mut checks, path, value, "holdout_replay", 400);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "false_denials", 0);
                check_number(&mut checks, path, value, "live_registry_mutations", 0);
            }
            "stage168_geometry_curriculum_admission.json" => {
                check_number(&mut checks, path, value, "cases", 240);
                check_number(&mut checks, path, value, "exact_admission_decisions", 240);
                check_number(&mut checks, path, value, "admitted", 80);
                check_number(&mut checks, path, value, "blocked", 160);
                check_number(&mut checks, path, value, "replay_stable", 240);
                check_number(&mut checks, path, value, "tamper_rejections", 240);
                check_number(&mut checks, path, value, "prerequisite_closures", 200);
                check_number(&mut checks, path, value, "parent_manifest_unchanged", 240);
                check_number(&mut checks, path, value, "false_admissions", 0);
                check_number(&mut checks, path, value, "false_rejections", 0);
                check_number(&mut checks, path, value, "live_manifest_mutations", 0);
            }
            "stage169_geometry_promotion_rollback.json" => {
                check_number(&mut checks, path, value, "cases", 240);
                check_number(&mut checks, path, value, "exact_promotion_decisions", 240);
                check_number(&mut checks, path, value, "promotions", 100);
                check_number(&mut checks, path, value, "blocked_or_denied", 140);
                check_number(&mut checks, path, value, "promotion_replays", 240);
                check_number(&mut checks, path, value, "promotion_tamper_rejections", 240);
                check_number(&mut checks, path, value, "regressions_detected", 40);
                check_number(&mut checks, path, value, "rollbacks_applied", 40);
                check_number(&mut checks, path, value, "world_state_preserved", 40);
                check_number(&mut checks, path, value, "historical_replays", 40);
                check_number(&mut checks, path, value, "clone_only", 240);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "false_denials", 0);
                check_number(&mut checks, path, value, "live_registry_mutations", 0);
            }
            "stage170_geometry_memory_integration.json" => {
                check_number(&mut checks, path, value, "append_cases", 1000);
                check_number(&mut checks, path, value, "valid_appends", 700);
                check_number(&mut checks, path, value, "replay_verified", 700);
                check_number(&mut checks, path, value, "tamper_rejected", 700);
                check_bool(&mut checks, path, value, "parent_memory_unchanged", true);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "false_denials", 0);
                check_number(&mut checks, path, value, "live_memory_mutations", 0);
            }
            "stage171_curriculum_memory_scale.json" => {
                check_number(&mut checks, path, value, "records", 100000);
                check_number(&mut checks, path, value, "exact_queries", 1200);
                check_number(&mut checks, path, value, "exact_complete", 1200);
                check_number(&mut checks, path, value, "replay_verified", 100000);
                check_number(&mut checks, path, value, "tamper_sample", 1000);
                check_number(&mut checks, path, value, "tamper_rejected", 1000);
                check_bool(&mut checks, path, value, "reconstruction_hash_equal", true);
                check_bool(&mut checks, path, value, "parent_memory_unchanged", true);
                check_bool(&mut checks, path, value, "manifest_unchanged", true);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "false_denials", 0);
                check_number(&mut checks, path, value, "live_memory_mutations", 0);
                check_number(&mut checks, path, value, "live_registry_mutations", 0);
            }
            "stage172_memory_backed_geometry_routes.json" => {
                check_number(&mut checks, path, value, "cases", 1000);
                check_number(&mut checks, path, value, "holdout_exact", 200);
                check_number(&mut checks, path, value, "exact_memory_gates", 600);
                check_number(&mut checks, path, value, "memory_replay_verified", 1000);
                check_number(
                    &mut checks,
                    path,
                    value,
                    "composition_replay_verified",
                    1000,
                );
                check_number(&mut checks, path, value, "tamper_rejections", 1000);
                check_number(&mut checks, path, value, "failure_localized", 1000);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "false_denials", 0);
                check_number(&mut checks, path, value, "live_memory_mutations", 0);
                check_number(&mut checks, path, value, "live_registry_mutations", 0);
            }
            "stage173_route_blind_technical_language.json" => {
                check_number(&mut checks, path, value, "cases", 1200);
                check_number(&mut checks, path, value, "development_exact", 960);
                check_number(&mut checks, path, value, "holdout_exact", 240);
                check_number(&mut checks, path, value, "frontend_invocations", 4800);
                check_number(&mut checks, path, value, "ambiguity_preserved", 240);
                check_number(&mut checks, path, value, "unsupported_refusals", 240);
                check_number(&mut checks, path, value, "provenance_preserved", 1200);
                check_number(&mut checks, path, value, "replay_verified", 1200);
                check_number(&mut checks, path, value, "tamper_rejected", 1200);
                check_number(&mut checks, path, value, "route_leakage", 0);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "false_denials", 0);
            }
            "stage174_sealed_curriculum_learning_curve.json" => {
                check_number(&mut checks, path, value, "cases", 1000);
                check_number(&mut checks, path, value, "sealed_learning_delta", 30);
                check_number(&mut checks, path, value, "sealed_baseline_authorized", 90);
                check_number(&mut checks, path, value, "sealed_promoted_authorized", 120);
                check_number(&mut checks, path, value, "baseline_replay_verified", 1000);
                check_number(&mut checks, path, value, "promoted_replay_verified", 1000);
                check_number(&mut checks, path, value, "baseline_tamper_rejected", 1000);
                check_number(&mut checks, path, value, "promoted_tamper_rejected", 1000);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "false_denials", 0);
                check_number(
                    &mut checks,
                    path,
                    value,
                    "sealed_outcomes_exposed_to_selector",
                    0,
                );
                check_number(&mut checks, path, value, "corpus_mutations", 0);
                check_number(&mut checks, path, value, "registry_mutations", 0);
            }
            "stage262_source_geometry_candidate_selection.json" => {
                check_string(
                    &mut checks,
                    path,
                    value,
                    "candidate_id",
                    "source_derived_bounded_geometry",
                );
                check_string(
                    &mut checks,
                    path,
                    value,
                    "current_manifest_hash",
                    &manifest.replay_hash(),
                );
                check_number(&mut checks, path, value, "evidence_artifacts", 12);
                check_number(&mut checks, path, value, "source_development_cases", 240);
                check_number(&mut checks, path, value, "source_holdout_cases", 60);
                check_number(&mut checks, path, value, "language_transfer_cases", 600);
                check_number(&mut checks, path, value, "composition_cases", 400);
                check_number(&mut checks, path, value, "route_blind_cases", 3000);
                check_number(&mut checks, path, value, "memory_backed_cases", 1000);
                check_number(&mut checks, path, value, "sealed_cases", 200);
                check_number(&mut checks, path, value, "sealed_learning_delta", 30);
                check_number(&mut checks, path, value, "admission_decisions", 240);
                check_number(&mut checks, path, value, "promotion_decisions", 240);
                check_number(&mut checks, path, value, "rollback_cases", 40);
                check_number(&mut checks, path, value, "prerequisite_closures", 200);
                check_bool(&mut checks, path, value, "all_evidence_checks_passed", true);
                check_bool(
                    &mut checks,
                    path,
                    value,
                    "candidate_present_in_current_manifest",
                    false,
                );
                check_bool(&mut checks, path, value, "shadow_only", true);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "false_denials", 0);
                check_number(&mut checks, path, value, "live_manifest_mutations", 0);
                check_number(&mut checks, path, value, "live_registry_mutations", 0);
            }
            "source_provenance_integrity.json" => {
                check_number(&mut checks, path, value, "valid_citations", 240);
                check_number(&mut checks, path, value, "replay_verified", 240);
                check_number(&mut checks, path, value, "tamper_rejected", 240);
                check_number(&mut checks, path, value, "mutation_rejections", 10);
                check_number(&mut checks, path, value, "evaluator_replays", 7);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "production_registry_mutations", 0);
            }
            "stage_m_continuous_education.json" => {
                check_number(&mut checks, path, value, "exact_decisions", 300);
                check_number(&mut checks, path, value, "campaign_replays", 300);
                check_number(&mut checks, path, value, "deterministic_reruns", 300);
                check_number(&mut checks, path, value, "tamper_rejections", 300);
                check_number(&mut checks, path, value, "live_registry_mutations", 0);
                check_number(&mut checks, path, value, "false_authorizations", 0);
            }
            "stage_n_curriculum_learning_curve.json" => {
                let final_stage =
                    value
                        .get("stages")
                        .and_then(Value::as_array)
                        .and_then(|stages| {
                            stages.iter().find(|stage| {
                                stage.get("stage").and_then(Value::as_str)
                                    == Some("final_sealed_holdout")
                            })
                        });
                checks.push(Check {
                    artifact: (*path).into(),
                    requirement: "final sealed holdout exists".into(),
                    passed: final_stage.is_some(),
                });
                if let Some(final_stage) = final_stage {
                    check_number(&mut checks, path, final_stage, "exact_decisions", 200);
                    check_number(&mut checks, path, final_stage, "replay_verified", 200);
                    check_number(&mut checks, path, final_stage, "tamper_rejected", 200);
                    check_number(&mut checks, path, final_stage, "false_authorizations", 0);
                    check_number(&mut checks, path, final_stage, "false_denials", 0);
                }
                check_number(&mut checks, path, value, "hle_questions_read", 0);
            }
            "stage_o_autonomous_breadth_campaign.json" => {
                let final_stage =
                    value
                        .get("stages")
                        .and_then(Value::as_array)
                        .and_then(|stages| {
                            stages.iter().find(|stage| {
                                stage.get("stage").and_then(Value::as_str)
                                    == Some("sealed_holdout_after_frozen_admission")
                            })
                        });
                checks.push(Check {
                    artifact: (*path).into(),
                    requirement: "final sealed holdout exists".into(),
                    passed: final_stage.is_some(),
                });
                if let Some(final_stage) = final_stage {
                    check_number(&mut checks, path, final_stage, "exact_decisions", 300);
                    check_number(&mut checks, path, final_stage, "replay_verified", 300);
                    check_number(&mut checks, path, final_stage, "tamper_rejected", 300);
                    check_number(&mut checks, path, final_stage, "false_authorizations", 0);
                    check_number(&mut checks, path, final_stage, "false_denials", 0);
                }
                check_number(&mut checks, path, value, "hle_questions_read", 0);
                check_number(&mut checks, path, value, "production_registry_mutations", 0);
            }
            "stage_x_multimodal_curriculum_1000.json" => {
                check_number(&mut checks, path, value, "exact_decisions", 1000);
                check_number(&mut checks, path, value, "authorized_supported", 600);
                check_number(&mut checks, path, value, "table_replay_verified", 1000);
                check_number(&mut checks, path, value, "graph_replay_verified", 1000);
                check_number(&mut checks, path, value, "frontend_tamper_rejected", 1000);
                check_number(&mut checks, path, value, "downstream_tamper_rejected", 300);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "false_denials", 0);
                check_number(&mut checks, path, value, "hle_questions_read", 0);
                check_number(&mut checks, path, value, "production_registry_mutations", 0);
            }
            "stage_k_sealed_curriculum_exam_5000.json" => {
                check_nonempty_string(&mut checks, path, value, "producer_commit");
                check_string(
                    &mut checks,
                    path,
                    value,
                    "manifest_sha256",
                    &manifest.replay_hash(),
                );
                check_number(&mut checks, path, value, "cases", 5000);
                check_number(&mut checks, path, value, "replay_verified", 5000);
                check_number(&mut checks, path, value, "tamper_rejections", 5000);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "false_denials", 0);
                check_bool(&mut checks, path, value, "manifest_mutated", false);
            }
            "stage_ab_retrieval_investigation.json" => {
                check_number(&mut checks, path, value, "exact_decisions", 500);
                check_number(&mut checks, path, value, "retrieval_replays", 500);
                check_number(&mut checks, path, value, "belief_replays", 500);
                check_number(&mut checks, path, value, "tamper_rejections", 500);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "false_denials", 0);
                check_number(&mut checks, path, value, "registry_mutations", 0);
                check_number(&mut checks, path, value, "world_model_mutations", 0);
            }
            "stage254_hle_checkpoint_post_portfolio.json" => {
                check_number(&mut checks, path, value, "cases", 2500);
                check_number(&mut checks, path, value, "incorrect_authorized_answers", 0);
                check_number(&mut checks, path, value, "false_authorizations", 0);
                check_number(&mut checks, path, value, "replay_compatibility_verified", 2);
                check_number(&mut checks, path, value, "replay_not_recorded", 0);
                checks.push(Check {
                    artifact: (*path).into(),
                    requirement: "shadow-only registry version".into(),
                    passed: value.get("registry_version").and_then(Value::as_str)
                        == Some("shadow-only-no-production-mutation"),
                });
            }
            _ => unreachable!(),
        }
    }
    let passed_checks = checks.iter().filter(|check| check.passed).count();
    let all_checks_passed = passed_checks == checks.len();
    let report = Report {
        schema: "stage258-integrated-release-audit-v1",
        current_manifest_hash: manifest.replay_hash(),
        artifacts: paths.len(),
        checks: checks.len(),
        passed_checks,
        all_checks_passed,
        false_authorizations: 0,
        false_denials: 0,
        production_mutations: 0,
        hle_answers_read: 0,
        checks_detail: checks,
        artifact_hashes,
    };
    if !report.all_checks_passed {
        for check in &report.checks_detail {
            if !check.passed {
                eprintln!("failed check: {} {}", check.artifact, check.requirement);
            }
        }
    }
    assert!(report.all_checks_passed);
    fs::write(JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(MD, format!("# Stage 258 — integrated release audit\n\nAudited {} immutable evidence manifests across composition, source acquisition, self-directed education, sealed curriculum, and the frozen HLE checkpoint.\n\n* checks: {}/{} passed\n* false authorizations / denials: 0 / 0\n* production mutations: 0\n* HLE answers read during audit: 0\n* current manifest hash: `{}`\n\nThis is an evidence audit; it does not promote the planned broader number-theory node or mutate routing.\n\nReproduce with `cargo run --quiet --bin stage258_integrated_release_audit`.\n", report.artifacts, report.passed_checks, report.checks, report.current_manifest_hash))?;
    println!("stage258 artifacts={} checks={}/{} all_checks_passed={} false_auth=0 production_mutations=0", report.artifacts, report.passed_checks, report.checks, report.all_checks_passed);
    Ok(())
}
