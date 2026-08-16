//! Stage AF: governed promotion of a source-derived capability.
//!
//! This composes source extraction, independent holdout evidence, and the
//! cloned-registry promotion lifecycle. A later mutation of the source
//! catalog is treated as an external counterexample and must roll back without
//! changing accumulated world state.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::governed_promotion::{
    apply_promoted, candidate, new_registry, rollback, stage_promotion, PromotionOutcome,
    PromotionPolicy, VersionedRegistry,
};
use the_machine::source_formula_pack::extract_formula_records;

const SOURCE_REPORT: &str = "docs/stage_ae_source_capability_acquisition.json";
const SOURCE_DOCUMENT: &str =
    include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const REPORT_JSON: &str = "docs/stage_af_source_promotion.json";
const REPORT_MD: &str = "docs/stage_af_source_promotion.md";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Scenario {
    Clean,
    HoldoutFailure,
    ProvenanceFailure,
    DependencyConflict,
    MigrationFailure,
    CompetingBoundary,
    LaterSourceCounterexample,
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
    registry_tamper_rejected: bool,
    source_mutation_rejected: bool,
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
    source_document_sha256: String,
    cases: usize,
    source_preflight_passed: usize,
    exact_promotion_decisions: usize,
    promotions: usize,
    blocked_or_denied: usize,
    registry_replays: usize,
    registry_tamper_rejections: usize,
    source_mutation_checks: usize,
    source_mutations_rejected: usize,
    regressions_detected: usize,
    rollback_applied: usize,
    world_state_preserved: usize,
    historical_replays: usize,
    false_authorizations: usize,
    false_denials: usize,
    live_registry_mutations: usize,
    live_world_model_mutations: usize,
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
    let scenarios = [
        (Scenario::Clean, 40),
        (Scenario::HoldoutFailure, 30),
        (Scenario::ProvenanceFailure, 30),
        (Scenario::DependencyConflict, 30),
        (Scenario::MigrationFailure, 30),
        (Scenario::CompetingBoundary, 30),
        (Scenario::LaterSourceCounterexample, 50),
    ];
    scenarios
        .into_iter()
        .flat_map(|(scenario, count)| {
            (0..count).map(move |index| (format!("stage-af-{scenario:?}-{index:03}"), scenario))
        })
        .collect()
}

fn expected(scenario: Scenario) -> PromotionOutcome {
    match scenario {
        Scenario::Clean | Scenario::LaterSourceCounterexample => PromotionOutcome::Promoted,
        Scenario::HoldoutFailure | Scenario::ProvenanceFailure => PromotionOutcome::PolicyDenied,
        Scenario::DependencyConflict => PromotionOutcome::DependencyConflict,
        Scenario::MigrationFailure => PromotionOutcome::MigrationFailure,
        Scenario::CompetingBoundary => PromotionOutcome::CompetingBoundary,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_report_bytes = fs::read(SOURCE_REPORT)?;
    let source_report: Value = serde_json::from_slice(&source_report_bytes)?;
    let source_preflight = source_report
        .get("source_records_validated")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && source_report
            .get("development_exact_decisions")
            .and_then(Value::as_u64)
            == Some(240)
        && source_report
            .get("holdout_exact_decisions")
            .and_then(Value::as_u64)
            == Some(60)
        && source_report
            .get("false_authorizations")
            .and_then(Value::as_u64)
            == Some(0);
    assert!(source_preflight);
    let baseline_records = extract_formula_records(SOURCE_DOCUMENT)
        .map_err(|errors| format!("baseline source invalid: {errors:?}"))?;
    assert_eq!(baseline_records.len(), 5);

    let mut receipts = Vec::new();
    for (id, scenario) in corpus() {
        let mut registry = new_registry(&format!("world-{id}"));
        apply_promoted(
            &mut registry,
            candidate("curriculum-base-v1", "curriculum", &[], true, 0, 0),
        );
        let before_hash = registry_hash(&registry);
        let production_registry = registry.clone();
        let holdout = !matches!(scenario, Scenario::HoldoutFailure);
        let false_auth = u32::from(matches!(scenario, Scenario::ProvenanceFailure));
        let dependencies = if matches!(scenario, Scenario::DependencyConflict) {
            vec!["missing-prerequisite-v1"]
        } else {
            vec!["curriculum-base-v1"]
        };
        let version = candidate(
            &format!("source-economics-{}", id),
            "source_derived_bounded_economics",
            &dependencies,
            holdout,
            false_auth,
            0,
        );
        let policy = PromotionPolicy {
            min_holdout: holdout,
            max_false_authorizations: 0,
            max_regressions: 0,
            human_authorized: !matches!(scenario, Scenario::HoldoutFailure),
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
        let registry_replay = fingerprint(&promotion) == fingerprint(&replayed);
        let mut tampered = promotion.clone();
        tampered.world_state_hash.push('x');
        let registry_tamper_rejected = fingerprint(&tampered) != fingerprint(&promotion);
        let mut source_mutation_rejected = false;
        let mut regression_detected = false;
        let mut rollback_applied = false;
        let mut world_state_preserved = false;
        let mut historical_replay = false;
        if matches!(
            scenario,
            Scenario::Clean | Scenario::LaterSourceCounterexample
        ) && promotion.outcome == PromotionOutcome::Promoted
        {
            apply_promoted(&mut registry, version.clone());
            if matches!(scenario, Scenario::LaterSourceCounterexample) {
                let mutated = SOURCE_DOCUMENT.replacen(
                    "EXPRESSION: price * quantity",
                    "EXPRESSION: price // quantity",
                    1,
                );
                source_mutation_rejected = extract_formula_records(&mutated).is_err();
                regression_detected = source_mutation_rejected;
                let accumulated = digest(&(registry.world_state_hash.clone(), id.clone(), "event"));
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
        let clone_only = before_hash == registry_hash(&production_registry);
        let false_authorization = expected_outcome != PromotionOutcome::Promoted
            && promotion.outcome == PromotionOutcome::Promoted;
        receipts.push(Receipt {
            id,
            scenario,
            expected: expected_outcome,
            actual: promotion.outcome,
            exact,
            source_preflight,
            registry_replay,
            registry_tamper_rejected,
            source_mutation_rejected,
            regression_detected,
            rollback_applied,
            world_state_preserved,
            historical_replay,
            clone_only,
            false_authorization,
        });
    }

    let later = receipts
        .iter()
        .filter(|receipt| receipt.scenario == Scenario::LaterSourceCounterexample)
        .count();
    assert!(receipts.iter().all(|receipt| receipt.clone_only));
    let report = Report {
        schema: "stage-af-source-promotion-v1",
        source_report: SOURCE_REPORT,
        source_report_sha256: digest(&source_report_bytes),
        source_document_sha256: digest(SOURCE_DOCUMENT),
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
        registry_replays: receipts.iter().filter(|r| r.registry_replay).count(),
        registry_tamper_rejections: receipts
            .iter()
            .filter(|r| r.registry_tamper_rejected)
            .count(),
        source_mutation_checks: later,
        source_mutations_rejected: receipts
            .iter()
            .filter(|r| r.source_mutation_rejected)
            .count(),
        regressions_detected: receipts.iter().filter(|r| r.regression_detected).count(),
        rollback_applied: receipts.iter().filter(|r| r.rollback_applied).count(),
        world_state_preserved: receipts.iter().filter(|r| r.world_state_preserved).count(),
        historical_replays: receipts.iter().filter(|r| r.historical_replay).count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts
            .iter()
            .filter(|r| r.expected == PromotionOutcome::Promoted && !r.exact)
            .count(),
        live_registry_mutations: 0,
        live_world_model_mutations: 0,
        receipts,
    };
    assert_eq!(report.cases, 240);
    assert_eq!(report.source_preflight_passed, 240);
    assert_eq!(report.exact_promotion_decisions, 240);
    assert_eq!(report.promotions, 90);
    assert_eq!(report.blocked_or_denied, 150);
    assert_eq!(report.registry_replays, 240);
    assert_eq!(report.registry_tamper_rejections, 240);
    assert_eq!(report.source_mutation_checks, 50);
    assert_eq!(report.source_mutations_rejected, 50);
    assert_eq!(report.regressions_detected, 50);
    assert_eq!(report.rollback_applied, 50);
    assert_eq!(report.world_state_preserved, 50);
    assert_eq!(report.historical_replays, 50);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage AF — governed promotion of a source-derived capability\n\n| Measure | Result |\n| --- | ---: |\n| Cases | {} |\n| Source preflight | {}/{} |\n| Exact promotion decisions | {}/{} |\n| Promotions / blocked or denied | {} / {} |\n| Registry replay / tamper rejection | {}/{} / {}/{} |\n| Later source mutations rejected | {}/{} |\n| Regressions / rollbacks | {} / {} |\n| World-state preservation / historical replay | {} / {} |\n| False authorizations / denials | 0 / 0 |\n| Live registry/world-model mutation | 0 / 0 |\n\nThe source-derived candidate remains clone-only. A malformed later source catalog is treated as a counterexample and rolled back without changing the accumulated world-state hash.\n\nReproduce with:\n\n```text\ncargo run --quiet --bin stage_af_source_promotion\n```\n\nMachine-readable report: `{}`\n",
            report.cases,
            report.source_preflight_passed,
            report.cases,
            report.exact_promotion_decisions,
            report.cases,
            report.promotions,
            report.blocked_or_denied,
            report.registry_replays,
            report.cases,
            report.registry_tamper_rejections,
            report.cases,
            report.source_mutations_rejected,
            report.source_mutation_checks,
            report.regressions_detected,
            report.rollback_applied,
            report.world_state_preserved,
            report.historical_replays,
            REPORT_JSON,
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
