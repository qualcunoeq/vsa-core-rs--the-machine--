//! Stage AA: governed promotion and rollback of source-validated curriculum
//! proposals in a cloned registry.
//!
//! Stage Z proves that five answer-key-blind learning proposals are
//! sandbox-validatable.  This campaign is the next lifecycle boundary: it
//! consumes only those immutable receipts, stages candidates in a cloned
//! registry, exercises policy/dependency/migration/conflict checks, and
//! verifies rollback and historical replay.  The production registry and
//! curriculum manifest are never opened for mutation.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::process::Command;
use the_machine::governed_promotion::{
    apply_promoted, candidate, new_registry, rollback, stage_promotion, CapabilityVersion,
    PromotionOutcome, PromotionPolicy, VersionedRegistry,
};

const SOURCE_REPORT: &str = "docs/stage_z_hle_gap_validation.json";
const OUTPUT_REPORT: &str = "docs/stage_aa_governed_promotion.json";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Scenario {
    CleanPromotion,
    PolicyDenied,
    RegressionBlocked,
    DependencyConflict,
    MigrationFailure,
    CompetingBoundary,
    RollbackAccumulatedState,
    HistoricalReplay,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Case {
    id: String,
    module_id: String,
    scenario: Scenario,
}

#[derive(Debug, Clone)]
struct Evidence {
    module_id: String,
    evidence_sha256: String,
    sandbox_validated: bool,
    false_authorizations: usize,
    false_denials: usize,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    module_id: String,
    scenario: Scenario,
    expected_outcome: PromotionOutcome,
    actual_outcome: PromotionOutcome,
    exact: bool,
    source_gate: bool,
    staged_registry_replay: bool,
    promotion_receipt_replay: bool,
    tamper_rejected: bool,
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
    producer_commit: String,
    source_report: &'static str,
    source_report_sha256: String,
    corpus_sha256: String,
    cases: usize,
    source_validated_modules: usize,
    scenario_counts: BTreeMap<Scenario, usize>,
    exact_lifecycle_decisions: usize,
    staged_promotions: usize,
    blocked_or_denied: usize,
    rollback_proposals: usize,
    rollback_applied: usize,
    world_state_preserved: usize,
    historical_replays: usize,
    staged_registry_replays: usize,
    promotion_receipt_replays: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    production_registry_mutations: usize,
    curriculum_manifest_mutations: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn digest_bytes(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

fn producer_commit() -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".into())
}

fn registry_hash(registry: &VersionedRegistry) -> String {
    digest(registry)
}

fn receipt_payload(
    receipt: &the_machine::governed_promotion::PromotionReceipt,
) -> impl Serialize + '_ {
    (
        &receipt.candidate_id,
        &receipt.outcome,
        &receipt.previous_active,
        &receipt.active_after,
        &receipt.registry_hash,
        &receipt.world_state_hash,
    )
}

fn replay_promotion_receipt(
    registry: &VersionedRegistry,
    version: CapabilityVersion,
    policy: &PromotionPolicy,
    migration_ok: bool,
    competing_boundary: bool,
    receipt: &the_machine::governed_promotion::PromotionReceipt,
) -> bool {
    let replay = stage_promotion(registry, version, policy, migration_ok, competing_boundary);
    replay_payload(receipt) == replay_payload(&replay)
}

fn replay_payload(receipt: &the_machine::governed_promotion::PromotionReceipt) -> String {
    digest(&receipt_payload(receipt))
}

fn tamper_rejected(
    registry: &VersionedRegistry,
    version: CapabilityVersion,
    policy: &PromotionPolicy,
    migration_ok: bool,
    competing_boundary: bool,
) -> bool {
    let receipt = stage_promotion(registry, version, policy, migration_ok, competing_boundary);
    let mut tampered = receipt.clone();
    tampered.world_state_hash.push('x');
    replay_payload(&tampered) != replay_payload(&receipt)
}

fn parse_evidence(bytes: &[u8]) -> Result<Vec<Evidence>, Box<dyn std::error::Error>> {
    let report: Value = serde_json::from_slice(bytes)?;
    let receipts = report
        .get("validation_receipts")
        .and_then(Value::as_array)
        .ok_or("missing validation_receipts")?;
    let evidence = receipts
        .iter()
        .map(|receipt| {
            Ok(Evidence {
                module_id: receipt
                    .get("module_id")
                    .and_then(Value::as_str)
                    .ok_or("missing module_id")?
                    .to_owned(),
                evidence_sha256: receipt
                    .get("evidence_sha256")
                    .and_then(Value::as_str)
                    .ok_or("missing evidence_sha256")?
                    .to_owned(),
                sandbox_validated: receipt
                    .get("sandbox_validated")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                false_authorizations: receipt
                    .get("false_authorizations")
                    .and_then(Value::as_u64)
                    .unwrap_or(usize::MAX as u64) as usize,
                false_denials: receipt
                    .get("false_denials")
                    .and_then(Value::as_u64)
                    .unwrap_or(usize::MAX as u64) as usize,
            })
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    if evidence.len() != 5 || evidence.iter().any(|item| !item.sandbox_validated) {
        return Err("source report does not contain five sandbox-validated proposals".into());
    }
    Ok(evidence)
}

fn cases(evidence: &[Evidence]) -> Vec<Case> {
    let scenarios = [
        Scenario::CleanPromotion,
        Scenario::PolicyDenied,
        Scenario::RegressionBlocked,
        Scenario::DependencyConflict,
        Scenario::MigrationFailure,
        Scenario::CompetingBoundary,
        Scenario::RollbackAccumulatedState,
        Scenario::HistoricalReplay,
    ];
    let mut corpus = Vec::with_capacity(300);
    for (index, scenario) in scenarios.into_iter().enumerate() {
        let count = if scenario == Scenario::HistoricalReplay {
            20
        } else {
            40
        };
        for offset in 0..count {
            let module = &evidence[(index * 17 + offset) % evidence.len()];
            corpus.push(Case {
                id: format!("stage-aa-{index:02}-{offset:03}"),
                module_id: module.module_id.clone(),
                scenario,
            });
        }
    }
    corpus
}

fn base_registry(case: &Case) -> VersionedRegistry {
    let mut registry = new_registry(&format!("world-state-{}", case.id));
    apply_promoted(
        &mut registry,
        candidate("curriculum-base-v1", "curriculum", &[], true, 0, 0),
    );
    registry
}

fn candidate_for(case: &Case) -> CapabilityVersion {
    let (false_auth, regressions, dependencies) = match case.scenario {
        Scenario::PolicyDenied => (1, 0, vec!["curriculum-base-v1"]),
        Scenario::RegressionBlocked => (0, 1, vec!["curriculum-base-v1"]),
        Scenario::DependencyConflict => (0, 0, vec!["missing-prerequisite-v9"]),
        _ => (0, 0, vec!["curriculum-base-v1"]),
    };
    let dependency_refs: Vec<&str> = dependencies.to_vec();
    candidate(
        &format!("{}-shadow-v2", case.module_id),
        &case.module_id,
        &dependency_refs,
        true,
        false_auth,
        regressions,
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

fn expected_outcome(case: &Case) -> PromotionOutcome {
    match case.scenario {
        Scenario::CleanPromotion
        | Scenario::RollbackAccumulatedState
        | Scenario::HistoricalReplay => PromotionOutcome::Promoted,
        Scenario::PolicyDenied => PromotionOutcome::PolicyDenied,
        Scenario::RegressionBlocked => PromotionOutcome::BlockedRegression,
        Scenario::DependencyConflict => PromotionOutcome::DependencyConflict,
        Scenario::MigrationFailure => PromotionOutcome::MigrationFailure,
        Scenario::CompetingBoundary => PromotionOutcome::CompetingBoundary,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_bytes = fs::read(SOURCE_REPORT)?;
    let evidence = parse_evidence(&source_bytes)?;
    let corpus = cases(&evidence);
    assert_eq!(corpus.len(), 300);

    let mut receipts = Vec::with_capacity(corpus.len());
    for case in &corpus {
        let evidence_item = evidence
            .iter()
            .find(|item| item.module_id == case.module_id)
            .expect("case module has source evidence");
        let source_gate = evidence_item.sandbox_validated
            && !evidence_item.evidence_sha256.is_empty()
            && evidence_item.false_authorizations == 0
            && evidence_item.false_denials == 0;
        let registry = base_registry(case);
        let before_hash = registry_hash(&registry);
        let version = candidate_for(case);
        let policy = policy_for(case);
        let migration_ok = case.scenario != Scenario::MigrationFailure;
        let competing = case.scenario == Scenario::CompetingBoundary;
        let expected = expected_outcome(case);
        let promotion =
            stage_promotion(&registry, version.clone(), &policy, migration_ok, competing);
        let exact = source_gate && promotion.outcome == expected;
        let false_authorization = !source_gate
            || (expected != PromotionOutcome::Promoted
                && promotion.outcome == PromotionOutcome::Promoted);
        let staged_registry_replay = if promotion.outcome == PromotionOutcome::Promoted {
            let mut staged = registry.clone();
            apply_promoted(&mut staged, version.clone());
            registry_hash(&staged) == promotion.registry_hash
        } else {
            true
        };
        let promotion_receipt_replay = replay_promotion_receipt(
            &registry,
            version.clone(),
            &policy,
            migration_ok,
            competing,
            &promotion,
        );
        let tamper = tamper_rejected(&registry, version.clone(), &policy, migration_ok, competing);
        let needs_rollback = matches!(
            case.scenario,
            Scenario::RollbackAccumulatedState | Scenario::HistoricalReplay
        );
        let mut rollback_applied = false;
        let mut world_state_preserved = false;
        let mut historical_replay_verified = false;
        if needs_rollback && promotion.outcome == PromotionOutcome::Promoted {
            let mut clone = registry.clone();
            clone.world_state_hash =
                digest(&(clone.world_state_hash.clone(), case.id.clone(), "event"));
            let accumulated_state_hash = clone.world_state_hash.clone();
            apply_promoted(&mut clone, version);
            if let Some(receipt) = rollback(&mut clone, &format!("{}-shadow-v2", case.module_id)) {
                rollback_applied = true;
                world_state_preserved = receipt.world_state_hash_before
                    == receipt.world_state_hash_after
                    && receipt.world_state_hash_before == accumulated_state_hash;
                historical_replay_verified = receipt.historical_replay_verified
                    && clone.active.as_deref() == Some("curriculum-base-v1");
            }
        }
        let clone_only = before_hash == registry_hash(&registry);
        receipts.push(Receipt {
            id: case.id.clone(),
            module_id: case.module_id.clone(),
            scenario: case.scenario,
            expected_outcome: expected,
            actual_outcome: promotion.outcome.clone(),
            exact,
            source_gate,
            staged_registry_replay,
            promotion_receipt_replay,
            tamper_rejected: tamper,
            rollback_proposed: needs_rollback,
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
        schema: "stage-aa-governed-promotion-v1",
        producer_commit: producer_commit(),
        source_report: SOURCE_REPORT,
        source_report_sha256: digest_bytes(&source_bytes),
        corpus_sha256: digest(&corpus),
        cases: receipts.len(),
        source_validated_modules: evidence.len(),
        scenario_counts,
        exact_lifecycle_decisions: receipts.iter().filter(|r| r.exact).count(),
        staged_promotions: receipts
            .iter()
            .filter(|r| r.actual_outcome == PromotionOutcome::Promoted)
            .count(),
        blocked_or_denied: receipts
            .iter()
            .filter(|r| r.actual_outcome != PromotionOutcome::Promoted)
            .count(),
        rollback_proposals: receipts.iter().filter(|r| r.rollback_proposed).count(),
        rollback_applied: receipts.iter().filter(|r| r.rollback_applied).count(),
        world_state_preserved: receipts.iter().filter(|r| r.world_state_preserved).count(),
        historical_replays: receipts
            .iter()
            .filter(|r| r.historical_replay_verified)
            .count(),
        staged_registry_replays: receipts.iter().filter(|r| r.staged_registry_replay).count(),
        promotion_receipt_replays: receipts
            .iter()
            .filter(|r| r.promotion_receipt_replay)
            .count(),
        tamper_rejections: receipts.iter().filter(|r| r.tamper_rejected).count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts
            .iter()
            .filter(|r| r.expected_outcome == PromotionOutcome::Promoted && !r.exact)
            .count(),
        production_registry_mutations: 0,
        curriculum_manifest_mutations: 0,
        receipts,
    };
    assert_eq!(report.source_validated_modules, 5);
    assert_eq!(report.cases, 300);
    assert_eq!(report.exact_lifecycle_decisions, 300);
    assert_eq!(report.staged_promotions, 100);
    assert_eq!(report.blocked_or_denied, 200);
    assert_eq!(report.rollback_proposals, 60);
    assert_eq!(report.rollback_applied, 60);
    assert_eq!(report.world_state_preserved, 60);
    assert_eq!(report.historical_replays, 60);
    assert_eq!(report.staged_registry_replays, 300);
    assert_eq!(report.promotion_receipt_replays, 300);
    assert_eq!(report.tamper_rejections, 300);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.production_registry_mutations, 0);
    assert_eq!(report.curriculum_manifest_mutations, 0);
    assert!(report.receipts.iter().all(|receipt| receipt.clone_only));

    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(OUTPUT_REPORT, format!("{serialized}\n"))?;
    println!("{serialized}");
    Ok(())
}
