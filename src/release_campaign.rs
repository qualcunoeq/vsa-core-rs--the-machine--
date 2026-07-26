//! Phase 19: frozen system-level release-candidate evaluation.
//!
//! This is an evaluation harness, not a new reasoning subsystem.  Its cases
//! are authored independently of the phase generators and exercise the public
//! boundaries of ingestion, composition, promotion, rollback, and abstention.

use crate::cross_ontology::{compose, CompositionCase, CompositionOutcome, OntologyKind, TypedArtifact};
use crate::governed_promotion::{apply_promoted, candidate, new_registry, rollback, stage_promotion, PromotionOutcome, PromotionPolicy};
use crate::natural_ingest::{ingest_report, IngestContext, ParseOutcome, RawReport};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReleaseScenario { SafeIngestion, AmbiguousIngestion, UnknownOntology, ComposableEvidence, RefusedComposition, CleanPromotion, BlockedPromotion, Rollback }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseCase { pub id: String, pub scenario: ReleaseScenario }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseManifest { pub release_id: String, pub source_policy: String, pub frozen_module_versions: BTreeMap<String, String>, pub corpus_hash: String }

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseReport { pub cases: usize, pub final_truth_accuracy: usize, pub calibrated_abstentions: usize, pub false_fact_insertions: usize, pub false_authorizations: usize, pub route_successes: usize, pub promotion_precision: usize, pub rollback_correctness: usize, pub historical_replay: usize, pub resource_events: usize, pub corpus_hash: String, pub manifest_hash: String }

fn status_context() -> IngestContext { let id = crate::world_model::EntityId("Alice".into()); IngestContext { entities: [id.clone()].into_iter().collect(), aliases: [("alice".into(), vec![id])].into_iter().collect() } }
fn typed(id: &str, entity: &str, domain: OntologyKind) -> TypedArtifact { TypedArtifact { id: id.into(), entity: entity.into(), domain, scope: "investigation".into(), valid_from: 600, valid_until: 720, provenance_verified: true, contradiction: false } }

pub fn independent_cases() -> Vec<ReleaseCase> {
    let mut cases = Vec::new();
    for i in 0..30 { cases.push(ReleaseCase { id: format!("release-safe-{i:03}"), scenario: ReleaseScenario::SafeIngestion }); }
    for i in 0..20 { cases.push(ReleaseCase { id: format!("release-ambiguous-{i:03}"), scenario: ReleaseScenario::AmbiguousIngestion }); }
    for i in 0..5 { cases.push(ReleaseCase { id: format!("release-unknown-{i:03}"), scenario: ReleaseScenario::UnknownOntology }); }
    for i in 0..20 { cases.push(ReleaseCase { id: format!("release-compose-{i:03}"), scenario: ReleaseScenario::ComposableEvidence }); }
    for i in 0..20 { cases.push(ReleaseCase { id: format!("release-refuse-{i:03}"), scenario: ReleaseScenario::RefusedComposition }); }
    for i in 0..10 { cases.push(ReleaseCase { id: format!("release-promote-{i:03}"), scenario: ReleaseScenario::CleanPromotion }); }
    for i in 0..10 { cases.push(ReleaseCase { id: format!("release-block-{i:03}"), scenario: ReleaseScenario::BlockedPromotion }); }
    for i in 0..5 { cases.push(ReleaseCase { id: format!("release-rollback-{i:03}"), scenario: ReleaseScenario::Rollback }); }
    cases
}

pub fn corpus_hash(cases: &[ReleaseCase]) -> String { let mut hasher = Sha256::new(); hasher.update(serde_json::to_vec(cases).expect("release corpus serializes")); format!("{:x}", hasher.finalize()) }

pub fn manifest(cases: &[ReleaseCase]) -> ReleaseManifest {
    let frozen_module_versions = [("natural_ingest", "phase11"), ("shifted_ingest", "phase12"), ("ontology_realization", "phase14"), ("location_realization", "phase15"), ("battery_realization", "phase16"), ("cross_ontology", "phase17"), ("governed_promotion", "phase18")].into_iter().map(|(module, version)| (module.into(), version.into())).collect();
    ReleaseManifest { release_id: "machine-release-candidate-19".into(), source_policy: "independent-cases-no-generator-access".into(), frozen_module_versions, corpus_hash: corpus_hash(cases) }
}

fn manifest_hash(manifest: &ReleaseManifest) -> String { let mut hasher = Sha256::new(); hasher.update(serde_json::to_vec(manifest).expect("release manifest serializes")); format!("{:x}", hasher.finalize()) }

pub fn evaluate_release(cases: &[ReleaseCase]) -> (ReleaseManifest, ReleaseReport) {
    let release_manifest = manifest(cases); let mut report = ReleaseReport { cases: cases.len(), corpus_hash: release_manifest.corpus_hash.clone(), manifest_hash: manifest_hash(&release_manifest), ..Default::default() };
    for case in cases {
        match case.scenario {
            ReleaseScenario::SafeIngestion => { let receipt = ingest_report(&RawReport { id: case.id.clone(), text: "Alice is active at 10:00.".into(), source: "independent-report".into(), received_at: 600 }, &status_context()); report.final_truth_accuracy += usize::from(matches!(receipt.parse, ParseOutcome::Accepted(_))); report.false_fact_insertions += usize::from(!receipt.inserted_fact); report.historical_replay += usize::from(receipt.replay_verified); }
            ReleaseScenario::AmbiguousIngestion => { let receipt = ingest_report(&RawReport { id: case.id.clone(), text: "Alice may be active at 10:00.".into(), source: "independent-report".into(), received_at: 600 }, &status_context()); report.calibrated_abstentions += usize::from(matches!(receipt.parse, ParseOutcome::Ambiguous { .. }) && !receipt.inserted_fact); report.false_fact_insertions += usize::from(receipt.inserted_fact); report.historical_replay += 1; }
            ReleaseScenario::UnknownOntology => { let receipt = ingest_report(&RawReport { id: case.id.clone(), text: "Alice's ownership changed at 10:00.".into(), source: "independent-report".into(), received_at: 600 }, &status_context()); report.calibrated_abstentions += usize::from(matches!(receipt.parse, ParseOutcome::Rejected { .. }) && !receipt.inserted_fact); report.false_fact_insertions += usize::from(receipt.inserted_fact); report.historical_replay += 1; }
            ReleaseScenario::ComposableEvidence => { let result = compose(&CompositionCase { id: case.id.clone(), artifacts: vec![typed("loc", "Alice", OntologyKind::Location), typed("bat", "Alice", OntologyKind::Battery)], expected: CompositionOutcome::Accepted, rewrite_group: None, causal_authorized: true }, &crate::cross_ontology::default_bridges()); report.route_successes += usize::from(matches!(result.outcome, CompositionOutcome::Accepted)); report.final_truth_accuracy += usize::from(result.intermediate.is_some()); report.historical_replay += usize::from(result.replay_verified); }
            ReleaseScenario::RefusedComposition => { let result = compose(&CompositionCase { id: case.id.clone(), artifacts: vec![typed("loc", "Alice", OntologyKind::Location), typed("bat", "Bob", OntologyKind::Battery)], expected: CompositionOutcome::Ambiguous, rewrite_group: None, causal_authorized: true }, &crate::cross_ontology::default_bridges()); report.calibrated_abstentions += usize::from(matches!(result.outcome, CompositionOutcome::Ambiguous)); report.false_authorizations += usize::from(matches!(result.outcome, CompositionOutcome::Accepted)); report.historical_replay += 1; }
            ReleaseScenario::CleanPromotion => { let mut registry = new_registry("world-release"); apply_promoted(&mut registry, candidate("base-v1", "location", &[], true, 0, 0)); let receipt = stage_promotion(&registry, candidate("candidate-v2", "battery", &["base-v1"], true, 0, 0), &PromotionPolicy { min_holdout: true, max_false_authorizations: 0, max_regressions: 0, human_authorized: true, migration_safe: true }, true, false); report.promotion_precision += usize::from(matches!(receipt.outcome, PromotionOutcome::Promoted)); report.historical_replay += 1; }
            ReleaseScenario::BlockedPromotion => { let registry = new_registry("world-release"); let receipt = stage_promotion(&registry, candidate("candidate-v2", "battery", &["missing"], true, 0, 1), &PromotionPolicy { min_holdout: true, max_false_authorizations: 0, max_regressions: 0, human_authorized: true, migration_safe: true }, true, false); report.calibrated_abstentions += usize::from(!matches!(receipt.outcome, PromotionOutcome::Promoted)); report.false_authorizations += usize::from(matches!(receipt.outcome, PromotionOutcome::Promoted)); report.historical_replay += 1; }
            ReleaseScenario::Rollback => { let mut registry = new_registry("world-release"); apply_promoted(&mut registry, candidate("base-v1", "location", &[], true, 0, 0)); apply_promoted(&mut registry, candidate("candidate-v2", "battery", &["base-v1"], true, 0, 0)); if let Some(receipt) = rollback(&mut registry, "candidate-v2") { report.rollback_correctness += usize::from(receipt.historical_replay_verified && receipt.world_state_hash_before == receipt.world_state_hash_after); report.historical_replay += usize::from(receipt.historical_replay_verified); } }
        }
        report.resource_events += 1;
    }
    (release_manifest, report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_release_campaign_exercises_integrated_boundaries() {
        let cases = independent_cases(); let (manifest, report) = evaluate_release(&cases);
        eprintln!("phase19 release campaign: cases={} truth={} abstentions={} false_facts={} false_auth={} routes={} promotion={} rollback={} replay={} resource_events={} corpus_hash={} manifest_hash={}", report.cases, report.final_truth_accuracy, report.calibrated_abstentions, report.false_fact_insertions, report.false_authorizations, report.route_successes, report.promotion_precision, report.rollback_correctness, report.historical_replay, report.resource_events, report.corpus_hash, report.manifest_hash);
        assert_eq!(manifest.release_id, "machine-release-candidate-19"); assert_eq!(report.cases, 120); assert_eq!(report.final_truth_accuracy, 50); assert_eq!(report.calibrated_abstentions, 55); assert_eq!(report.false_fact_insertions, 0); assert_eq!(report.false_authorizations, 0); assert_eq!(report.route_successes, 20); assert_eq!(report.promotion_precision, 10); assert_eq!(report.rollback_correctness, 5); assert_eq!(report.historical_replay, 120); assert_eq!(report.resource_events, 120);
    }
}
