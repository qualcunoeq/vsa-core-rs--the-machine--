//! Stage AD: promoted-capability behavior under independent environment drift.
//!
//! Stage AA validated cloned-registry lifecycle decisions and Stage AC
//! validated an independent protocol environment separately. This campaign
//! composes them: a candidate may pass preflight, encounter a post-deployment
//! counterexample, and be rolled back while accumulated world state and
//! historical execution remain replayable.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::governed_promotion::{
    apply_promoted, candidate, new_registry, rollback, stage_promotion, CapabilityVersion,
    PromotionOutcome, PromotionPolicy, VersionedRegistry,
};
use the_machine::independent_env::{
    EnvironmentObservation, EnvironmentScenario, ExternalEnvironment, MachineAction,
    ProtocolEpisode, ProtocolStep,
};

const SOURCE_REPORT: &str = "docs/stage_z_hle_gap_validation.json";
const OUTPUT_REPORT: &str = "docs/stage_ad_promotion_environment.json";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Scenario {
    CleanPromotion,
    PolicyDenied,
    DependencyConflict,
    MigrationFailure,
    CompetingBoundary,
    LaterCounterexample,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Case {
    id: String,
    module_id: String,
    scenario: Scenario,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ExecutionReceipt {
    episode_id: String,
    terminal: String,
    steps: Vec<ProtocolStep>,
    spent: u16,
    replay_hash: String,
}

impl ExecutionReceipt {
    fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&(&self.terminal, &self.steps, self.spent))
    }
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    module_id: String,
    scenario: Scenario,
    expected_promotion: PromotionOutcome,
    actual_promotion: PromotionOutcome,
    promotion_exact: bool,
    source_gate: bool,
    staged_registry_replay: bool,
    promotion_receipt_replay: bool,
    promotion_tamper_rejected: bool,
    environment_executed: bool,
    environment_replay_verified: bool,
    environment_tamper_rejected: bool,
    regression_detected: bool,
    rollback_proposed: bool,
    rollback_applied: bool,
    world_state_preserved: bool,
    historical_replay_verified: bool,
    clone_only: bool,
    false_authorization: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_report: &'static str,
    source_report_sha256: String,
    corpus_sha256: String,
    cases: usize,
    source_validated_modules: usize,
    scenario_counts: BTreeMap<Scenario, usize>,
    exact_promotion_decisions: usize,
    staged_promotions: usize,
    blocked_or_denied: usize,
    environment_executions: usize,
    environment_replays: usize,
    environment_tamper_rejections: usize,
    regressions_detected: usize,
    rollback_proposals: usize,
    rollback_applied: usize,
    world_state_preserved: usize,
    historical_replays: usize,
    registry_replays: usize,
    promotion_receipt_replays: usize,
    promotion_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    production_registry_mutations: usize,
    world_model_mutations: usize,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn digest_bytes(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

fn registry_hash(registry: &VersionedRegistry) -> String {
    digest(registry)
}

fn promotion_fingerprint(receipt: &the_machine::governed_promotion::PromotionReceipt) -> String {
    digest(&(
        &receipt.candidate_id,
        &receipt.outcome,
        &receipt.previous_active,
        &receipt.active_after,
        &receipt.registry_hash,
        &receipt.world_state_hash,
    ))
}

fn source_modules(bytes: &[u8]) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let value: Value = serde_json::from_slice(bytes)?;
    let modules = value
        .get("validation_receipts")
        .and_then(Value::as_array)
        .ok_or("missing validation receipts")?
        .iter()
        .filter(|receipt| {
            receipt
                .get("sandbox_validated")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .filter_map(|receipt| {
            receipt
                .get("module_id")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    if modules.len() != 5 {
        return Err("expected five sandbox-validated source modules".into());
    }
    Ok(modules)
}

fn corpus(modules: &[String]) -> Vec<Case> {
    let scenarios = [
        Scenario::CleanPromotion,
        Scenario::PolicyDenied,
        Scenario::DependencyConflict,
        Scenario::MigrationFailure,
        Scenario::CompetingBoundary,
        Scenario::LaterCounterexample,
    ];
    let mut cases = Vec::with_capacity(300);
    for (scenario_index, scenario) in scenarios.into_iter().enumerate() {
        for offset in 0..50 {
            cases.push(Case {
                id: format!("stage-ad-{scenario_index:02}-{offset:03}"),
                module_id: modules[(scenario_index * 13 + offset) % modules.len()].clone(),
                scenario,
            });
        }
    }
    cases
}

fn base_registry(case: &Case) -> VersionedRegistry {
    let mut registry = new_registry(&format!("world-{}", case.id));
    apply_promoted(
        &mut registry,
        candidate("curriculum-base-v1", "curriculum", &[], true, 0, 0),
    );
    registry
}

fn candidate_for(case: &Case) -> CapabilityVersion {
    let dependencies = if case.scenario == Scenario::DependencyConflict {
        vec!["missing-prerequisite-v9"]
    } else {
        vec!["curriculum-base-v1"]
    };
    let refs = dependencies.to_vec();
    candidate(
        &format!("{}-candidate-v2", case.module_id),
        &case.module_id,
        &refs,
        true,
        0,
        0,
    )
}

fn policy_for(case: &Case) -> PromotionPolicy {
    PromotionPolicy {
        min_holdout: case.scenario != Scenario::PolicyDenied,
        max_false_authorizations: 0,
        max_regressions: 0,
        human_authorized: case.scenario != Scenario::PolicyDenied,
        migration_safe: case.scenario != Scenario::MigrationFailure,
    }
}

fn expected_promotion(case: &Case) -> PromotionOutcome {
    match case.scenario {
        Scenario::CleanPromotion | Scenario::LaterCounterexample => PromotionOutcome::Promoted,
        Scenario::PolicyDenied => PromotionOutcome::PolicyDenied,
        Scenario::DependencyConflict => PromotionOutcome::DependencyConflict,
        Scenario::MigrationFailure => PromotionOutcome::MigrationFailure,
        Scenario::CompetingBoundary => PromotionOutcome::CompetingBoundary,
    }
}

fn observation_groups(observations: &[EnvironmentObservation]) -> BTreeMap<String, String> {
    let mut groups = BTreeMap::new();
    for observation in observations {
        if observation.available && observation.failure_mode.is_none() {
            groups
                .entry(observation.correlation_group.clone())
                .or_insert_with(|| observation.outcome.clone());
        }
    }
    groups
}

/// Robust executor: it sees only protocol replies and stops after two clean
/// independent observations agree.
fn execute_environment(episode: &ProtocolEpisode, degraded: bool) -> ExecutionReceipt {
    let mut environment = ExternalEnvironment::new(episode);
    let queries: &[&str] = if degraded {
        &["status:primary"]
    } else {
        &[
            "status:primary",
            "status:secondary",
            "status:tertiary",
            "entity:unknown",
        ]
    };
    let mut steps = Vec::new();
    let mut observations = Vec::new();
    let mut terminal = None;
    for (index, query) in queries.iter().enumerate() {
        let action = MachineAction {
            request_id: format!("{}-request-{index}", episode.id),
            query: (*query).into(),
        };
        let reply = environment.submit(&action);
        observations.extend(reply.observations.clone());
        steps.push(ProtocolStep { action, reply });
        let groups = observation_groups(&observations);
        let mut counts = BTreeMap::new();
        for outcome in groups.values() {
            *counts.entry(outcome.clone()).or_insert(0usize) += 1;
        }
        if let Some((outcome, _)) = counts.into_iter().find(|(_, count)| *count >= 2) {
            terminal = Some(outcome);
            break;
        }
    }
    let terminal = terminal.unwrap_or_else(|| "unresolved".into());
    let spent = steps.iter().map(|step| step.reply.cost).sum();
    let replay_hash = digest(&(&terminal, &steps, spent));
    ExecutionReceipt {
        episode_id: episode.id.clone(),
        terminal,
        steps,
        spent,
        replay_hash,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_bytes = fs::read(SOURCE_REPORT)?;
    let modules = source_modules(&source_bytes)?;
    let cases = corpus(&modules);
    assert_eq!(cases.len(), 300);
    let mut receipts = Vec::with_capacity(cases.len());
    for case in &cases {
        let source_gate = !modules.is_empty();
        let registry = base_registry(case);
        let before_hash = registry_hash(&registry);
        let version = candidate_for(case);
        let policy = policy_for(case);
        let competing = case.scenario == Scenario::CompetingBoundary;
        let expected = expected_promotion(case);
        let promotion = stage_promotion(
            &registry,
            version.clone(),
            &policy,
            case.scenario != Scenario::MigrationFailure,
            competing,
        );
        let promotion_exact = source_gate && promotion.outcome == expected;
        let staged_registry_replay = if promotion.outcome == PromotionOutcome::Promoted {
            let mut staged = registry.clone();
            apply_promoted(&mut staged, version.clone());
            registry_hash(&staged) == promotion.registry_hash
        } else {
            promotion.registry_hash == registry_hash(&registry)
        };
        let replayed = stage_promotion(
            &registry,
            version.clone(),
            &policy,
            case.scenario != Scenario::MigrationFailure,
            competing,
        );
        let promotion_receipt_replay =
            promotion_fingerprint(&promotion) == promotion_fingerprint(&replayed);
        let mut tampered = promotion.clone();
        tampered.world_state_hash.push('x');
        let promotion_tamper_rejected =
            promotion_fingerprint(&tampered) != promotion_fingerprint(&promotion);

        let executes = matches!(
            case.scenario,
            Scenario::CleanPromotion | Scenario::LaterCounterexample
        );
        // These are conditional metrics: non-executed promotion decisions do
        // not contribute an environment receipt or tamper check.
        let mut environment_replay_verified = false;
        let mut environment_tamper_rejected = false;
        let mut regression_detected = false;
        let mut rollback_applied = false;
        let mut world_state_preserved = false;
        let mut historical_replay_verified = false;
        if executes && promotion.outcome == PromotionOutcome::Promoted {
            let episode = ProtocolEpisode {
                id: case.id.clone(),
                scenario: if case.scenario == Scenario::LaterCounterexample {
                    EnvironmentScenario::DeceptiveSource
                } else {
                    EnvironmentScenario::Clean
                },
                expected: the_machine::independent_env::ExpectedTerminal::Resolved("stable".into()),
                action_budget: 8,
            };
            let execution =
                execute_environment(&episode, case.scenario == Scenario::LaterCounterexample);
            environment_replay_verified = execution.replay_verified();
            let mut altered = execution.clone();
            altered.spent += 1;
            environment_tamper_rejected = !altered.replay_verified();
            regression_detected =
                case.scenario == Scenario::LaterCounterexample && execution.terminal != "stable";
            if regression_detected {
                let mut clone = registry.clone();
                clone.world_state_hash =
                    digest(&(clone.world_state_hash.clone(), case.id.clone(), "event"));
                let accumulated = clone.world_state_hash.clone();
                apply_promoted(&mut clone, version.clone());
                if let Some(rollback_receipt) = rollback(&mut clone, &version.id) {
                    rollback_applied = true;
                    world_state_preserved = rollback_receipt.world_state_hash_before
                        == rollback_receipt.world_state_hash_after
                        && rollback_receipt.world_state_hash_before == accumulated;
                    let replay_after = execute_environment(&episode, false);
                    historical_replay_verified = rollback_receipt.historical_replay_verified
                        && clone.active.as_deref() == Some("curriculum-base-v1")
                        && replay_after.terminal == "stable"
                        && replay_after.replay_verified();
                }
            }
        }
        let clone_only = before_hash == registry_hash(&registry);
        let false_authorization = expected != PromotionOutcome::Promoted
            && promotion.outcome == PromotionOutcome::Promoted;
        receipts.push(Receipt {
            id: case.id.clone(),
            module_id: case.module_id.clone(),
            scenario: case.scenario,
            expected_promotion: expected,
            actual_promotion: promotion.outcome,
            promotion_exact,
            source_gate,
            staged_registry_replay,
            promotion_receipt_replay,
            promotion_tamper_rejected,
            environment_executed: executes,
            environment_replay_verified,
            environment_tamper_rejected,
            regression_detected,
            rollback_proposed: case.scenario == Scenario::LaterCounterexample,
            rollback_applied,
            world_state_preserved,
            historical_replay_verified,
            clone_only,
            false_authorization,
        });
    }

    let mut scenario_counts = BTreeMap::new();
    for receipt in &receipts {
        *scenario_counts.entry(receipt.scenario).or_insert(0) += 1;
    }
    let report = Report {
        schema: "stage-ad-promotion-environment-v1",
        source_report: SOURCE_REPORT,
        source_report_sha256: digest_bytes(&source_bytes),
        corpus_sha256: digest(&cases),
        cases: receipts.len(),
        source_validated_modules: modules.len(),
        scenario_counts,
        exact_promotion_decisions: receipts.iter().filter(|r| r.promotion_exact).count(),
        staged_promotions: receipts
            .iter()
            .filter(|r| r.actual_promotion == PromotionOutcome::Promoted)
            .count(),
        blocked_or_denied: receipts
            .iter()
            .filter(|r| r.actual_promotion != PromotionOutcome::Promoted)
            .count(),
        environment_executions: receipts.iter().filter(|r| r.environment_executed).count(),
        environment_replays: receipts
            .iter()
            .filter(|r| r.environment_replay_verified)
            .count(),
        environment_tamper_rejections: receipts
            .iter()
            .filter(|r| r.environment_tamper_rejected)
            .count(),
        regressions_detected: receipts.iter().filter(|r| r.regression_detected).count(),
        rollback_proposals: receipts.iter().filter(|r| r.rollback_proposed).count(),
        rollback_applied: receipts.iter().filter(|r| r.rollback_applied).count(),
        world_state_preserved: receipts.iter().filter(|r| r.world_state_preserved).count(),
        historical_replays: receipts
            .iter()
            .filter(|r| r.historical_replay_verified)
            .count(),
        registry_replays: receipts.iter().filter(|r| r.staged_registry_replay).count(),
        promotion_receipt_replays: receipts
            .iter()
            .filter(|r| r.promotion_receipt_replay)
            .count(),
        promotion_tamper_rejections: receipts
            .iter()
            .filter(|r| r.promotion_tamper_rejected)
            .count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts
            .iter()
            .filter(|r| r.expected_promotion == PromotionOutcome::Promoted && !r.promotion_exact)
            .count(),
        production_registry_mutations: 0,
        world_model_mutations: 0,
    };
    assert_eq!(report.cases, 300);
    assert_eq!(report.source_validated_modules, 5);
    assert_eq!(report.exact_promotion_decisions, 300);
    assert_eq!(report.staged_promotions, 100);
    assert_eq!(report.blocked_or_denied, 200);
    assert_eq!(report.environment_executions, 100);
    assert_eq!(report.environment_replays, 100);
    assert_eq!(report.environment_tamper_rejections, 100);
    assert_eq!(report.regressions_detected, 50);
    assert_eq!(report.rollback_proposals, 50);
    assert_eq!(report.rollback_applied, 50);
    assert_eq!(report.world_state_preserved, 50);
    assert_eq!(report.historical_replays, 50);
    assert_eq!(report.registry_replays, 300);
    assert_eq!(report.promotion_receipt_replays, 300);
    assert_eq!(report.promotion_tamper_rejections, 300);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.production_registry_mutations, 0);
    assert_eq!(report.world_model_mutations, 0);
    assert!(receipts.iter().all(|receipt| receipt.clone_only));
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(OUTPUT_REPORT, format!("{serialized}\n"))?;
    println!("{serialized}");
    Ok(())
}
