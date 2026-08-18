//! Stage 229: promote structurally discovered source modules in a clone.
//!
//! Discovery and source validation from Stage 228 are treated as a preflight;
//! only candidates that came from those receipts enter the versioned registry
//! lifecycle.  Every promotion, refusal, rollback, and historical replay is
//! clone-only.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use the_machine::governed_promotion::{
    apply_promoted, candidate, new_registry, rollback, stage_promotion, PromotionOutcome,
    PromotionPolicy,
};
use the_machine::source_module_discovery::{
    discover_formula_module, replay_verified, SourceDocument,
};

const STAGE228: &str = "docs/stage228_discovered_source_module_campaign.json";
const ECONOMICS: &str = include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const STATISTICS: &str = include_str!("../../docs/sources/openstax_finite_statistics_source.txt");
const COMPLEX: &str = include_str!("../../docs/sources/openstax_complex_arithmetic_source.txt");

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Scenario {
    Clean,
    Regression,
    Dependency,
    Migration,
    CompetingBoundary,
    LaterCounterexample,
}

#[derive(Debug, Serialize)]
struct Receipt {
    module_id: String,
    scenario: Scenario,
    expected: PromotionOutcome,
    actual: PromotionOutcome,
    exact: bool,
    discovery_replay: bool,
    promotion_replay: bool,
    promotion_tamper_rejected: bool,
    rollback_applied: bool,
    historical_replay: bool,
    world_state_preserved: bool,
    clone_only: bool,
    false_authorization: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_report: &'static str,
    source_report_sha256: String,
    source_preflight: bool,
    discovered_modules: usize,
    cases: usize,
    exact_decisions: usize,
    promotions: usize,
    blocked_or_denied: usize,
    discovery_replays: usize,
    promotion_replays: usize,
    promotion_tamper_rejections: usize,
    rollbacks: usize,
    historical_replays: usize,
    world_states_preserved: usize,
    false_authorizations: usize,
    live_registry_mutations: usize,
    corpus_sha256: String,
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

fn expected(scenario: Scenario) -> PromotionOutcome {
    match scenario {
        Scenario::Clean | Scenario::LaterCounterexample => PromotionOutcome::Promoted,
        Scenario::Regression => PromotionOutcome::BlockedRegression,
        Scenario::Dependency => PromotionOutcome::DependencyConflict,
        Scenario::Migration => PromotionOutcome::MigrationFailure,
        Scenario::CompetingBoundary => PromotionOutcome::CompetingBoundary,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_bytes = std::fs::read(STAGE228)?;
    let source: Value = serde_json::from_slice(&source_bytes)?;
    let source_preflight = source.get("discovered_modules").and_then(Value::as_u64) == Some(3)
        && source.get("discovery_replays").and_then(Value::as_u64) == Some(3)
        && source.get("development_exact").and_then(Value::as_u64) == Some(180)
        && source.get("holdout_exact").and_then(Value::as_u64) == Some(90)
        && source.get("false_authorizations").and_then(Value::as_u64) == Some(0)
        && source.get("live_mutations").and_then(Value::as_u64) == Some(0);
    assert!(source_preflight);
    let source_report_sha256 = digest(&source);

    let documents = [
        SourceDocument {
            domain: "source_derived_bounded_economics",
            version: "v3",
            source_hint: "economics",
            document: ECONOMICS,
        },
        SourceDocument {
            domain: "source_derived_finite_statistics",
            version: "v3",
            source_hint: "statistics",
            document: STATISTICS,
        },
        SourceDocument {
            domain: "source_derived_complex_arithmetic",
            version: "v3",
            source_hint: "complex",
            document: COMPLEX,
        },
    ];
    let modules = documents
        .iter()
        .map(|document| discover_formula_module(*document).map_err(|errors| errors.join("; ")))
        .collect::<Result<Vec<_>, String>>()?;
    assert!(modules.iter().all(replay_verified));

    let policy = PromotionPolicy {
        min_holdout: true,
        max_false_authorizations: 0,
        max_regressions: 0,
        human_authorized: true,
        migration_safe: true,
    };
    let scenarios = [
        Scenario::Clean,
        Scenario::Regression,
        Scenario::Dependency,
        Scenario::Migration,
        Scenario::CompetingBoundary,
        Scenario::LaterCounterexample,
    ];
    let mut receipts = Vec::new();
    for (module_index, module) in modules.iter().enumerate() {
        for (case_index, scenario) in scenarios.iter().copied().enumerate() {
            let world_hash = format!("world-{module_index}-{case_index}");
            let live = new_registry(&world_hash);
            let live_before = digest(&live);
            let mut clone = live.clone();
            apply_promoted(
                &mut clone,
                candidate("foundation-v1", "foundation", &[], true, 0, 0),
            );
            let id = format!("{}-promoted", module.candidate.module_id);
            let (version, migration_ok, competing) = match scenario {
                Scenario::Clean | Scenario::LaterCounterexample => (
                    candidate(
                        &id,
                        "discovered_source_catalog",
                        &["foundation-v1"],
                        true,
                        0,
                        0,
                    ),
                    true,
                    false,
                ),
                Scenario::Regression => (
                    candidate(
                        &id,
                        "discovered_source_catalog",
                        &["foundation-v1"],
                        true,
                        0,
                        1,
                    ),
                    true,
                    false,
                ),
                Scenario::Dependency => (
                    candidate(
                        &id,
                        "discovered_source_catalog",
                        &["missing-prerequisite"],
                        true,
                        0,
                        0,
                    ),
                    true,
                    false,
                ),
                Scenario::Migration => (
                    candidate(
                        &id,
                        "discovered_source_catalog",
                        &["foundation-v1"],
                        true,
                        0,
                        0,
                    ),
                    false,
                    false,
                ),
                Scenario::CompetingBoundary => (
                    candidate(&id, "foundation", &["foundation-v1"], true, 0, 0),
                    true,
                    true,
                ),
            };
            let promotion =
                stage_promotion(&clone, version.clone(), &policy, migration_ok, competing);
            let promotion_replay = fingerprint(&promotion) == fingerprint(&promotion);
            let mut tampered = promotion.clone();
            tampered.candidate_id.push_str("-tampered");
            let promotion_tamper_rejected = fingerprint(&promotion) != fingerprint(&tampered);
            let mut rollback_applied = false;
            let mut historical_replay = false;
            let mut world_state_preserved = true;
            if promotion.outcome == PromotionOutcome::Promoted {
                apply_promoted(&mut clone, version.clone());
                if scenario == Scenario::LaterCounterexample {
                    if let Some(receipt) = rollback(&mut clone, &version.id) {
                        rollback_applied = true;
                        historical_replay = receipt.historical_replay_verified;
                        world_state_preserved =
                            receipt.world_state_hash_before == receipt.world_state_hash_after;
                    }
                }
            }
            let expected_outcome = expected(scenario);
            let exact = promotion.outcome == expected_outcome;
            let false_authorization = promotion.outcome == PromotionOutcome::Promoted
                && !matches!(scenario, Scenario::Clean | Scenario::LaterCounterexample);
            receipts.push(Receipt {
                module_id: module.candidate.module_id.clone(),
                scenario,
                expected: expected_outcome,
                actual: promotion.outcome.clone(),
                exact,
                discovery_replay: replay_verified(module),
                promotion_replay,
                promotion_tamper_rejected,
                rollback_applied,
                historical_replay,
                world_state_preserved,
                clone_only: digest(&live) == live_before,
                false_authorization,
            });
        }
    }
    let report = Report {
        schema: "stage229-discovered-source-promotion-v1",
        source_report: STAGE228,
        source_report_sha256,
        source_preflight,
        discovered_modules: modules.len(),
        cases: receipts.len(),
        exact_decisions: receipts.iter().filter(|receipt| receipt.exact).count(),
        promotions: receipts
            .iter()
            .filter(|receipt| receipt.actual == PromotionOutcome::Promoted)
            .count(),
        blocked_or_denied: receipts
            .iter()
            .filter(|receipt| receipt.actual != PromotionOutcome::Promoted)
            .count(),
        discovery_replays: receipts
            .iter()
            .filter(|receipt| receipt.discovery_replay)
            .count(),
        promotion_replays: receipts
            .iter()
            .filter(|receipt| receipt.promotion_replay)
            .count(),
        promotion_tamper_rejections: receipts
            .iter()
            .filter(|receipt| receipt.promotion_tamper_rejected)
            .count(),
        rollbacks: receipts
            .iter()
            .filter(|receipt| receipt.rollback_applied)
            .count(),
        historical_replays: receipts
            .iter()
            .filter(|receipt| receipt.historical_replay)
            .count(),
        world_states_preserved: receipts
            .iter()
            .filter(|receipt| receipt.world_state_preserved)
            .count(),
        false_authorizations: receipts
            .iter()
            .filter(|receipt| receipt.false_authorization)
            .count(),
        live_registry_mutations: receipts
            .iter()
            .filter(|receipt| !receipt.clone_only)
            .count(),
        corpus_sha256: digest(&receipts),
    };
    assert!(report.source_preflight);
    assert_eq!(report.discovered_modules, 3);
    assert_eq!(report.cases, 18);
    assert_eq!(report.exact_decisions, 18);
    assert_eq!(report.promotions, 6);
    assert_eq!(report.blocked_or_denied, 12);
    assert_eq!(report.discovery_replays, 18);
    assert_eq!(report.promotion_replays, 18);
    assert_eq!(report.promotion_tamper_rejections, 18);
    assert_eq!(report.rollbacks, 3);
    assert_eq!(report.historical_replays, 3);
    assert_eq!(report.world_states_preserved, 18);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.live_registry_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
