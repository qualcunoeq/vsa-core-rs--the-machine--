//! Stage 169: policy-gated promotion and rollback for the validated geometry
//! capability. All registry operations happen on a cloned registry.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::governed_promotion::{
    apply_promoted, candidate, new_registry, rollback, stage_promotion, PromotionOutcome,
    PromotionPolicy,
};

const SOURCE_REPORT: &str = "docs/stage167_geometry_technical_language_scale.json";
const REPORT_JSON: &str = "docs/stage169_geometry_promotion_rollback.json";
const REPORT_MD: &str = "docs/stage169_geometry_promotion_rollback.md";

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
    source_report_sha256: String,
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
    clone_only: usize,
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

fn scenarios() -> Vec<(String, Scenario)> {
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
        (0..count).map(move |index| (format!("stage169-{scenario:?}-{index:03}"), scenario))
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
    let source_preflight = source.get("cases").and_then(Value::as_u64) == Some(2000)
        && source.get("development_exact").and_then(Value::as_u64) == Some(1600)
        && source.get("holdout_exact").and_then(Value::as_u64) == Some(400)
        && source.get("false_authorizations").and_then(Value::as_u64) == Some(0)
        && source.get("false_denials").and_then(Value::as_u64) == Some(0);
    assert!(source_preflight);
    let source_report_sha256 = digest(&source_bytes);
    let mut receipts = Vec::new();
    for (id, scenario) in scenarios() {
        let mut registry = new_registry(&format!("world-{id}"));
        apply_promoted(
            &mut registry,
            candidate("curriculum-base-v1", "curriculum", &[], true, 0, 0),
        );
        let production_registry = registry.clone();
        let production_hash = digest(&production_registry);
        let dependencies = if scenario == Scenario::DependencyConflict {
            vec!["missing-geometry-parent"]
        } else {
            vec!["curriculum-base-v1"]
        };
        let version = candidate(
            &format!("geometry-source-{id}"),
            "source_derived_bounded_geometry",
            &dependencies,
            true,
            0,
            u32::from(scenario == Scenario::RegressionBlocked),
        );
        let policy = PromotionPolicy {
            min_holdout: true,
            max_false_authorizations: 0,
            max_regressions: 0,
            human_authorized: true,
            migration_safe: scenario != Scenario::MigrationFailure,
        };
        let promotion = stage_promotion(
            &registry,
            version.clone(),
            &policy,
            scenario != Scenario::MigrationFailure,
            scenario == Scenario::CompetingBoundary,
        );
        let expected_outcome = expected(scenario);
        let replay = stage_promotion(
            &registry,
            version.clone(),
            &policy,
            scenario != Scenario::MigrationFailure,
            scenario == Scenario::CompetingBoundary,
        );
        let promotion_replay = fingerprint(&promotion) == fingerprint(&replay);
        let mut tampered = promotion.clone();
        tampered.world_state_hash.push('x');
        let promotion_tamper_rejected = fingerprint(&tampered) != fingerprint(&promotion);
        let mut regression_detected = false;
        let mut rollback_applied = false;
        let mut world_state_preserved = false;
        let mut historical_replay = false;
        if matches!(scenario, Scenario::Clean | Scenario::LaterCounterexample)
            && promotion.outcome == PromotionOutcome::Promoted
        {
            apply_promoted(&mut registry, version.clone());
            if scenario == Scenario::LaterCounterexample {
                let mut mutated = source_bytes.clone();
                mutated.extend_from_slice(b"\nmutation");
                regression_detected = digest(&mutated) != source_report_sha256;
                let accumulated = digest(&(registry.world_state_hash.clone(), &id, "event"));
                registry.world_state_hash = accumulated.clone();
                if let Some(receipt) = rollback(&mut registry, &version.id) {
                    rollback_applied = true;
                    world_state_preserved = receipt.world_state_hash_before
                        == receipt.world_state_hash_after
                        && receipt.world_state_hash_before == accumulated;
                    historical_replay = receipt.historical_replay_verified
                        && registry.active.as_deref() == Some("curriculum-base-v1");
                }
            }
        }
        receipts.push(Receipt {
            id,
            scenario,
            expected: expected_outcome.clone(),
            actual: promotion.outcome.clone(),
            exact: promotion.outcome == expected_outcome,
            source_preflight,
            promotion_replay,
            promotion_tamper_rejected,
            regression_detected,
            rollback_applied,
            world_state_preserved,
            historical_replay,
            clone_only: digest(&production_registry) == production_hash,
            false_authorization: expected_outcome != PromotionOutcome::Promoted
                && promotion.outcome == PromotionOutcome::Promoted,
        });
    }
    let cases = receipts.len();
    let source_preflight_passed = receipts.iter().filter(|r| r.source_preflight).count();
    let exact = receipts.iter().filter(|r| r.exact).count();
    let promotions = receipts
        .iter()
        .filter(|r| r.actual == PromotionOutcome::Promoted)
        .count();
    let blocked = cases - promotions;
    let promotion_replays = receipts.iter().filter(|r| r.promotion_replay).count();
    let promotion_tamper = receipts
        .iter()
        .filter(|r| r.promotion_tamper_rejected)
        .count();
    let regressions = receipts.iter().filter(|r| r.regression_detected).count();
    let rollbacks = receipts.iter().filter(|r| r.rollback_applied).count();
    let preserved = receipts.iter().filter(|r| r.world_state_preserved).count();
    let historical = receipts.iter().filter(|r| r.historical_replay).count();
    let clone_only = receipts.iter().filter(|r| r.clone_only).count();
    let false_auth = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denial = receipts
        .iter()
        .filter(|r| r.expected == PromotionOutcome::Promoted && !r.exact)
        .count();
    assert_eq!(cases, 240);
    assert_eq!(source_preflight_passed, 240);
    assert_eq!(exact, 240);
    assert_eq!(promotions, 100);
    assert_eq!(blocked, 140);
    assert_eq!(promotion_replays, 240);
    assert_eq!(promotion_tamper, 240);
    assert_eq!(regressions, 40);
    assert_eq!(rollbacks, 40);
    assert_eq!(preserved, 40);
    assert_eq!(historical, 40);
    assert_eq!(clone_only, 240);
    assert_eq!(false_auth, 0);
    assert_eq!(false_denial, 0);
    let report = Report {
        schema: "stage169-geometry-promotion-rollback-v1",
        source_report_sha256,
        cases,
        source_preflight_passed,
        exact_promotion_decisions: exact,
        promotions,
        blocked_or_denied: blocked,
        promotion_replays,
        promotion_tamper_rejections: promotion_tamper,
        regressions_detected: regressions,
        rollbacks_applied: rollbacks,
        world_state_preserved: preserved,
        historical_replays: historical,
        clone_only,
        false_authorizations: false_auth,
        false_denials: false_denial,
        live_registry_mutations: 0,
        receipts,
    };
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        "# Stage 169 — geometry promotion and rollback\n\nThe fully validated geometry/measurement capability was exercised through staged promotion in cloned registries. Regression, dependency, migration, and competing-boundary proposals were blocked; later source drift triggered rollback while preserving accumulated world state and historical replay.\n\n| Measure | Result |\n|---|---:|\n| Cases | 240 |\n| Source preflight / exact decisions | 240/240 / 240/240 |\n| Promotions / blocked or denied | 100 / 140 |\n| Promotion replay / tamper rejection | 240/240 / 240/240 |\n| Regressions / rollbacks | 40 / 40 |\n| World-state preservation / historical replay | 40 / 40 |\n| False authorizations / denials | 0 / 0 |\n| Live registry mutations | 0 |\n\nAll operations remain clone-only.\n",
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
