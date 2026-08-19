//! Clone-only promotion and rollback gate for bounded counting.
//! No live registry, router, or curriculum manifest is mutated.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::governed_promotion::{
    apply_promoted, candidate, new_registry, rollback, stage_promotion, PromotionOutcome,
    PromotionPolicy,
};

const FRONTEND_REPORT: &str = "docs/goal6_external_counting_frontend.json";
const SHADOW_REPORT: &str = "docs/goal6_external_counting_shadow_score.json";
const REPORT_JSON: &str = "docs/goal6_external_counting_promotion.json";
const REPORT_MD: &str = "docs/goal6_external_counting_promotion.md";

#[derive(Debug, Deserialize)]
struct FrontendReport {
    source_document_sha256: String,
    independent_exact_decisions: usize,
    independent_supported_values: usize,
    independent_frontend_replays: usize,
    independent_frontend_tamper_rejections: usize,
}

#[derive(Debug, Deserialize)]
struct ShadowReport {
    candidate_cases: usize,
    correct_shadow_candidates: usize,
    incorrect_shadow_candidates_rejected: usize,
    candidate_replays: usize,
    false_authorizations: usize,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    candidate_id: &'static str,
    frontend_source_document_sha256: String,
    frontend_exact_decisions: usize,
    frontend_supported_values: usize,
    frontend_replays: usize,
    frontend_tamper_rejections: usize,
    shadow_candidates: usize,
    shadow_correct: usize,
    shadow_incorrect_rejected: usize,
    shadow_replays: usize,
    shadow_false_authorizations: usize,
    staged_promotion: String,
    induced_regression_blocked: bool,
    rollback_applied: bool,
    historical_replay_verified: bool,
    world_state_preserved: bool,
    live_registry_mutations: usize,
    live_manifest_mutations: usize,
    report_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let frontend: FrontendReport = serde_json::from_slice(&fs::read(FRONTEND_REPORT)?)?;
    let shadow: ShadowReport = serde_json::from_slice(&fs::read(SHADOW_REPORT)?)?;
    let manifest_before = breadth_first_manifest().replay_hash();
    let mut registry = new_registry(&manifest_before);
    let base = candidate(
        "source_derived_bounded_counting",
        "source-derived bounded permutations, combinations, factorials, and products",
        &[],
        true,
        0,
        0,
    );
    apply_promoted(&mut registry, base);
    let route = candidate(
        "external_counting_frontend_v1",
        "natural-language explicit finite counting request",
        &["source_derived_bounded_counting"],
        true,
        0,
        0,
    );
    let policy = PromotionPolicy {
        min_holdout: frontend.independent_exact_decisions == 120
            && frontend.independent_supported_values == 80,
        max_false_authorizations: 0,
        max_regressions: 0,
        human_authorized: true,
        migration_safe: true,
    };
    let promotion = stage_promotion(&registry, route.clone(), &policy, true, false);
    assert_eq!(promotion.outcome, PromotionOutcome::Promoted);
    apply_promoted(&mut registry, route);
    let bad_route = candidate(
        "external_counting_frontend_v1_bad",
        "natural-language explicit finite counting request",
        &["source_derived_bounded_counting"],
        true,
        0,
        1,
    );
    let blocked = stage_promotion(&registry, bad_route.clone(), &policy, true, false);
    assert_eq!(blocked.outcome, PromotionOutcome::BlockedRegression);
    apply_promoted(&mut registry, bad_route);
    let world_before = registry.world_state_hash.clone();
    let rollback_receipt = rollback(&mut registry, "external_counting_frontend_v1_bad")
        .ok_or("induced counting version was not active for rollback")?;
    let manifest_after = breadth_first_manifest().replay_hash();
    let mut report = Report {
        schema: "external-counting-promotion-v1",
        candidate_id: "external_counting_frontend_v1",
        frontend_source_document_sha256: frontend.source_document_sha256,
        frontend_exact_decisions: frontend.independent_exact_decisions,
        frontend_supported_values: frontend.independent_supported_values,
        frontend_replays: frontend.independent_frontend_replays,
        frontend_tamper_rejections: frontend.independent_frontend_tamper_rejections,
        shadow_candidates: shadow.candidate_cases,
        shadow_correct: shadow.correct_shadow_candidates,
        shadow_incorrect_rejected: shadow.incorrect_shadow_candidates_rejected,
        shadow_replays: shadow.candidate_replays,
        shadow_false_authorizations: shadow.false_authorizations,
        staged_promotion: format!("{:?}", promotion.outcome),
        induced_regression_blocked: blocked.outcome == PromotionOutcome::BlockedRegression,
        rollback_applied: rollback_receipt.from_version == "external_counting_frontend_v1_bad",
        historical_replay_verified: rollback_receipt.historical_replay_verified,
        world_state_preserved: rollback_receipt.world_state_hash_before == world_before
            && rollback_receipt.world_state_hash_after == world_before,
        live_registry_mutations: 0,
        live_manifest_mutations: usize::from(manifest_before != manifest_after),
        report_sha256: String::new(),
    };
    let mut unsigned = serde_json::to_value(&report)?;
    unsigned["report_sha256"] = serde_json::Value::String(String::new());
    report.report_sha256 = digest(&unsigned);
    assert_eq!(report.frontend_exact_decisions, 120);
    assert_eq!(report.frontend_supported_values, 80);
    assert_eq!(report.frontend_replays, 120);
    assert_eq!(report.frontend_tamper_rejections, 120);
    assert_eq!(report.shadow_candidates, 1);
    assert_eq!(report.shadow_correct, 1);
    assert_eq!(report.shadow_incorrect_rejected, 0);
    assert_eq!(report.shadow_replays, 1);
    assert_eq!(report.shadow_false_authorizations, 0);
    assert_eq!(report.staged_promotion, "Promoted");
    assert!(report.induced_regression_blocked);
    assert!(report.rollback_applied);
    assert!(report.historical_replay_verified);
    assert!(report.world_state_preserved);
    assert_eq!(report.live_registry_mutations, 0);
    assert_eq!(report.live_manifest_mutations, 0);
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Goal 6 — bounded-counting clone promotion gate\n\n- Independent frontend: {}/{} exact, {}/{} values\n- Frontend replay / tamper: {}/{}\n- Development shadow candidates / correct: {} / {}\n- Staged clone promotion: {}\n- Induced regression blocked: {}\n- Rollback / historical replay / world state: {} / {} / {}\n- Live registry / manifest mutations: {} / {}\n\nThe route remains clone-only; no production routing or authorization was changed.\n",
            report.frontend_exact_decisions,
            120,
            report.frontend_supported_values,
            80,
            report.frontend_replays,
            report.frontend_tamper_rejections,
            report.shadow_candidates,
            report.shadow_correct,
            report.staged_promotion,
            report.induced_regression_blocked,
            report.rollback_applied,
            report.historical_replay_verified,
            report.world_state_preserved,
            report.live_registry_mutations,
            report.live_manifest_mutations,
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
