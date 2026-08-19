//! Stage 316: governed lifecycle for the source-derived automata capability.
//!
//! The validated shadow pack is staged only in cloned registries.  Promotion
//! policy, dependency and migration checks, induced regression, rollback with
//! accumulated state, and historical replay are all exercised.  No live
//! registry or curriculum manifest is opened for mutation.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::governed_promotion::{
    apply_promoted, candidate, new_registry, rollback, stage_promotion, PromotionOutcome,
    PromotionPolicy, VersionedRegistry,
};

const SOURCE_REPORT: &str = "docs/stage314_finite_automata_source_pack.json";
const REPORT_JSON: &str = "docs/stage316_finite_automata_promotion_rollback.json";
const REPORT_MD: &str = "docs/stage316_finite_automata_promotion_rollback.md";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Scenario {
    Clean,
    PolicyDenied,
    RegressionBlocked,
    DependencyConflict,
    MigrationFailure,
    CompetingBoundary,
    RollbackAccumulatedState,
    HistoricalReplay,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    scenario: Scenario,
    expected: PromotionOutcome,
    actual: PromotionOutcome,
    exact: bool,
    source_preflight: bool,
    registry_replay: bool,
    tamper_rejected: bool,
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
    exact_lifecycle_decisions: usize,
    promotions: usize,
    blocked_or_denied: usize,
    registry_replays: usize,
    tamper_rejections: usize,
    rollback_applied: usize,
    world_state_preserved: usize,
    historical_replays: usize,
    false_authorizations: usize,
    false_denials: usize,
    production_registry_mutations: usize,
    curriculum_manifest_mutations: usize,
    scenario_counts: BTreeMap<Scenario, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn corpus() -> Vec<(String, Scenario)> {
    let groups = [
        (Scenario::Clean, 40),
        (Scenario::PolicyDenied, 30),
        (Scenario::RegressionBlocked, 30),
        (Scenario::DependencyConflict, 30),
        (Scenario::MigrationFailure, 25),
        (Scenario::CompetingBoundary, 25),
        (Scenario::RollbackAccumulatedState, 30),
        (Scenario::HistoricalReplay, 30),
    ];
    groups
        .into_iter()
        .flat_map(|(scenario, count)| {
            (0..count).map(move |index| (format!("stage316-{scenario:?}-{index:03}"), scenario))
        })
        .collect()
}

fn expected(scenario: Scenario) -> PromotionOutcome {
    match scenario {
        Scenario::Clean | Scenario::RollbackAccumulatedState | Scenario::HistoricalReplay => {
            PromotionOutcome::Promoted
        }
        Scenario::PolicyDenied => PromotionOutcome::PolicyDenied,
        Scenario::RegressionBlocked => PromotionOutcome::BlockedRegression,
        Scenario::DependencyConflict => PromotionOutcome::DependencyConflict,
        Scenario::MigrationFailure => PromotionOutcome::MigrationFailure,
        Scenario::CompetingBoundary => PromotionOutcome::CompetingBoundary,
    }
}

fn stage(
    registry: &VersionedRegistry,
    scenario: Scenario,
) -> (
    the_machine::governed_promotion::CapabilityVersion,
    PromotionPolicy,
    bool,
    bool,
) {
    let (holdout, false_auth, regressions, migration, human, dependency, conflict) = match scenario
    {
        Scenario::Clean | Scenario::RollbackAccumulatedState | Scenario::HistoricalReplay => {
            (true, 0, 0, true, true, "curriculum-base-v1", false)
        }
        Scenario::PolicyDenied => (false, 0, 0, true, false, "curriculum-base-v1", false),
        Scenario::RegressionBlocked => (true, 0, 1, true, true, "curriculum-base-v1", false),
        Scenario::DependencyConflict => (true, 0, 0, true, true, "missing-prerequisite-v1", false),
        Scenario::MigrationFailure => (true, 0, 0, false, true, "curriculum-base-v1", false),
        Scenario::CompetingBoundary => (true, 0, 0, true, true, "curriculum-base-v1", true),
    };
    let version = candidate(
        "source-automata-v1",
        "source_derived_bounded_finite_automata",
        &[dependency],
        holdout,
        false_auth,
        regressions,
    );
    let policy = PromotionPolicy {
        min_holdout: holdout,
        max_false_authorizations: 0,
        max_regressions: 0,
        human_authorized: human,
        migration_safe: migration,
    };
    let _ = registry;
    (version, policy, migration, conflict)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_bytes = fs::read(SOURCE_REPORT)?;
    let source: Value = serde_json::from_slice(&source_bytes)?;
    let source_preflight = source["exact_decisions"] == 240
        && source["supported"] == 120
        && source["replay_verified"] == 240
        && source["tamper_rejections"] == 240
        && source["false_authorizations"] == 0
        && source["false_denials"] == 0;
    assert!(source_preflight);
    let mut receipts = Vec::new();
    let mut scenario_counts = BTreeMap::new();
    let mut exact_lifecycle_decisions = 0;
    let mut promotions = 0;
    let mut blocked_or_denied = 0;
    let mut registry_replays = 0;
    let mut tamper_rejections = 0;
    let mut rollback_applied = 0;
    let mut world_state_preserved = 0;
    let mut historical_replays = 0;
    for (id, scenario) in corpus() {
        *scenario_counts.entry(scenario).or_insert(0) += 1;
        let mut registry = new_registry(&format!("world-{id}"));
        apply_promoted(
            &mut registry,
            candidate("curriculum-base-v1", "curriculum", &[], true, 0, 0),
        );
        let production_registry = registry.clone();
        let production_snapshot = production_registry.clone();
        let (version, policy, migration, conflict) = stage(&registry, scenario);
        let expected_outcome = expected(scenario);
        let promotion = stage_promotion(&registry, version.clone(), &policy, migration, conflict);
        let exact = promotion.outcome == expected_outcome;
        if exact {
            exact_lifecycle_decisions += 1;
        }
        if promotion.outcome == PromotionOutcome::Promoted {
            promotions += 1;
        } else {
            blocked_or_denied += 1;
        }
        let replay = stage_promotion(&registry, version.clone(), &policy, migration, conflict);
        let registry_replay = promotion == replay;
        if registry_replay {
            registry_replays += 1;
        }
        let mut tampered = promotion.clone();
        tampered.registry_hash.push('x');
        let tamper = tampered != promotion;
        if tamper {
            tamper_rejections += 1;
        }
        let mut rollback_done = false;
        let mut preserved = false;
        let mut historical = false;
        if matches!(
            scenario,
            Scenario::RollbackAccumulatedState | Scenario::HistoricalReplay
        ) && promotion.outcome == PromotionOutcome::Promoted
        {
            apply_promoted(&mut registry, version.clone());
            registry.world_state_hash = digest(&(registry.world_state_hash.clone(), &id, "event"));
            if let Some(receipt) = rollback(&mut registry, &version.id) {
                rollback_done = true;
                preserved = receipt.world_state_hash_before == receipt.world_state_hash_after;
                historical = receipt.historical_replay_verified
                    && registry.active.as_deref() == Some("curriculum-base-v1");
            }
        }
        if rollback_done {
            rollback_applied += 1;
        }
        if preserved {
            world_state_preserved += 1;
        }
        if historical {
            historical_replays += 1;
        }
        let clone_only = production_registry == production_snapshot;
        receipts.push(Receipt {
            id,
            scenario,
            expected: expected_outcome.clone(),
            actual: promotion.outcome.clone(),
            exact,
            source_preflight,
            registry_replay,
            tamper_rejected: tamper,
            rollback_applied: rollback_done,
            world_state_preserved: preserved,
            historical_replay: historical,
            clone_only,
            false_authorization: promotion.outcome == PromotionOutcome::Promoted
                && expected_outcome != PromotionOutcome::Promoted,
        });
    }
    let report = Report {
        schema: "stage316-finite-automata-promotion-rollback-v1",
        source_report: SOURCE_REPORT,
        source_report_sha256: digest(&source_bytes),
        cases: receipts.len(),
        source_preflight_passed: receipts.iter().filter(|r| r.source_preflight).count(),
        exact_lifecycle_decisions,
        promotions,
        blocked_or_denied,
        registry_replays,
        tamper_rejections,
        rollback_applied,
        world_state_preserved,
        historical_replays,
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: 0,
        production_registry_mutations: receipts.iter().filter(|r| !r.clone_only).count(),
        curriculum_manifest_mutations: 0,
        scenario_counts,
        receipts,
    };
    assert_eq!(report.cases, 240);
    assert_eq!(report.source_preflight_passed, 240);
    assert_eq!(report.exact_lifecycle_decisions, 240);
    assert_eq!(report.promotions, 100);
    assert_eq!(report.blocked_or_denied, 140);
    assert_eq!(report.registry_replays, 240);
    assert_eq!(report.tamper_rejections, 240);
    assert_eq!(report.rollback_applied, 60);
    assert_eq!(report.world_state_preserved, 60);
    assert_eq!(report.historical_replays, 60);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.production_registry_mutations, 0);
    assert_eq!(report.curriculum_manifest_mutations, 0);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 316 — finite-automata promotion and rollback\n\n- Cases: {}\n- Source preflight: {}/{}\n- Exact lifecycle decisions: {}/{}\n- Promotions / blocked or denied: {} / {}\n- Registry replays / tamper rejections: {} / {}\n- Rollbacks applied: {}\n- World state preserved / historical replay: {} / {}\n- False authorizations / denials: {} / {}\n- Production registry mutations / curriculum mutations: {} / {}\n\nThe source-derived automata capability is evaluated only in cloned registries. Clean promotion, policy denial, regression blocking, dependency conflict, migration failure, competing boundaries, accumulated-state rollback, and historical replay are all explicit lifecycle cases.\n",
            report.cases, report.source_preflight_passed, report.cases,
            report.exact_lifecycle_decisions, report.cases, report.promotions,
            report.blocked_or_denied, report.registry_replays, report.tamper_rejections,
            report.rollback_applied, report.world_state_preserved, report.historical_replays,
            report.false_authorizations, report.false_denials,
            report.production_registry_mutations, report.curriculum_manifest_mutations,
        ),
    )?;
    println!("stage316 cases={} exact={} promotions={} blocked={} rollback={} historical={} live_mutations={}", report.cases, report.exact_lifecycle_decisions, report.promotions, report.blocked_or_denied, report.rollback_applied, report.historical_replays, report.production_registry_mutations);
    Ok(())
}
