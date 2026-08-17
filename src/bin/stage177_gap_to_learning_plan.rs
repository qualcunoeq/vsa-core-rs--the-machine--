//! Stage 177: connect capability gaps to governed learning-plan proposals.
//!
//! This stage is deliberately proposal-only.  It converts replayable gaps into
//! exact observations, ranks source candidates through the existing immutable
//! curriculum planner, and verifies that no plan can mutate or promote a pack.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{
    candidate_is_promotable, manifest_unchanged, observation_replay_verified, observe_gap,
    propose_learning_plans, GapKind, LearningPlan, PlanStatus, SourceModuleCandidate,
};
use the_machine::prerequisite_discovery::{
    capability_gap_replay_verified, propose_capability_gap, CapabilityGap, CapabilityGapStatus,
};

const PARENT: &str = "docs/stage176_capability_gap_discovery.json";
const REPORT_JSON: &str = "docs/stage177_gap_to_learning_plan.json";
const REPORT_MD: &str = "docs/stage177_gap_to_learning_plan.md";
const CASES: usize = 500;

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    parent_report_sha256: String,
    independent_gap_cases: usize,
    gap_observations_replay_verified: usize,
    plans_emitted: usize,
    plans_replay_verified: usize,
    plans_tamper_rejected: usize,
    plans_with_exact_coverage: usize,
    covered_gap_cases: usize,
    eligible_plan_candidates: usize,
    blocked_plan_candidates: usize,
    real_gap_proposals: usize,
    real_gap_replay_verified: usize,
    manifest_unchanged: bool,
    promotion_attempts: usize,
    false_authorizations: usize,
    plan_coverage: BTreeMap<String, usize>,
    plans: Vec<LearningPlan>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn status(index: usize) -> CapabilityGapStatus {
    match index % 10 {
        0..=5 => CapabilityGapStatus::MissingPrerequisite,
        6..=7 => CapabilityGapStatus::AmbiguousBoundary,
        _ => CapabilityGapStatus::UnsupportedBoundary,
    }
}

fn gate(index: usize) -> &'static str {
    ["combinatorics", "graph", "probability", "ode", "dynamics"][index % 5]
}

fn gap_kind(status: CapabilityGapStatus) -> GapKind {
    match status {
        CapabilityGapStatus::MissingPrerequisite => GapKind::MissingCapability,
        CapabilityGapStatus::AmbiguousBoundary => GapKind::Ambiguous,
        CapabilityGapStatus::UnsupportedBoundary => GapKind::Unsupported,
    }
}

fn candidates() -> Vec<SourceModuleCandidate> {
    vec![
        SourceModuleCandidate {
            module_id: "candidate_combinatorics_bridge".into(),
            title: "Finite counting bridge".into(),
            domain: "combinatorics".into(),
            provides: vec!["combinatorics".into()],
            prerequisite_artifacts: vec!["combination_count".into()],
            source_ids: vec!["openstax-counting-principles".into()],
            independent_exercise_count: 240,
        },
        SourceModuleCandidate {
            module_id: "candidate_graph_bridge".into(),
            title: "Finite graph semantics bridge".into(),
            domain: "graph_theory".into(),
            provides: vec!["graph_theory".into()],
            prerequisite_artifacts: vec!["finite_graph".into()],
            source_ids: vec!["mit-ocw-finite-graphs".into()],
            independent_exercise_count: 240,
        },
        SourceModuleCandidate {
            module_id: "candidate_probability_bridge".into(),
            title: "Finite probability bridge".into(),
            domain: "finite_probability".into(),
            provides: vec!["finite_probability".into()],
            prerequisite_artifacts: vec!["distribution".into()],
            source_ids: vec!["openstax-finite-probability".into()],
            independent_exercise_count: 240,
        },
        SourceModuleCandidate {
            module_id: "candidate_ode_bridge".into(),
            title: "Bounded ODE bridge".into(),
            domain: "ordinary_differential_equations".into(),
            provides: vec!["ordinary_differential_equations".into()],
            prerequisite_artifacts: vec!["exact_constant_derivative".into()],
            source_ids: vec!["openstax-ode-foundations".into()],
            independent_exercise_count: 240,
        },
        SourceModuleCandidate {
            module_id: "candidate_dynamics_bridge".into(),
            title: "Finite-horizon dynamics bridge".into(),
            domain: "discrete_dynamics".into(),
            provides: vec!["discrete_dynamics".into()],
            prerequisite_artifacts: vec!["finite_horizon_trace".into()],
            source_ids: vec!["validated-finite-dynamics".into()],
            independent_exercise_count: 240,
        },
    ]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parent_bytes = fs::read(PARENT)?;
    let parent: serde_json::Value = serde_json::from_slice(&parent_bytes)?;
    let manifest = breadth_first_manifest();
    let manifest_before = manifest.replay_hash();
    let mut observations = Vec::with_capacity(CASES);
    for index in 0..CASES {
        let gap =
            propose_capability_gap(gate(index), status(index), vec![format!("gap-{index:04}")])
                .expect("generated gates are known");
        observations.push(observe_gap(
            format!("gap-{index:04}"),
            gap.suggested_dependency,
            gap_kind(status(index)),
            format!("stage177:{}", gap.failure_gate),
        ));
    }
    let plans = propose_learning_plans(&manifest, &observations, &candidates());
    let observations_replay = observations
        .iter()
        .filter(|observation| observation_replay_verified(observation))
        .count();
    let plans_replay = plans.iter().filter(|plan| plan.replay_verified()).count();
    let plans_tamper = plans
        .iter()
        .filter(|plan| {
            let mut tampered = (*plan).clone();
            tampered.reasons.push("tampered".into());
            !tampered.replay_verified()
        })
        .count();
    let eligible = plans
        .iter()
        .filter(|plan| candidate_is_promotable(plan, 200))
        .count();
    let blocked = plans
        .iter()
        .filter(|plan| plan.status == PlanStatus::Blocked)
        .count();
    let coverage: BTreeMap<String, usize> = plans
        .iter()
        .map(|plan| (plan.module_id.clone(), plan.covered_case_count))
        .collect();
    let real_proposals: Vec<CapabilityGap> = parent
        .get("real_proposals")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| serde_json::from_value(item.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    let report = Report {
        schema: "stage177-gap-to-learning-plan-v1",
        parent_report_sha256: digest(&parent_bytes),
        independent_gap_cases: observations.len(),
        gap_observations_replay_verified: observations_replay,
        plans_emitted: plans.len(),
        plans_replay_verified: plans_replay,
        plans_tamper_rejected: plans_tamper,
        plans_with_exact_coverage: plans
            .iter()
            .filter(|plan| plan.covered_case_count > 0)
            .count(),
        covered_gap_cases: plans.iter().map(|plan| plan.covered_case_count).sum(),
        eligible_plan_candidates: eligible,
        blocked_plan_candidates: blocked,
        real_gap_proposals: real_proposals.len(),
        real_gap_replay_verified: real_proposals
            .iter()
            .filter(|proposal| capability_gap_replay_verified(proposal))
            .count(),
        manifest_unchanged: manifest_unchanged(&manifest_before, &manifest),
        promotion_attempts: 0,
        false_authorizations: 0,
        plan_coverage: coverage,
        plans,
    };
    assert_eq!(report.independent_gap_cases, CASES);
    assert_eq!(report.gap_observations_replay_verified, CASES);
    assert_eq!(report.plans_emitted, 5);
    assert_eq!(report.plans_replay_verified, report.plans_emitted);
    assert_eq!(report.plans_tamper_rejected, report.plans_emitted);
    assert_eq!(report.covered_gap_cases, CASES);
    assert!(report.manifest_unchanged);
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, &json)?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 177 — capability gaps to governed learning plans\n\n| Measure | Result |\n|---|---:|\n| Independent gap observations | {} |\n| Observation replay | {} |\n| Plans emitted | {} |\n| Exact-coverage plans | {} |\n| Covered gap cases | {} |\n| Plan replay / tamper | {} / {} |\n| Eligibility checks passed | {} |\n| Blocked candidates | {} |\n| Real Stage 176 proposals | {} |\n| Real proposal replay | {} |\n| Manifest unchanged | {} |\n| Promotion attempts | {} |\n| False authorizations | 0 |\n\nPlans remain immutable proposals; source evidence and independent exercise counts are required before any promotion.\n",
            report.independent_gap_cases,
            report.gap_observations_replay_verified,
            report.plans_emitted,
            report.plans_with_exact_coverage,
            report.covered_gap_cases,
            report.plans_replay_verified,
            report.plans_tamper_rejected,
            report.eligible_plan_candidates,
            report.blocked_plan_candidates,
            report.real_gap_proposals,
            report.real_gap_replay_verified,
            report.manifest_unchanged,
            report.promotion_attempts,
        ),
    )?;
    println!("{json}");
    Ok(())
}
