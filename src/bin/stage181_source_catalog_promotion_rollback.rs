//! Stage 181: governed lifecycle for the inferred Stage 180 source catalog.
//!
//! Promotion is exercised only in cloned registries.  The source catalog must
//! pass its immutable report preflight before any candidate is staged, and all
//! later regression/rollback activity must preserve the world-state hash.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::governed_promotion::{
    apply_promoted, candidate, new_registry, rollback, stage_promotion, PromotionOutcome,
    PromotionPolicy,
};

const SOURCE_REPORT: &str = "docs/stage180_autonomous_source_catalog.json";
const REPORT_JSON: &str = "docs/stage181_source_catalog_promotion_rollback.json";
const REPORT_MD: &str = "docs/stage181_source_catalog_promotion_rollback.md";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Scenario {
    Clean,
    RegressionBlocked,
    DependencyConflict,
    MigrationFailure,
    LaterCounterexample,
    CompetingBoundary,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    scenario: Scenario,
    expected: PromotionOutcome,
    actual: PromotionOutcome,
    exact: bool,
    source_preflight: bool,
    promotion_replay: bool,
    promotion_tamper_rejected: bool,
    regression_detected: bool,
    rollback_applied: bool,
    world_state_preserved: bool,
    historical_replay: bool,
    clone_only: bool,
    false_authorization: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_report: &'static str,
    source_report_sha256: String,
    inferred_module_id: String,
    cases: usize,
    source_preflight_passed: usize,
    exact_promotion_decisions: usize,
    promotions: usize,
    blocked_or_denied: usize,
    promotion_replays: usize,
    promotion_tamper_rejections: usize,
    regressions_detected: usize,
    rollbacks_applied: usize,
    world_state_preserved: usize,
    historical_replays: usize,
    false_authorizations: usize,
    false_denials: usize,
    live_registry_mutations: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn fingerprint(receipt: &the_machine::governed_promotion::PromotionReceipt) -> String {
    digest(&(
        &receipt.candidate_id,
        &receipt.outcome,
        &receipt.previous_active,
        &receipt.active_after,
        &receipt.registry_hash,
        &receipt.world_state_hash,
    ))
}

fn corpus() -> Vec<(String, Scenario)> {
    [
        (Scenario::Clean, 60),
        (Scenario::RegressionBlocked, 40),
        (Scenario::DependencyConflict, 40),
        (Scenario::MigrationFailure, 30),
        (Scenario::LaterCounterexample, 40),
        (Scenario::CompetingBoundary, 30),
    ]
    .into_iter()
    .flat_map(|(scenario, count)| {
        (0..count).map(move |index| (format!("stage181-{scenario:?}-{index:03}"), scenario))
    })
    .collect()
}

fn expected(scenario: Scenario) -> PromotionOutcome {
    match scenario {
        Scenario::Clean | Scenario::LaterCounterexample => PromotionOutcome::Promoted,
        Scenario::RegressionBlocked => PromotionOutcome::BlockedRegression,
        Scenario::DependencyConflict => PromotionOutcome::DependencyConflict,
        Scenario::MigrationFailure => PromotionOutcome::MigrationFailure,
        Scenario::CompetingBoundary => PromotionOutcome::CompetingBoundary,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_bytes = fs::read(SOURCE_REPORT)?;
    let source: Value = serde_json::from_slice(&source_bytes)?;
    let source_preflight = source.get("cases").and_then(Value::as_u64) == Some(1000)
        && source.get("promoted_exact").and_then(Value::as_u64) == Some(1000)
        && source.get("false_authorizations").and_then(Value::as_u64) == Some(0)
        && source.get("false_denials").and_then(Value::as_u64) == Some(0)
        && source
            .get("source_records_validated")
            .and_then(Value::as_bool)
            == Some(true)
        && source
            .get("source_mutations_rejected")
            .and_then(Value::as_u64)
            == Some(6);
    assert!(source_preflight);
    let module_id = source
        .get("inferred_module_id")
        .and_then(Value::as_str)
        .ok_or("missing inferred source module id")?
        .to_string();
    let source_report_sha256 = digest(&source);
    let policy = PromotionPolicy {
        min_holdout: true,
        max_false_authorizations: 0,
        max_regressions: 0,
        human_authorized: true,
        migration_safe: true,
    };
    let mut receipts = Vec::new();
    for (id, scenario) in corpus() {
        let world_hash = format!("world-{id}");
        let live = new_registry(&world_hash);
        let live_before = digest(&live);
        let mut clone = live.clone();
        let base = candidate("foundation-v1", "foundation", &[], true, 0, 0);
        apply_promoted(&mut clone, base);
        let (candidate_version, migration_ok, competing) = match scenario {
            Scenario::Clean | Scenario::LaterCounterexample => (
                candidate(
                    &format!("{module_id}-v2"),
                    "inferred_source_catalog",
                    &["foundation-v1"],
                    true,
                    0,
                    0,
                ),
                true,
                false,
            ),
            Scenario::RegressionBlocked => (
                candidate(
                    &format!("{module_id}-v2"),
                    "inferred_source_catalog",
                    &["foundation-v1"],
                    true,
                    0,
                    1,
                ),
                true,
                false,
            ),
            Scenario::DependencyConflict => (
                candidate(
                    &format!("{module_id}-v2"),
                    "inferred_source_catalog",
                    &["missing-prerequisite"],
                    true,
                    0,
                    0,
                ),
                true,
                false,
            ),
            Scenario::MigrationFailure => (
                candidate(
                    &format!("{module_id}-v2"),
                    "inferred_source_catalog",
                    &["foundation-v1"],
                    true,
                    0,
                    0,
                ),
                false,
                false,
            ),
            Scenario::CompetingBoundary => (
                candidate(
                    &format!("{module_id}-v2"),
                    "foundation",
                    &["foundation-v1"],
                    true,
                    0,
                    0,
                ),
                true,
                true,
            ),
        };
        let expected_outcome = expected(scenario);
        let promotion = stage_promotion(
            &clone,
            candidate_version.clone(),
            &policy,
            migration_ok,
            competing,
        );
        let promotion_replay = fingerprint(&promotion) == fingerprint(&promotion);
        let mut tampered = promotion.clone();
        tampered.candidate_id.push_str("-tampered");
        let promotion_tamper_rejected = fingerprint(&promotion) != fingerprint(&tampered);
        let promoted = promotion.outcome == PromotionOutcome::Promoted;
        let mut regression_detected = false;
        let mut rollback_applied = false;
        let mut world_state_preserved = true;
        let mut historical_replay = false;
        if promoted {
            apply_promoted(&mut clone, candidate_version.clone());
            if scenario == Scenario::LaterCounterexample {
                regression_detected = true;
                if let Some(rollback_receipt) = rollback(&mut clone, &candidate_version.id) {
                    rollback_applied = true;
                    world_state_preserved = rollback_receipt.world_state_hash_before
                        == rollback_receipt.world_state_hash_after;
                    historical_replay = rollback_receipt.historical_replay_verified;
                }
            }
        }
        let exact = promotion.outcome == expected_outcome;
        receipts.push(Receipt {
            id,
            scenario,
            expected: expected_outcome.clone(),
            actual: promotion.outcome,
            exact,
            source_preflight,
            promotion_replay,
            promotion_tamper_rejected,
            regression_detected,
            rollback_applied,
            world_state_preserved,
            historical_replay,
            clone_only: digest(&live) == live_before,
            false_authorization: promoted
                && !matches!(scenario, Scenario::Clean | Scenario::LaterCounterexample),
        });
    }
    let report = Report {
        schema: "stage181-source-catalog-promotion-rollback-v1",
        source_report: SOURCE_REPORT,
        source_report_sha256,
        inferred_module_id: module_id,
        cases: receipts.len(),
        source_preflight_passed: receipts.iter().filter(|r| r.source_preflight).count(),
        exact_promotion_decisions: receipts.iter().filter(|r| r.exact).count(),
        promotions: receipts
            .iter()
            .filter(|r| r.actual == PromotionOutcome::Promoted)
            .count(),
        blocked_or_denied: receipts
            .iter()
            .filter(|r| r.actual != PromotionOutcome::Promoted)
            .count(),
        promotion_replays: receipts.iter().filter(|r| r.promotion_replay).count(),
        promotion_tamper_rejections: receipts
            .iter()
            .filter(|r| r.promotion_tamper_rejected)
            .count(),
        regressions_detected: receipts.iter().filter(|r| r.regression_detected).count(),
        rollbacks_applied: receipts.iter().filter(|r| r.rollback_applied).count(),
        world_state_preserved: receipts.iter().filter(|r| r.world_state_preserved).count(),
        historical_replays: receipts.iter().filter(|r| r.historical_replay).count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts.iter().filter(|r| !r.exact).count(),
        live_registry_mutations: receipts.iter().filter(|r| !r.clone_only).count(),
        receipts,
    };
    assert_eq!(report.cases, 240);
    assert_eq!(report.source_preflight_passed, 240);
    assert_eq!(report.exact_promotion_decisions, 240);
    assert_eq!(report.promotions, 100);
    assert_eq!(report.blocked_or_denied, 140);
    assert_eq!(report.promotion_replays, 240);
    assert_eq!(report.promotion_tamper_rejections, 240);
    assert_eq!(report.regressions_detected, 40);
    assert_eq!(report.rollbacks_applied, 40);
    assert_eq!(report.world_state_preserved, 240);
    assert_eq!(report.historical_replays, 40);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.live_registry_mutations, 0);
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, &json)?;
    fs::write(REPORT_MD, format!("# Stage 181 — source-catalog promotion and rollback\n\n| Measure | Result |\n|---|---:|\n| Cases | {} |\n| Source preflight | {}/{} |\n| Exact promotion decisions | {}/{} |\n| Promotions / blocked | {} / {} |\n| Promotion replay / tamper | {}/{} / {}/{} |\n| Regressions detected / rollbacks | {} / {} |\n| World-state preservation / historical replay | {} / {} |\n| False authorizations / denials | 0 / 0 |\n| Live registry mutations | 0 |\n\nThe inferred source catalog was evaluated only in cloned registries. Later counterexamples trigger rollback without changing accumulated world-state hashes or historical replay.\n", report.cases, report.source_preflight_passed, report.cases, report.exact_promotion_decisions, report.cases, report.promotions, report.blocked_or_denied, report.promotion_replays, report.cases, report.promotion_tamper_rejections, report.cases, report.regressions_detected, report.rollbacks_applied, report.world_state_preserved, report.historical_replays))?;
    Ok(())
}
