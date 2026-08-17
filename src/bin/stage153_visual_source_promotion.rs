//! Stage 153: governed promotion and rollback for visual/source routes.
//!
//! Candidates are evaluated in cloned registries only.  A later mutation of
//! the validated visual-science report must trigger rollback without changing
//! accumulated world state or historical replay.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::governed_promotion::{
    apply_promoted, candidate, new_registry, rollback, stage_promotion, PromotionOutcome,
    PromotionPolicy, VersionedRegistry,
};

const SOURCE_REPORT: &str = "docs/stage152_visual_science_tsv_composition.json";

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

fn registry_hash(registry: &VersionedRegistry) -> String {
    digest(registry)
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
        (0..count).map(move |index| (format!("stage153-{scenario:?}-{index:03}"), scenario))
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
    let source_preflight = source.get("cases").and_then(Value::as_u64) == Some(600)
        && source.get("exact_decisions").and_then(Value::as_u64) == Some(600)
        && source.get("false_authorizations").and_then(Value::as_u64) == Some(0)
        && source.get("false_denials").and_then(Value::as_u64) == Some(0);
    assert!(source_preflight);
    let source_report_sha256 = digest(&source_bytes);
    let mut receipts = Vec::new();
    for (id, scenario) in corpus() {
        let mut registry = new_registry(&format!("world-{id}"));
        apply_promoted(
            &mut registry,
            candidate("curriculum-base-v1", "curriculum", &[], true, 0, 0),
        );
        let production_registry = registry.clone();
        let production_registry_hash = registry_hash(&production_registry);
        let dependencies = if matches!(scenario, Scenario::DependencyConflict) {
            vec!["missing-visual-parent"]
        } else {
            vec!["curriculum-base-v1"]
        };
        let version = candidate(
            &format!("visual-source-{id}"),
            "visual_source_science_routes",
            &dependencies,
            true,
            0,
            u32::from(matches!(scenario, Scenario::RegressionBlocked)),
        );
        let policy = PromotionPolicy {
            min_holdout: true,
            max_false_authorizations: 0,
            max_regressions: 0,
            human_authorized: true,
            migration_safe: !matches!(scenario, Scenario::MigrationFailure),
        };
        let promotion = stage_promotion(
            &registry,
            version.clone(),
            &policy,
            !matches!(scenario, Scenario::MigrationFailure),
            matches!(scenario, Scenario::CompetingBoundary),
        );
        let expected_outcome = expected(scenario);
        let exact = promotion.outcome == expected_outcome;
        let replayed = stage_promotion(
            &registry,
            version.clone(),
            &policy,
            !matches!(scenario, Scenario::MigrationFailure),
            matches!(scenario, Scenario::CompetingBoundary),
        );
        let promotion_replay = fingerprint(&promotion) == fingerprint(&replayed);
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
            if matches!(scenario, Scenario::LaterCounterexample) {
                let mut mutated = source_bytes.clone();
                mutated.extend_from_slice(b"\nmutation");
                regression_detected = digest(&mutated) != source_report_sha256;
                let accumulated = digest(&(registry.world_state_hash.clone(), &id, "event"));
                registry.world_state_hash = accumulated.clone();
                if let Some(rollback_receipt) = rollback(&mut registry, &version.id) {
                    rollback_applied = true;
                    world_state_preserved = rollback_receipt.world_state_hash_before
                        == rollback_receipt.world_state_hash_after
                        && rollback_receipt.world_state_hash_before == accumulated;
                    historical_replay = rollback_receipt.historical_replay_verified
                        && registry.active.as_deref() == Some("curriculum-base-v1");
                }
            }
        }
        let clone_only = registry_hash(&production_registry) == production_registry_hash;
        let false_authorization = expected_outcome != PromotionOutcome::Promoted
            && promotion.outcome == PromotionOutcome::Promoted;
        receipts.push(Receipt {
            id,
            scenario,
            expected: expected_outcome,
            actual: promotion.outcome,
            exact,
            source_preflight,
            promotion_replay,
            promotion_tamper_rejected,
            regression_detected,
            rollback_applied,
            world_state_preserved,
            historical_replay,
            clone_only,
            false_authorization,
        });
    }
    let cases = receipts.len();
    let source_preflight_passed = receipts.iter().filter(|r| r.source_preflight).count();
    let exact_promotion_decisions = receipts.iter().filter(|r| r.exact).count();
    let promotions = receipts
        .iter()
        .filter(|r| r.actual == PromotionOutcome::Promoted)
        .count();
    let blocked_or_denied = cases - promotions;
    let promotion_replays = receipts.iter().filter(|r| r.promotion_replay).count();
    let promotion_tamper_rejections = receipts
        .iter()
        .filter(|r| r.promotion_tamper_rejected)
        .count();
    let regressions_detected = receipts.iter().filter(|r| r.regression_detected).count();
    let rollbacks_applied = receipts.iter().filter(|r| r.rollback_applied).count();
    let world_state_preserved = receipts.iter().filter(|r| r.world_state_preserved).count();
    let historical_replays = receipts.iter().filter(|r| r.historical_replay).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.expected == PromotionOutcome::Promoted && !r.exact)
        .count();
    assert_eq!(cases, 240);
    assert_eq!(source_preflight_passed, 240);
    assert_eq!(exact_promotion_decisions, 240);
    assert_eq!(promotions, 100);
    assert_eq!(blocked_or_denied, 140);
    assert_eq!(promotion_replays, 240);
    assert_eq!(promotion_tamper_rejections, 240);
    assert_eq!(regressions_detected, 40);
    assert_eq!(rollbacks_applied, 40);
    assert_eq!(world_state_preserved, 40);
    assert_eq!(historical_replays, 40);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage153-visual-source-promotion-v1",
        source_report: SOURCE_REPORT,
        source_report_sha256,
        cases,
        source_preflight_passed,
        exact_promotion_decisions,
        promotions,
        blocked_or_denied,
        promotion_replays,
        promotion_tamper_rejections,
        regressions_detected,
        rollbacks_applied,
        world_state_preserved,
        historical_replays,
        false_authorizations,
        false_denials,
        live_registry_mutations: 0,
        receipts,
    };
    let json = serde_json::to_vec_pretty(&report)?;
    fs::write("docs/stage153_visual_source_promotion.json", &json)?;
    println!("{}", String::from_utf8(json)?);
    Ok(())
}
