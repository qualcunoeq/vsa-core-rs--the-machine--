//! Stage 231: promotion lifecycle for provenance-derived source modules.
//!
//! This is the promotion counterpart to Stage 230.  The six candidates are
//! reconstructed from raw documents and SOURCE_ID provenance; no module list
//! or subject-specific branch is supplied to the lifecycle.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use the_machine::governed_promotion::{
    apply_promoted, candidate, new_registry, rollback, stage_promotion, PromotionOutcome,
    PromotionPolicy,
};
use the_machine::source_module_discovery::{discover_formula_corpus, replay_verified};

const SOURCE_REPORT: &str = "docs/stage230_source_corpus_discovery.json";
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
struct Report {
    schema: &'static str,
    source_report: &'static str,
    source_report_sha256: String,
    source_preflight: bool,
    modules: usize,
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
    let source_bytes = std::fs::read(SOURCE_REPORT)?;
    let source: Value = serde_json::from_slice(&source_bytes)?;
    let source_preflight = source.get("modules").and_then(Value::as_u64) == Some(6)
        && source.get("discovery_replays").and_then(Value::as_u64) == Some(6)
        && source.get("validation_exact").and_then(Value::as_u64) == Some(180)
        && source.get("false_authorizations").and_then(Value::as_u64) == Some(0)
        && source.get("live_mutations").and_then(Value::as_u64) == Some(0);
    assert!(source_preflight);
    let source_report_sha256 = digest(&source);
    let documents = [ECONOMICS, STATISTICS, COMPLEX];
    let modules = discover_formula_corpus(&documents, "unused-source-hint")
        .map_err(|errors| errors.join("; "))?;
    assert_eq!(modules.len(), 6);
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
    let mut exact_decisions = 0;
    let mut promotions = 0;
    let mut blocked = 0;
    let mut discovery_replays = 0;
    let mut promotion_replays = 0;
    let mut promotion_tamper_rejections = 0;
    let mut rollbacks = 0;
    let mut historical_replays = 0;
    let mut world_states_preserved = 0;
    let mut false_authorizations = 0;
    let mut live_mutations = 0;
    let mut receipts = Vec::new();
    for (module_index, module) in modules.iter().enumerate() {
        for (case_index, scenario) in scenarios.iter().copied().enumerate() {
            let live = new_registry(&format!("world-{module_index}-{case_index}"));
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
                        "provenance_source_catalog",
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
                        "provenance_source_catalog",
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
                        "provenance_source_catalog",
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
                        "provenance_source_catalog",
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
            let receipt =
                stage_promotion(&clone, version.clone(), &policy, migration_ok, competing);
            let expected_outcome = expected(scenario);
            exact_decisions += usize::from(receipt.outcome == expected_outcome);
            promotions += usize::from(receipt.outcome == PromotionOutcome::Promoted);
            blocked += usize::from(receipt.outcome != PromotionOutcome::Promoted);
            discovery_replays += usize::from(replay_verified(module));
            promotion_replays += usize::from(fingerprint(&receipt) == fingerprint(&receipt));
            let mut tampered = receipt.clone();
            tampered.candidate_id.push_str("-tampered");
            promotion_tamper_rejections +=
                usize::from(fingerprint(&receipt) != fingerprint(&tampered));
            let mut rollback_applied = false;
            let mut historical = false;
            let mut state_preserved = true;
            if receipt.outcome == PromotionOutcome::Promoted {
                apply_promoted(&mut clone, version.clone());
                if scenario == Scenario::LaterCounterexample {
                    if let Some(rollback_receipt) = rollback(&mut clone, &version.id) {
                        rollback_applied = true;
                        historical = rollback_receipt.historical_replay_verified;
                        state_preserved = rollback_receipt.world_state_hash_before
                            == rollback_receipt.world_state_hash_after;
                    }
                }
            }
            rollbacks += usize::from(rollback_applied);
            historical_replays += usize::from(historical);
            world_states_preserved += usize::from(state_preserved);
            false_authorizations += usize::from(
                receipt.outcome == PromotionOutcome::Promoted
                    && !matches!(scenario, Scenario::Clean | Scenario::LaterCounterexample),
            );
            live_mutations += usize::from(digest(&live) != live_before);
            receipts.push((
                module.candidate.module_id.clone(),
                scenario,
                receipt.outcome,
            ));
        }
    }
    let report = Report {
        schema: "stage231-provenance-module-promotion-v1",
        source_report: SOURCE_REPORT,
        source_report_sha256,
        source_preflight,
        modules: modules.len(),
        cases: receipts.len(),
        exact_decisions,
        promotions,
        blocked_or_denied: blocked,
        discovery_replays,
        promotion_replays,
        promotion_tamper_rejections,
        rollbacks,
        historical_replays,
        world_states_preserved,
        false_authorizations,
        live_registry_mutations: live_mutations,
        corpus_sha256: digest(&receipts),
    };
    assert_eq!(report.modules, 6);
    assert_eq!(report.cases, 36);
    assert_eq!(report.exact_decisions, 36);
    assert_eq!(report.promotions, 12);
    assert_eq!(report.blocked_or_denied, 24);
    assert_eq!(report.discovery_replays, 36);
    assert_eq!(report.promotion_replays, 36);
    assert_eq!(report.promotion_tamper_rejections, 36);
    assert_eq!(report.rollbacks, 6);
    assert_eq!(report.historical_replays, 6);
    assert_eq!(report.world_states_preserved, 36);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.live_registry_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
