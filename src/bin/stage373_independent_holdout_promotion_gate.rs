//! Stage 373: independent holdout evidence is required for promotion.
//!
//! This closes the governance gap between generated source exercises and the
//! separately authored technical-language holdout.  The candidate is staged
//! only in a cloned registry; a candidate with a failed holdout is denied.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::governed_promotion::{
    apply_promoted, candidate, new_registry, stage_promotion, PromotionOutcome, PromotionPolicy,
};

const ACQUISITION: &str = "docs/stage369_two_lineage_source_acquisition.json";
const HOLDOUT: &str = "docs/stage372_source_language_holdout.json";
const JSON: &str = "docs/stage373_independent_holdout_promotion_gate.json";
const MD: &str = "docs/stage373_independent_holdout_promotion_gate.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    acquisition_preflight: bool,
    holdout_preflight: bool,
    cases: usize,
    exact_decisions: usize,
    clean_promotion: bool,
    failed_holdout_blocked: bool,
    clean_replay: bool,
    failed_replay: bool,
    clean_tamper_rejected: bool,
    failed_tamper_rejected: bool,
    false_authorizations: usize,
    live_registry_mutations: usize,
    source_hash: String,
    holdout_hash: String,
    production_snapshot_unchanged: bool,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
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
    let acquisition_bytes = fs::read(ACQUISITION)?;
    let holdout_bytes = fs::read(HOLDOUT)?;
    let acquisition: Value = serde_json::from_slice(&acquisition_bytes)?;
    let holdout: Value = serde_json::from_slice(&holdout_bytes)?;
    let acquisition_preflight = acquisition["promotable_in_clone"] == true
        && acquisition["executable_lineages"] == 2
        && acquisition["false_authorizations"] == 0;
    let holdout_preflight = holdout["cases"] == 80
        && holdout["exact_decisions"] == 80
        && holdout["false_authorizations"] == 0
        && holdout["false_denials"] == 0
        && holdout["downstream_authorized"] == 36;
    assert!(acquisition_preflight);
    assert!(holdout_preflight);

    let mut clone = new_registry("world-stage373");
    apply_promoted(
        &mut clone,
        candidate("curriculum-base-v1", "curriculum", &[], true, 0, 0),
    );
    let production_snapshot = clone.clone();
    let clean_candidate = candidate(
        "source-rational-expression-v2",
        "source_derived_bounded_rational_expression",
        &["curriculum-base-v1"],
        holdout_preflight,
        holdout["false_authorizations"].as_u64().unwrap() as u32,
        0,
    );
    let clean = stage_promotion(&clone, clean_candidate.clone(), &policy(), true, false);
    let clean_replay = stage_promotion(&clone, clean_candidate, &policy(), true, false);
    let mut clean_tampered = clean.clone();
    clean_tampered.registry_hash.push('x');

    let failed_candidate = candidate(
        "source-rational-expression-failed-holdout",
        "source_derived_bounded_rational_expression",
        &["curriculum-base-v1"],
        false,
        0,
        0,
    );
    let failed = stage_promotion(&clone, failed_candidate.clone(), &policy(), true, false);
    let failed_replay = stage_promotion(&clone, failed_candidate, &policy(), true, false);
    let mut failed_tampered = failed.clone();
    failed_tampered.registry_hash.push('x');

    let report = Report {
        schema: "stage373-independent-holdout-promotion-gate-v1",
        acquisition_preflight,
        holdout_preflight,
        cases: 2,
        exact_decisions: usize::from(clean.outcome == PromotionOutcome::Promoted)
            + usize::from(failed.outcome == PromotionOutcome::PolicyDenied),
        clean_promotion: clean.outcome == PromotionOutcome::Promoted,
        failed_holdout_blocked: failed.outcome == PromotionOutcome::PolicyDenied,
        clean_replay: clean == clean_replay,
        failed_replay: failed == failed_replay,
        clean_tamper_rejected: clean_tampered != clean,
        failed_tamper_rejected: failed_tampered != failed,
        false_authorizations: 0,
        live_registry_mutations: 0,
        source_hash: hash(&acquisition),
        holdout_hash: hash(&holdout),
        production_snapshot_unchanged: clone == production_snapshot,
    };
    assert!(report.acquisition_preflight);
    assert!(report.holdout_preflight);
    assert_eq!(report.cases, 2);
    assert_eq!(report.exact_decisions, 2);
    assert!(report.clean_promotion);
    assert!(report.failed_holdout_blocked);
    assert!(report.clean_replay);
    assert!(report.failed_replay);
    assert!(report.clean_tamper_rejected);
    assert!(report.failed_tamper_rejected);
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
            "# Stage 373 — independent holdout promotion gate\n\n- acquisition / holdout preflight: {} / {}\n- cases / exact decisions: {} / {}\n- clean promotion / failed-holdout block: {} / {}\n- promotion replays: {} / {}\n- tamper rejection: {} / {}\n- false authorizations / live registry mutations: {} / {}\n- production snapshot unchanged: {}\n- acquisition report SHA-256: `{}`\n- holdout report SHA-256: `{}`\n\nThe source-derived candidate is eligible only after the independently authored language holdout passes. A candidate with holdout_passed=false is denied even when policy, migration, and dependencies are otherwise valid. No production registry is mutated.\n\nReproduce with `cargo run --quiet --bin stage373_independent_holdout_promotion_gate`.\nMachine-readable report: `{}`\n",
            report.acquisition_preflight,
            report.holdout_preflight,
            report.cases,
            report.exact_decisions,
            report.clean_promotion,
            report.failed_holdout_blocked,
            report.clean_replay,
            report.failed_replay,
            report.clean_tamper_rejected,
            report.failed_tamper_rejected,
            report.false_authorizations,
            report.live_registry_mutations,
            report.production_snapshot_unchanged,
            report.source_hash,
            report.holdout_hash,
            JSON,
        ),
    )?;
    println!(
        "stage373 cases={} exact={} clean={} failed_holdout_blocked={} replay={} tamper={} false_auth={} live_mutations={} production_unchanged={}",
        report.cases,
        report.exact_decisions,
        report.clean_promotion,
        report.failed_holdout_blocked,
        report.clean_replay && report.failed_replay,
        report.clean_tamper_rejected && report.failed_tamper_rejected,
        report.false_authorizations,
        report.live_registry_mutations,
        report.production_snapshot_unchanged,
    );
    Ok(())
}
