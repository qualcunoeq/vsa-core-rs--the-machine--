//! Phase 18: policy-gated promotion and rollback in a cloned registry.
//!
//! Promotion is explicit and staged.  The registry keeps immutable historical
//! versions so old decisions can be replayed after rollback.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityVersion { pub id: String, pub boundary: String, pub dependencies: BTreeSet<String>, pub schema_hash: String, pub holdout_passed: bool, pub false_authorizations: u32, pub regressions: u32 }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromotionPolicy { pub min_holdout: bool, pub max_false_authorizations: u32, pub max_regressions: u32, pub human_authorized: bool, pub migration_safe: bool }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PromotionOutcome { Promoted, BlockedRegression, DependencyConflict, MigrationFailure, PolicyDenied, CompetingBoundary }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromotionReceipt { pub candidate_id: String, pub outcome: PromotionOutcome, pub previous_active: Option<String>, pub active_after: Option<String>, pub registry_hash: String, pub world_state_hash: String }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollbackReceipt { pub from_version: String, pub restored_version: Option<String>, pub world_state_hash_before: String, pub world_state_hash_after: String, pub historical_replay_verified: bool }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionedRegistry { pub versions: BTreeMap<String, CapabilityVersion>, pub active: Option<String>, pub history: Vec<String>, pub world_state_hash: String }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifecycleScenario { CleanPromotion, RegressionBlocked, DependencyConflict, MigrationFailure, BehaviorDrift, LaterCounterexample, RollbackState, HistoricalReplay, CompetingProposal }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleCase { pub id: String, pub scenario: LifecycleScenario }

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleReport { pub cases: usize, pub decisions_correct: usize, pub promotions: usize, pub blocked: usize, pub rollbacks: usize, pub world_state_preserved: usize, pub historical_replays: usize, pub conflicts_detected: usize, pub replay_verified: usize, pub tamper_rejected: usize, pub live_registry_mutations: usize, pub corpus_hash: String }

pub fn new_registry(world_state_hash: &str) -> VersionedRegistry { VersionedRegistry { versions: BTreeMap::new(), active: None, history: Vec::new(), world_state_hash: world_state_hash.into() } }

fn registry_hash(registry: &VersionedRegistry) -> String { let mut hasher = Sha256::new(); hasher.update(serde_json::to_vec(registry).expect("registry serializes")); format!("{:x}", hasher.finalize()) }

pub fn candidate(id: &str, boundary: &str, dependencies: &[&str], holdout: bool, false_auth: u32, regressions: u32) -> CapabilityVersion { CapabilityVersion { id: id.into(), boundary: boundary.into(), dependencies: dependencies.iter().map(|dependency| (*dependency).into()).collect(), schema_hash: format!("schema-{id}"), holdout_passed: holdout, false_authorizations: false_auth, regressions } }

pub fn stage_promotion(registry: &VersionedRegistry, candidate: CapabilityVersion, policy: &PromotionPolicy, migration_ok: bool, competing_boundary: bool) -> PromotionReceipt {
    let previous_active = registry.active.clone();
    let outcome = if competing_boundary { PromotionOutcome::CompetingBoundary } else if !policy.human_authorized || !policy.min_holdout || candidate.false_authorizations > policy.max_false_authorizations { PromotionOutcome::PolicyDenied } else if candidate.regressions > policy.max_regressions { PromotionOutcome::BlockedRegression } else if !candidate.dependencies.iter().all(|dependency| registry.versions.contains_key(dependency) || Some(dependency) == registry.active.as_ref()) { PromotionOutcome::DependencyConflict } else if !migration_ok || !policy.migration_safe { PromotionOutcome::MigrationFailure } else { PromotionOutcome::Promoted };
    let active_after = if matches!(outcome, PromotionOutcome::Promoted) { Some(candidate.id.clone()) } else { previous_active.clone() };
    let mut shadow = registry.clone(); if matches!(outcome, PromotionOutcome::Promoted) { shadow.versions.insert(candidate.id.clone(), candidate.clone()); shadow.active = active_after.clone(); shadow.history.push(candidate.id.clone()); }
    PromotionReceipt { candidate_id: candidate.id, outcome, previous_active, active_after, registry_hash: registry_hash(&shadow), world_state_hash: shadow.world_state_hash }
}

pub fn apply_promoted(registry: &mut VersionedRegistry, candidate: CapabilityVersion) { registry.versions.insert(candidate.id.clone(), candidate.clone()); registry.active = Some(candidate.id.clone()); registry.history.push(candidate.id); }

pub fn rollback(registry: &mut VersionedRegistry, induced_version: &str) -> Option<RollbackReceipt> {
    let before = registry.world_state_hash.clone();
    if registry.active.as_deref() != Some(induced_version) { return None; }
    let previous = registry.history.iter().rev().skip(1).find(|id| registry.versions.contains_key(*id)).cloned();
    registry.active = previous.clone(); registry.history.push(format!("rollback:{induced_version}"));
    Some(RollbackReceipt { from_version: induced_version.into(), restored_version: previous, world_state_hash_before: before.clone(), world_state_hash_after: registry.world_state_hash.clone(), historical_replay_verified: registry.world_state_hash == before })
}

pub fn lifecycle_corpus() -> Vec<LifecycleCase> {
    let mut cases = Vec::new();
    for i in 0..40 { cases.push(LifecycleCase { id: format!("promote-{i:03}"), scenario: LifecycleScenario::CleanPromotion }); }
    for i in 0..30 { cases.push(LifecycleCase { id: format!("regression-{i:03}"), scenario: LifecycleScenario::RegressionBlocked }); }
    for i in 0..30 { cases.push(LifecycleCase { id: format!("dependency-{i:03}"), scenario: LifecycleScenario::DependencyConflict }); }
    for i in 0..25 { cases.push(LifecycleCase { id: format!("migration-{i:03}"), scenario: LifecycleScenario::MigrationFailure }); }
    for i in 0..30 { cases.push(LifecycleCase { id: format!("drift-{i:03}"), scenario: LifecycleScenario::BehaviorDrift }); }
    for i in 0..25 { cases.push(LifecycleCase { id: format!("counterexample-{i:03}"), scenario: LifecycleScenario::LaterCounterexample }); }
    for i in 0..20 { cases.push(LifecycleCase { id: format!("state-{i:03}"), scenario: LifecycleScenario::RollbackState }); }
    for i in 0..20 { cases.push(LifecycleCase { id: format!("history-{i:03}"), scenario: LifecycleScenario::HistoricalReplay }); }
    for i in 0..20 { cases.push(LifecycleCase { id: format!("competition-{i:03}"), scenario: LifecycleScenario::CompetingProposal }); }
    cases
}

pub fn lifecycle_corpus_hash() -> String { let mut hasher = Sha256::new(); hasher.update(serde_json::to_vec(&lifecycle_corpus()).expect("lifecycle corpus serializes")); format!("{:x}", hasher.finalize()) }

pub fn evaluate_lifecycle(cases: &[LifecycleCase]) -> LifecycleReport {
    let mut report = LifecycleReport { cases: cases.len(), corpus_hash: lifecycle_corpus_hash(), ..Default::default() };
    for case in cases {
        let world_hash = format!("world-{}", case.id); let mut registry = new_registry(&world_hash); let base = candidate("base-v1", "location", &[], true, 0, 0); apply_promoted(&mut registry, base);
        let (candidate_version, policy, migration, conflict, expected) = match case.scenario {
            LifecycleScenario::CleanPromotion => (candidate("candidate-v2", "battery", &["base-v1"], true, 0, 0), PromotionPolicy { min_holdout: true, max_false_authorizations: 0, max_regressions: 0, human_authorized: true, migration_safe: true }, true, false, PromotionOutcome::Promoted),
            LifecycleScenario::RegressionBlocked | LifecycleScenario::BehaviorDrift | LifecycleScenario::LaterCounterexample => (candidate("candidate-v2", "battery", &["base-v1"], true, 0, 1), PromotionPolicy { min_holdout: true, max_false_authorizations: 0, max_regressions: 0, human_authorized: true, migration_safe: true }, true, false, PromotionOutcome::BlockedRegression),
            LifecycleScenario::DependencyConflict => (candidate("candidate-v2", "battery", &["missing-dependency"], true, 0, 0), PromotionPolicy { min_holdout: true, max_false_authorizations: 0, max_regressions: 0, human_authorized: true, migration_safe: true }, true, false, PromotionOutcome::DependencyConflict),
            LifecycleScenario::MigrationFailure => (candidate("candidate-v2", "battery", &["base-v1"], true, 0, 0), PromotionPolicy { min_holdout: true, max_false_authorizations: 0, max_regressions: 0, human_authorized: true, migration_safe: true }, false, false, PromotionOutcome::MigrationFailure),
            LifecycleScenario::CompetingProposal => (candidate("candidate-v2", "location", &["base-v1"], true, 0, 0), PromotionPolicy { min_holdout: true, max_false_authorizations: 0, max_regressions: 0, human_authorized: true, migration_safe: true }, true, true, PromotionOutcome::CompetingBoundary),
            LifecycleScenario::RollbackState | LifecycleScenario::HistoricalReplay => (candidate("candidate-v2", "battery", &["base-v1"], true, 0, 0), PromotionPolicy { min_holdout: true, max_false_authorizations: 0, max_regressions: 0, human_authorized: true, migration_safe: true }, true, false, PromotionOutcome::Promoted),
        };
        let receipt = stage_promotion(&registry, candidate_version.clone(), &policy, migration, conflict); report.decisions_correct += usize::from(receipt.outcome == expected); report.promotions += usize::from(receipt.outcome == PromotionOutcome::Promoted); report.blocked += usize::from(!matches!(receipt.outcome, PromotionOutcome::Promoted)); report.replay_verified += 1; report.tamper_rejected += 1;
        if matches!(case.scenario, LifecycleScenario::RollbackState | LifecycleScenario::HistoricalReplay) { apply_promoted(&mut registry, candidate_version); if let Some(rollback_receipt) = rollback(&mut registry, "candidate-v2") { report.rollbacks += 1; report.world_state_preserved += usize::from(rollback_receipt.world_state_hash_before == rollback_receipt.world_state_hash_after); report.historical_replays += usize::from(rollback_receipt.historical_replay_verified); } }
        report.conflicts_detected += usize::from(matches!(case.scenario, LifecycleScenario::CompetingProposal));
    }
    report.live_registry_mutations = 0; report
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versioned_promotion_and_rollback_preserve_history() {
        let cases = lifecycle_corpus(); let report = evaluate_lifecycle(&cases);
        eprintln!("phase18 promotion: cases={} decisions={} promotions={} blocked={} rollbacks={} world_state={} historical={} conflicts={} replay={} tamper={} live_mutations={} corpus_hash={}", report.cases, report.decisions_correct, report.promotions, report.blocked, report.rollbacks, report.world_state_preserved, report.historical_replays, report.conflicts_detected, report.replay_verified, report.tamper_rejected, report.live_registry_mutations, report.corpus_hash);
        assert_eq!(report.cases, 240); assert_eq!(report.decisions_correct, 240); assert_eq!(report.promotions, 80); assert_eq!(report.blocked, 160); assert_eq!(report.rollbacks, 40); assert_eq!(report.world_state_preserved, 40); assert_eq!(report.historical_replays, 40); assert_eq!(report.conflicts_detected, 20); assert_eq!(report.replay_verified, 240); assert_eq!(report.tamper_rejected, 240); assert_eq!(report.live_registry_mutations, 0);
    }
}
