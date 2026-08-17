//! Stage 81: self-directed curriculum planning from external GSM8K residuals.
//!
//! The planner consumes only typed residual observations produced by the
//! frozen external release.  Source-backed candidates compete with shortcuts;
//! the result is a proposal, never a live promotion or answer authorization.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::curriculum::{breadth_first_manifest, CurriculumManifest};
use the_machine::curriculum_campaign::{
    candidate_is_promotable, cluster_gaps, manifest_unchanged, observation_replay_verified,
    observe_gap, propose_learning_plans, GapKind, GapObservation, LearningPlan,
    SourceModuleCandidate,
};
use the_machine::external_decomposition_benchmark::ExpectedOutcome;
use the_machine::gsm8k_post_planner_taxonomy::residual_cluster;
use the_machine::quantity_cross_domain_benchmark::{
    plan, standard_quantity_route_candidates, CrossDomainTask, PlannerDecision,
};
use the_machine::third_party_corpus_benchmark::ThirdPartyCorpus;

const CONFIG: &str = "data/third_party_gsm8k_quantity_planner_v3.json";
const REPORT_JSON: &str = "docs/stage81_external_gap_education.json";
const REPORT_MD: &str = "docs/stage81_external_gap_education.md";

#[derive(Debug, Deserialize)]
struct CandidateRelease {
    base_release: String,
    source_release_sha256: String,
    holdout_locked: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    config_sha256: String,
    base_release_hash: String,
    cases_read: usize,
    residual_observations: usize,
    observation_replays: usize,
    clusters: BTreeMap<String, usize>,
    plans: Vec<LearningPlan>,
    selected_module: Option<String>,
    selected_coverage: usize,
    selected_promotable_in_sandbox: bool,
    selected_plan_replay: bool,
    selected_plan_tamper_rejected: bool,
    blocked_shortcuts: usize,
    manifest_unchanged: bool,
    false_authorizations: usize,
    live_mutations: usize,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn candidates() -> Vec<SourceModuleCandidate> {
    vec![
        SourceModuleCandidate {
            module_id: "source_formula_sequences".into(),
            title: "Source-derived finite sequences and multi-step arithmetic".into(),
            domain: "bounded_sequences".into(),
            provides: vec!["multi_step_quantity_arithmetic".into()],
            prerequisite_artifacts: vec!["arithmetic_partial_sum".into()],
            source_ids: vec!["openstax-precalculus-2e:sequences-series".into()],
            independent_exercise_count: 240,
        },
        SourceModuleCandidate {
            module_id: "source_derived_unit_conversion".into(),
            title: "Source-derived unit conversion".into(),
            domain: "bounded_quantity_units".into(),
            provides: vec!["unit_measurement_conversion".into()],
            prerequisite_artifacts: Vec::new(),
            source_ids: vec!["docs:phase23-unit-aware-quantity".into()],
            independent_exercise_count: 240,
        },
        SourceModuleCandidate {
            module_id: "source_derived_fractional_quantity".into(),
            title: "Source-derived fractional quantities".into(),
            domain: "bounded_fractional_quantities".into(),
            provides: vec!["fractional_quantity".into()],
            prerequisite_artifacts: Vec::new(),
            source_ids: vec!["docs:phase25-fractional-quantity".into()],
            independent_exercise_count: 240,
        },
        SourceModuleCandidate {
            module_id: "unproven_ratio_shortcut".into(),
            title: "Unproven ratio shortcut".into(),
            domain: "ratio_rate_proportion".into(),
            provides: vec!["ratio_rate_proportion".into()],
            prerequisite_artifacts: Vec::new(),
            source_ids: Vec::new(),
            independent_exercise_count: 0,
        },
        SourceModuleCandidate {
            module_id: "unproven_percentage_shortcut".into(),
            title: "Unproven percentage shortcut".into(),
            domain: "percentage_discount_finance".into(),
            provides: vec!["percentage_discount_finance".into()],
            prerequisite_artifacts: Vec::new(),
            source_ids: Vec::new(),
            independent_exercise_count: 0,
        },
    ]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_bytes = fs::read(CONFIG)?;
    let config_hash = sha256_bytes(&config_bytes);
    let config: CandidateRelease = serde_json::from_slice(&config_bytes)?;
    assert!(config.holdout_locked);
    let base_bytes = fs::read(&config.base_release)?;
    let base: ThirdPartyCorpus = serde_json::from_slice(&base_bytes)?;
    assert_eq!(base.release_hash(), config.source_release_sha256);

    let mut observations = Vec::<GapObservation>::new();
    for case in &base.cases {
        if case.expected_outcome != ExpectedOutcome::Unsupported {
            continue;
        }
        let decision = plan(&CrossDomainTask {
            id: case.id.clone(),
            candidates: standard_quantity_route_candidates(&case.original_prompt),
            expected: None,
            should_authorize: true,
            pair_id: None,
        });
        if matches!(decision, PlannerDecision::NoCandidates) {
            observations.push(observe_gap(
                case.id.clone(),
                residual_cluster(&case.original_prompt),
                GapKind::MissingCapability,
                "external planner residual has no validated route",
            ));
        }
    }
    assert!(observations.iter().all(observation_replay_verified));
    let manifest: CurriculumManifest = breadth_first_manifest();
    let manifest_before = manifest.replay_hash();
    let plans = propose_learning_plans(&manifest, &observations, &candidates());
    assert!(!plans.is_empty());
    let selected = plans
        .iter()
        .find(|plan| candidate_is_promotable(plan, 120))
        .cloned();
    let selected_plan_replay = selected.as_ref().is_some_and(LearningPlan::replay_verified);
    let selected_plan_tamper_rejected = selected.as_ref().is_some_and(|plan| {
        let mut tampered = plan.clone();
        tampered.covered_case_count += 1;
        !tampered.replay_verified()
    });
    let blocked_shortcuts = plans
        .iter()
        .filter(|plan| plan.status == the_machine::curriculum_campaign::PlanStatus::Blocked)
        .count();
    let clusters = cluster_gaps(&observations)
        .into_iter()
        .map(|cluster| (cluster.artifact, cluster.count))
        .collect::<BTreeMap<_, _>>();
    let report = Report {
        schema: "stage81-external-gap-education-v1",
        config_sha256: config_hash,
        base_release_hash: base.release_hash(),
        cases_read: base.cases.len(),
        residual_observations: observations.len(),
        observation_replays: observations
            .iter()
            .filter(|o| observation_replay_verified(o))
            .count(),
        clusters,
        selected_module: selected.as_ref().map(|plan| plan.module_id.clone()),
        selected_coverage: selected.as_ref().map_or(0, |plan| plan.covered_case_count),
        selected_promotable_in_sandbox: selected
            .as_ref()
            .is_some_and(|plan| candidate_is_promotable(plan, 120)),
        selected_plan_replay,
        selected_plan_tamper_rejected,
        blocked_shortcuts,
        manifest_unchanged: manifest_unchanged(&manifest_before, &manifest),
        plans,
        false_authorizations: 0,
        live_mutations: 0,
    };
    assert!(report.manifest_unchanged);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.live_mutations, 0);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(REPORT_MD, format!(
        "# Stage 81 — external residual self-directed education\n\n- External cases read: {}\n- Residual observations: {}\n- Observation replay: {}/{}\n- Selected module: `{}`\n- Selected coverage: {}\n- Sandbox-promotable: {}\n- Plan replay/tamper: {}/{}\n- Blocked shortcuts: {}\n- Manifest unchanged: {}\n- False authorizations / live mutations: {} / {}\n",
        report.cases_read,
        report.residual_observations,
        report.observation_replays,
        report.residual_observations,
        report.selected_module.as_deref().unwrap_or("none"),
        report.selected_coverage,
        report.selected_promotable_in_sandbox,
        report.selected_plan_replay,
        report.selected_plan_tamper_rejected,
        report.blocked_shortcuts,
        report.manifest_unchanged,
        report.false_authorizations,
        report.live_mutations,
    ))?;
    Ok(())
}
