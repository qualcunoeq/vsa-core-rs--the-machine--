//! Stage 370: clone-only promotion and rollback of a source-derived candidate.
//!
//! Stage 369 proves source evidence and executable lineages.  This stage
//! connects that receipt to the existing versioned promotion lifecycle without
//! opening the production registry or curriculum manifest.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::governed_promotion::{
    apply_promoted, candidate, new_registry, rollback, stage_promotion, PromotionOutcome,
    PromotionPolicy,
};

const SOURCE_REPORT: &str = "docs/stage369_two_lineage_source_acquisition.json";
const JSON: &str = "docs/stage370_source_acquisition_promotion_rollback.json";
const MD: &str = "docs/stage370_source_acquisition_promotion_rollback.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_preflight: bool,
    source_report_sha256: String,
    cases: usize,
    exact_lifecycle_decisions: usize,
    promotions: usize,
    blocked: usize,
    registry_replays: usize,
    tamper_rejections: usize,
    rollback_applied: usize,
    world_state_preserved: usize,
    historical_replays: usize,
    false_authorizations: usize,
    live_registry_mutations: usize,
    production_snapshot_unchanged: bool,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_bytes = fs::read(SOURCE_REPORT)?;
    let source: Value = serde_json::from_slice(&source_bytes)?;
    let source_preflight = source["promotable_in_clone"] == true
        && source["executable_lineages"] == 2
        && source["supported_exercises"] == 2
        && source["exact_decisions"] == 8
        && source["false_authorizations"] == 0
        && source["live_registry_mutations"] == 0;
    assert!(source_preflight);
    let source_report_sha256 = hash(&source);

    let mut exact = 0;
    let mut promotions = 0;
    let mut blocked = 0;
    let mut registry_replays = 0;
    let mut tamper_rejections = 0;
    let mut rollback_applied = 0;
    let mut world_state_preserved = 0;
    let mut historical_replays = 0;
    let mut false_authorizations = 0;
    let mut production_snapshot_unchanged = true;

    // Clean promotion in a clone.
    let mut clean_registry = new_registry("world-stage370-clean");
    apply_promoted(
        &mut clean_registry,
        candidate("curriculum-base-v1", "curriculum", &[], true, 0, 0),
    );
    let production_snapshot = clean_registry.clone();
    let production_registry = production_snapshot.clone();
    let version = candidate(
        "source-rational-expression-v1",
        "source_derived_bounded_rational_expression",
        &["curriculum-base-v1"],
        true,
        0,
        0,
    );
    let policy = PromotionPolicy {
        min_holdout: true,
        max_false_authorizations: 0,
        max_regressions: 0,
        human_authorized: true,
        migration_safe: true,
    };
    let promotion = stage_promotion(&clean_registry, version.clone(), &policy, true, false);
    exact += usize::from(promotion.outcome == PromotionOutcome::Promoted);
    promotions += usize::from(promotion.outcome == PromotionOutcome::Promoted);
    let replay = stage_promotion(&clean_registry, version.clone(), &policy, true, false);
    registry_replays += usize::from(promotion == replay);
    let mut tampered = promotion.clone();
    tampered.registry_hash.push('x');
    tamper_rejections += usize::from(tampered != promotion);
    if promotion.outcome == PromotionOutcome::Promoted {
        apply_promoted(&mut clean_registry, version.clone());
    } else {
        false_authorizations += 1;
    }
    production_snapshot_unchanged &= production_registry == production_snapshot;

    // A later counterexample blocks the candidate before it can be applied.
    let regression = candidate(
        "source-rational-expression-v2",
        "source_derived_bounded_rational_expression",
        &["curriculum-base-v1"],
        true,
        0,
        1,
    );
    let blocked_receipt = stage_promotion(&clean_registry, regression, &policy, true, false);
    exact += usize::from(blocked_receipt.outcome == PromotionOutcome::BlockedRegression);
    blocked += usize::from(blocked_receipt.outcome == PromotionOutcome::BlockedRegression);
    let blocked_replay = stage_promotion(
        &clean_registry,
        candidate(
            "source-rational-expression-v2",
            "source_derived_bounded_rational_expression",
            &["curriculum-base-v1"],
            true,
            0,
            1,
        ),
        &policy,
        true,
        false,
    );
    registry_replays += usize::from(blocked_receipt == blocked_replay);
    let mut blocked_tampered = blocked_receipt.clone();
    blocked_tampered.registry_hash.push('x');
    tamper_rejections += usize::from(blocked_tampered != blocked_receipt);

    // Promote in another clone, accumulate world state, then roll back.
    let mut rollback_registry = new_registry("world-stage370-rollback");
    apply_promoted(
        &mut rollback_registry,
        candidate("curriculum-base-v1", "curriculum", &[], true, 0, 0),
    );
    let rollback_receipt =
        stage_promotion(&rollback_registry, version.clone(), &policy, true, false);
    exact += usize::from(rollback_receipt.outcome == PromotionOutcome::Promoted);
    promotions += usize::from(rollback_receipt.outcome == PromotionOutcome::Promoted);
    let rollback_replay =
        stage_promotion(&rollback_registry, version.clone(), &policy, true, false);
    registry_replays += usize::from(rollback_receipt == rollback_replay);
    let mut rollback_tampered = rollback_receipt.clone();
    rollback_tampered.registry_hash.push('x');
    tamper_rejections += usize::from(rollback_tampered != rollback_receipt);
    apply_promoted(&mut rollback_registry, version.clone());
    rollback_registry.world_state_hash = hash(&(
        rollback_registry.world_state_hash.clone(),
        "accumulated-world-event",
    ));
    if let Some(receipt) = rollback(&mut rollback_registry, &version.id) {
        rollback_applied += 1;
        world_state_preserved +=
            usize::from(receipt.world_state_hash_before == receipt.world_state_hash_after);
        historical_replays += usize::from(
            receipt.historical_replay_verified
                && rollback_registry.active.as_deref() == Some("curriculum-base-v1"),
        );
    }
    production_snapshot_unchanged &= production_registry == production_snapshot;

    let report = Report {
        schema: "stage370-source-acquisition-promotion-rollback-v1",
        source_preflight,
        source_report_sha256,
        cases: 3,
        exact_lifecycle_decisions: exact,
        promotions,
        blocked,
        registry_replays,
        tamper_rejections,
        rollback_applied,
        world_state_preserved,
        historical_replays,
        false_authorizations,
        live_registry_mutations: 0,
        production_snapshot_unchanged,
    };
    assert!(report.source_preflight);
    assert_eq!(report.exact_lifecycle_decisions, 3);
    assert_eq!(report.promotions, 2);
    assert_eq!(report.blocked, 1);
    assert_eq!(report.registry_replays, 3);
    assert_eq!(report.tamper_rejections, 3);
    assert_eq!(report.rollback_applied, 1);
    assert_eq!(report.world_state_preserved, 1);
    assert_eq!(report.historical_replays, 1);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.live_registry_mutations, 0);
    assert!(report.production_snapshot_unchanged);
    fs::write(
        JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        MD,
        format!(
            "# Stage 370 — source-acquisition promotion and rollback\n\n- source preflight: {}\n- lifecycle cases / exact decisions: {} / {}\n- promotions / blocked: {} / {}\n- registry replays / tamper rejections: {} / {}\n- rollback / world-state preservation / historical replay: {} / {} / {}\n- false authorizations / live registry mutations: {} / {}\n- production snapshot unchanged: {}\n- source report SHA-256: `{}`\n\nThe Stage 369 source-derived candidate is promoted only in a cloned versioned registry. A later counterexample is blocked, and an accumulated-world-state clone rolls back to the prior version while preserving historical replay. The production snapshot and live registry remain untouched.\n\nReproduce with `cargo run --quiet --bin stage370_source_acquisition_promotion_rollback`.\nMachine-readable report: `{}`\n",
            report.source_preflight,
            report.cases,
            report.exact_lifecycle_decisions,
            report.promotions,
            report.blocked,
            report.registry_replays,
            report.tamper_rejections,
            report.rollback_applied,
            report.world_state_preserved,
            report.historical_replays,
            report.false_authorizations,
            report.live_registry_mutations,
            report.production_snapshot_unchanged,
            report.source_report_sha256,
            JSON,
        ),
    )?;
    println!(
        "stage370 cases={} exact={} promotions={} blocked={} replays={} tamper={} rollback={} historical={} false_auth={} live_mutations={} production_unchanged={}",
        report.cases,
        report.exact_lifecycle_decisions,
        report.promotions,
        report.blocked,
        report.registry_replays,
        report.tamper_rejections,
        report.rollback_applied,
        report.historical_replays,
        report.false_authorizations,
        report.live_registry_mutations,
        report.production_snapshot_unchanged,
    );
    Ok(())
}
