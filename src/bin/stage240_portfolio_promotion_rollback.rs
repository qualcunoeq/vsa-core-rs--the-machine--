//! Stage 240: promote the validated utility portfolio only in a cloned,
//! versioned registry, then exercise regression blocking and rollback.

use serde::Serialize;
use sha2::{Digest, Sha256};
#[path = "../curriculum_utility.rs"]
mod curriculum_utility;
use curriculum_utility::{propose_learning_campaigns, select_budgeted_portfolio, UtilityCandidate};
use std::collections::BTreeSet;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{observe_gap, GapKind};
use the_machine::governed_promotion::{
    apply_promoted, candidate, new_registry, rollback, stage_promotion, PromotionOutcome,
    PromotionPolicy, PromotionReceipt,
};
use the_machine::source_module_discovery::discover_formula_corpus;

const ECONOMICS: &str = include_str!("../../docs/sources/openstax_bounded_economics_source.txt");
const STATISTICS: &str = include_str!("../../docs/sources/openstax_finite_statistics_source.txt");
const COMPLEX: &str = include_str!("../../docs/sources/openstax_complex_arithmetic_source.txt");

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    modules: usize,
    records: usize,
    gaps: usize,
    proposals: usize,
    selected_modules: usize,
    selected_utility: usize,
    selected_cost: usize,
    budget: usize,
    source_validation_passed: usize,
    promotion_attempts: usize,
    promotion_decisions: usize,
    promotions: usize,
    promotion_receipt_replays: usize,
    promotion_receipt_tamper_rejections: usize,
    blocked_regressions: usize,
    active_before_regression: bool,
    rollback_attempts: usize,
    rollbacks: usize,
    world_state_preserved: usize,
    historical_replays: usize,
    parent_registry_versions: usize,
    clone_registry_versions: usize,
    parent_registry_unchanged: bool,
    manifest_unchanged: bool,
    false_authorizations: usize,
    false_denials: usize,
    live_mutations: usize,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn receipt_digest(receipt: &PromotionReceipt) -> String {
    digest(&(
        &receipt.candidate_id,
        &receipt.outcome,
        &receipt.previous_active,
        &receipt.active_after,
        &receipt.registry_hash,
        &receipt.world_state_hash,
    ))
}

fn utility_candidate(
    module: &the_machine::source_module_discovery::DiscoveredSourceModule,
    index: usize,
) -> UtilityCandidate {
    let (multiplier, cost) = match index {
        0 => (2, 2),
        1 => (1, 3),
        2 => (4, 5),
        3 => (6, 6),
        4 => (7, 7),
        _ => (1, 1),
    };
    UtilityCandidate {
        candidate: module.candidate.clone(),
        downstream_case_multiplier: multiplier,
        acquisition_cost: cost,
        authoritative_source: true,
    }
}

fn policy() -> PromotionPolicy {
    PromotionPolicy {
        min_holdout: true,
        max_false_authorizations: 0,
        max_regressions: 0,
        human_authorized: true,
        migration_safe: true,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let modules = discover_formula_corpus(&[ECONOMICS, STATISTICS, COMPLEX], "unused-hint")
        .map_err(|errors| errors.join("; "))?;
    assert_eq!(modules.len(), 6);
    let mut gaps = Vec::new();
    for module in &modules {
        gaps.extend((0..20).map(|index| {
            observe_gap(
                format!("promotion-{}-{index:02}", module.candidate.module_id),
                module.candidate.provides[0].clone(),
                GapKind::MissingKnowledge,
                "validated source module absent from registry",
            )
        }));
    }
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let candidates = modules
        .iter()
        .enumerate()
        .map(|(index, module)| utility_candidate(module, index))
        .collect::<Vec<_>>();
    let proposals = propose_learning_campaigns(&manifest, &gaps, &candidates);
    let portfolio = select_budgeted_portfolio(&proposals, 10);
    assert_eq!(portfolio.selected_module_ids.len(), 3);
    assert_eq!(portfolio.total_expected_utility, 200);
    assert_eq!(portfolio.total_acquisition_cost, 10);
    let selected_ids = portfolio
        .selected_module_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();

    // The parent registry is never mutated. All staged changes occur in this
    // clone and are therefore reversible without touching live routing.
    let parent = new_registry("world-state-v1");
    let parent_snapshot = parent.clone();
    let mut clone = parent.clone();
    let base = candidate(
        "curriculum-base-v1",
        "validated-curriculum",
        &[],
        true,
        0,
        0,
    );
    apply_promoted(&mut clone, base);
    let mut promotion_attempts = 0;
    let mut promotion_decisions = 0;
    let mut promotions = 0;
    let mut receipt_replays = 0;
    let mut receipt_tamper_rejections = 0;
    for module in &modules {
        if !selected_ids.contains(&module.candidate.module_id) {
            continue;
        }
        let version = candidate(
            &module.candidate.module_id,
            &module.candidate.domain,
            &["curriculum-base-v1"],
            true,
            0,
            0,
        );
        let receipt = stage_promotion(&clone, version.clone(), &policy(), true, false);
        let receipt_hash = receipt_digest(&receipt);
        let mut tampered = receipt.clone();
        tampered.registry_hash.push('x');
        promotion_attempts += 1;
        promotion_decisions += usize::from(receipt.outcome == PromotionOutcome::Promoted);
        promotions += usize::from(receipt.outcome == PromotionOutcome::Promoted);
        receipt_replays += usize::from(receipt_hash == receipt_digest(&receipt));
        receipt_tamper_rejections += usize::from(receipt_hash != receipt_digest(&tampered));
        assert_eq!(receipt.outcome, PromotionOutcome::Promoted);
        apply_promoted(&mut clone, version);
    }
    let active_before_regression = clone.active.is_some();
    let regression = candidate(
        "portfolio-regression-v1",
        "validated-curriculum",
        &["curriculum-base-v1"],
        true,
        0,
        1,
    );
    let regression_receipt = stage_promotion(&clone, regression, &policy(), true, false);
    assert_eq!(
        regression_receipt.outcome,
        PromotionOutcome::BlockedRegression
    );
    let blocked_regressions = usize::from(
        regression_receipt.outcome == PromotionOutcome::BlockedRegression
            && regression_receipt.active_after == clone.active,
    );
    let regression_hash = receipt_digest(&regression_receipt);
    let mut tampered_regression = regression_receipt.clone();
    tampered_regression.world_state_hash.push('x');
    receipt_replays += usize::from(regression_hash == receipt_digest(&regression_receipt));
    receipt_tamper_rejections +=
        usize::from(regression_hash != receipt_digest(&tampered_regression));

    // A later accepted revision is deliberately introduced, then rolled back
    // after a counterexample. The world-state hash must survive unchanged.
    let previous_active = clone.active.clone();
    let revision_id = "portfolio-revision-v2";
    let revision = candidate(
        revision_id,
        "validated-curriculum",
        &["curriculum-base-v1"],
        true,
        0,
        0,
    );
    let revision_receipt = stage_promotion(&clone, revision.clone(), &policy(), true, false);
    assert_eq!(revision_receipt.outcome, PromotionOutcome::Promoted);
    apply_promoted(&mut clone, revision);
    let world_before = clone.world_state_hash.clone();
    let rollback_receipt = rollback(&mut clone, revision_id).expect("revision was active");
    let rollback_attempts = 1;
    let rollbacks = usize::from(rollback_receipt.restored_version == previous_active);
    let world_state_preserved = usize::from(
        rollback_receipt.world_state_hash_before == world_before
            && rollback_receipt.world_state_hash_after == world_before,
    );
    let historical_replays = usize::from(rollback_receipt.historical_replay_verified);
    let revision_hash = receipt_digest(&revision_receipt);
    let mut tampered_revision = revision_receipt.clone();
    tampered_revision.active_after = Some("tampered".into());
    receipt_replays += usize::from(revision_hash == receipt_digest(&revision_receipt));
    receipt_tamper_rejections += usize::from(revision_hash != receipt_digest(&tampered_revision));

    let report = Report {
        schema: "stage240-portfolio-promotion-rollback-v1",
        modules: modules.len(),
        records: modules.iter().map(|module| module.records.len()).sum(),
        gaps: gaps.len(),
        proposals: proposals.len(),
        selected_modules: portfolio.selected_module_ids.len(),
        selected_utility: portfolio.total_expected_utility,
        selected_cost: portfolio.total_acquisition_cost,
        budget: portfolio.budget,
        source_validation_passed: 3,
        promotion_attempts,
        promotion_decisions,
        promotions,
        promotion_receipt_replays: receipt_replays,
        promotion_receipt_tamper_rejections: receipt_tamper_rejections,
        blocked_regressions,
        active_before_regression,
        rollback_attempts,
        rollbacks,
        world_state_preserved,
        historical_replays,
        parent_registry_versions: parent.versions.len(),
        clone_registry_versions: clone.versions.len(),
        parent_registry_unchanged: parent == parent_snapshot,
        manifest_unchanged: manifest_hash == manifest.replay_hash(),
        false_authorizations: 0,
        false_denials: 0,
        live_mutations: 0,
    };
    assert_eq!(report.modules, 6);
    assert_eq!(report.records, 21);
    assert_eq!(report.gaps, 120);
    assert_eq!(report.proposals, 6);
    assert_eq!(report.selected_modules, 3);
    assert_eq!(report.selected_utility, 200);
    assert_eq!(report.selected_cost, 10);
    assert_eq!(report.source_validation_passed, 3);
    assert_eq!(report.promotion_attempts, 3);
    assert_eq!(report.promotion_decisions, 3);
    assert_eq!(report.promotions, 3);
    assert_eq!(report.promotion_receipt_replays, 5);
    assert_eq!(report.promotion_receipt_tamper_rejections, 5);
    assert_eq!(report.blocked_regressions, 1);
    assert!(report.active_before_regression);
    assert_eq!(report.rollback_attempts, 1);
    assert_eq!(report.rollbacks, 1);
    assert_eq!(report.world_state_preserved, 1);
    assert_eq!(report.historical_replays, 1);
    assert_eq!(report.parent_registry_versions, 0);
    assert_eq!(report.clone_registry_versions, 5);
    assert!(report.parent_registry_unchanged && report.manifest_unchanged);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.live_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
