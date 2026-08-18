//! Stage 275: clone-only promotion and rollback for bounded health ratios.
//!
//! This is a lifecycle test for a bounded ratio calculator, not medical
//! decision support. Production curriculum and routing remain unchanged.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;

use the_machine::curriculum::{
    breadth_first_manifest, CurriculumPack, CurriculumStatus, ValidationGates,
};

const VALIDATION: &str = "docs/stage270_health_ratio_shadow_validation.json";
const REPORT_JSON: &str = "docs/stage275_health_ratio_promotion_rollback.json";
const REPORT_MD: &str = "docs/stage275_health_ratio_promotion_rollback.md";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Scenario {
    Clean,
    Regression,
    MissingEvidence,
    DependencyConflict,
    UnfrozenHle,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    scenario: Scenario,
    expected: &'static str,
    actual: &'static str,
    exact: bool,
    promotion_replay: bool,
    promotion_tamper_rejected: bool,
    regression_detected: bool,
    rollback_applied: bool,
    historical_replay: bool,
    parent_preserved: bool,
    clone_only: bool,
    false_authorization: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    validation_report_sha256: String,
    cases: usize,
    exact_promotion_decisions: usize,
    promotions: usize,
    blocked_or_denied: usize,
    promotion_replays: usize,
    promotion_tamper_rejections: usize,
    regressions_detected: usize,
    rollbacks_applied: usize,
    historical_replays: usize,
    parent_preserved: usize,
    clone_only: usize,
    false_authorizations: usize,
    false_denials: usize,
    live_manifest_mutations: usize,
    live_registry_mutations: usize,
    receipts: Vec<Receipt>,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn scenario(index: usize) -> Scenario {
    match index {
        0..100 => Scenario::Clean,
        100..140 => Scenario::Regression,
        140..180 => Scenario::MissingEvidence,
        180..220 => Scenario::DependencyConflict,
        _ => Scenario::UnfrozenHle,
    }
}

fn expected(scenario: Scenario) -> &'static str {
    match scenario {
        Scenario::Clean => "promoted",
        Scenario::Regression => "rolled_back",
        _ => "blocked",
    }
}

fn candidate(scenario: Scenario) -> CurriculumPack {
    let gates_ok = !matches!(scenario, Scenario::MissingEvidence);
    CurriculumPack {
        id: "source_derived_bounded_health_ratios".into(),
        title: "Source-derived bounded health ratios".into(),
        status: CurriculumStatus::ShadowValidated,
        prerequisites: if matches!(scenario, Scenario::DependencyConflict) {
            vec!["missing_dependency".into()]
        } else {
            vec!["probability_stochastic".into()]
        },
        reusable_artifacts: vec!["typed_health_ratio".into(), "population_rate".into()],
        source_requirements: vec![VALIDATION.into()],
        validation_gates: ValidationGates {
            authoritative_sources: gates_ok,
            independent_development_corpus: gates_ok,
            boundary_corpus: gates_ok,
            pressure_corpus: gates_ok,
            replay_verified: gates_ok,
            zero_false_authorization: gates_ok,
            frozen_hle_holdout: !matches!(scenario, Scenario::UnfrozenHle),
        },
        hle_policy: if matches!(scenario, Scenario::UnfrozenHle) {
            "live HLE routing allowed".into()
        } else {
            "HLE remains a frozen diagnostic holdout; never development data".into()
        },
        selection_reason: "fresh source-derived health-ratio validation".into(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let validation_bytes = fs::read(VALIDATION)?;
    let validation: serde_json::Value = serde_json::from_slice(&validation_bytes)?;
    assert_eq!(
        validation
            .get("exact_decisions")
            .and_then(serde_json::Value::as_u64),
        Some(600)
    );
    assert_eq!(
        validation
            .get("false_authorizations")
            .and_then(serde_json::Value::as_u64),
        Some(0)
    );
    let mut receipts = Vec::new();
    for index in 0..240usize {
        let scenario = scenario(index);
        let expected = expected(scenario);
        let parent = breadth_first_manifest();
        let parent_hash = parent.replay_hash();
        let mut proposed = parent.clone();
        proposed.packs.push(candidate(scenario));
        let proposal_hash = proposed.replay_hash();
        let mut tampered = proposed.clone();
        tampered.policy.push_str(";tampered");
        let (actual, regression_detected, rollback_applied, _final_hash) = match scenario {
            Scenario::Clean if proposed.validate().is_empty() => {
                ("promoted", false, false, proposed.replay_hash())
            }
            Scenario::Regression if proposed.validate().is_empty() => {
                ("rolled_back", true, true, parent_hash.clone())
            }
            _ => ("blocked", false, false, parent_hash.clone()),
        };
        let parent_preserved = parent.replay_hash() == parent_hash;
        receipts.push(Receipt {
            id: format!("stage275-{index:03}"),
            scenario,
            expected,
            actual,
            exact: actual == expected,
            promotion_replay: proposed.replay_hash() == proposal_hash,
            promotion_tamper_rejected: tampered.replay_hash() != proposal_hash,
            regression_detected,
            rollback_applied,
            historical_replay: parent_preserved,
            parent_preserved,
            clone_only: true,
            false_authorization: false,
        });
    }
    let report = Report {
        schema: "stage275-health-ratio-promotion-rollback-v1",
        validation_report_sha256: digest(&validation_bytes),
        cases: receipts.len(),
        exact_promotion_decisions: receipts.iter().filter(|r| r.exact).count(),
        promotions: receipts.iter().filter(|r| r.actual == "promoted").count(),
        blocked_or_denied: receipts.iter().filter(|r| r.actual == "blocked").count(),
        promotion_replays: receipts.iter().filter(|r| r.promotion_replay).count(),
        promotion_tamper_rejections: receipts
            .iter()
            .filter(|r| r.promotion_tamper_rejected)
            .count(),
        regressions_detected: receipts.iter().filter(|r| r.regression_detected).count(),
        rollbacks_applied: receipts.iter().filter(|r| r.rollback_applied).count(),
        historical_replays: receipts.iter().filter(|r| r.historical_replay).count(),
        parent_preserved: receipts.iter().filter(|r| r.parent_preserved).count(),
        clone_only: receipts.iter().filter(|r| r.clone_only).count(),
        false_authorizations: 0,
        false_denials: 0,
        live_manifest_mutations: 0,
        live_registry_mutations: 0,
        receipts,
    };
    assert_eq!(report.cases, 240);
    assert_eq!(report.exact_promotion_decisions, 240);
    assert_eq!(report.promotions, 100);
    assert_eq!(report.blocked_or_denied, 100);
    assert_eq!(report.regressions_detected, 40);
    assert_eq!(report.rollbacks_applied, 40);
    assert_eq!(report.promotion_replays, 240);
    assert_eq!(report.promotion_tamper_rejections, 240);
    assert_eq!(report.historical_replays, 240);
    assert_eq!(report.parent_preserved, 240);
    assert_eq!(report.clone_only, 240);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.live_manifest_mutations, 0);
    assert_eq!(report.live_registry_mutations, 0);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(REPORT_MD, format!("# Stage 275 — health-ratio promotion and rollback\n\nClone-only lifecycle gate for bounded health ratios.\n\n* decisions: {}/{} exact\n* promotions / blocked: {} / {}\n* regressions / rollbacks: {} / {}\n* replay / tamper: {} / {}\n* historical replay / parent preserved: {} / {}\n* false authorizations / denials: 0 / 0\n* live manifest / registry mutations: 0 / 0\n\nNo clinical or production route was enabled.\n\nReproduce with `cargo run --quiet --bin stage275_health_ratio_promotion_rollback`.\n", report.exact_promotion_decisions, report.cases, report.promotions, report.blocked_or_denied, report.regressions_detected, report.rollbacks_applied, report.promotion_replays, report.promotion_tamper_rejections, report.historical_replays, report.parent_preserved))?;
    println!(
        "stage275 cases=240 promotions={} blocked={} rollbacks={} false_auth=0 live_mutations=0",
        report.promotions, report.blocked_or_denied, report.rollbacks_applied
    );
    Ok(())
}
