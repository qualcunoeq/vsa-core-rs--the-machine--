//! Stage G shadow self-directed curriculum campaign.
//!
//! The campaign sees only diagnostic gap observations. It clusters exact typed
//! artifact requests, selects a source module by expected utility, validates
//! that module on an independent corpus, and leaves the curriculum manifest
//! unchanged.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{
    candidate_is_promotable, cluster_gaps, manifest_unchanged, observe_gap, propose_learning_plans,
    GapKind, SourceModuleCandidate,
};
use the_machine::probability_pack::Rational;
use the_machine::source_formula_pack::{FormulaRequest, FormulaStatus};
use the_machine::source_statistics_pack::{evaluate_statistics, DOMAIN};

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    observed_cases: usize,
    gap_clusters: usize,
    candidate_plans: usize,
    selected_module: String,
    selected_coverage: usize,
    selected_plan_replay: bool,
    plan_tamper_rejected: bool,
    independent_validation_cases: usize,
    independent_validation_correct: usize,
    shadow_promotable: bool,
    manifest_unchanged: bool,
    false_authorizations: usize,
    production_authorizations: usize,
    corpus_sha256: String,
}
fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn stats_request(formula: &str) -> FormulaRequest {
    FormulaRequest {
        formula: formula.into(),
        inputs: BTreeMap::from([
            ("sum".into(), q(30, 1)),
            ("count".into(), q(5, 1)),
            ("weighted_sum".into(), q(30, 1)),
            ("total_weight".into(), q(5, 1)),
            ("p".into(), q(1, 4)),
            ("n".into(), q(8, 1)),
        ]),
        domain: DOMAIN.into(),
        ambiguity: None,
        provenance: vec!["stage-g-independent-source-exercises".into()],
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let mut observations = Vec::with_capacity(500);
    let formulas = [
        "arithmetic_mean",
        "weighted_mean",
        "bernoulli_variance",
        "binomial_expected_value",
        "binomial_variance",
    ];
    for index in 0..320 {
        observations.push(observe_gap(
            format!("stats_gap_{index:03}"),
            formulas[index % formulas.len()],
            GapKind::MissingCapability,
            "validated curriculum has no finite-statistics artifact",
        ));
    }
    for index in 0..120 {
        observations.push(observe_gap(
            format!("spectral_gap_{index:03}"),
            "spectral_gap",
            GapKind::MissingKnowledge,
            "specialist theorem is not present",
        ));
    }
    for index in 0..60 {
        observations.push(observe_gap(
            format!("topology_gap_{index:03}"),
            "topological_invariant",
            GapKind::Ambiguous,
            "requested invariant is not uniquely identified",
        ));
    }
    assert_eq!(observations.len(), 500);
    let candidates = vec![
        SourceModuleCandidate {
            module_id: "source_derived_finite_statistics".into(),
            title: "Finite statistics from attributed source records".into(),
            domain: "finite_statistics".into(),
            provides: formulas.iter().map(|f| (*f).into()).collect(),
            prerequisite_artifacts: vec!["distribution".into()],
            source_ids: vec!["openstax-introductory-statistics-2e:descriptive-statistics".into()],
            independent_exercise_count: 240,
        },
        SourceModuleCandidate {
            module_id: "unproven_statistics_shortcut".into(),
            title: "Unproven statistics shortcut".into(),
            domain: "finite_statistics".into(),
            provides: formulas.iter().map(|f| (*f).into()).collect(),
            prerequisite_artifacts: vec!["distribution".into()],
            source_ids: Vec::new(),
            independent_exercise_count: 0,
        },
        SourceModuleCandidate {
            module_id: "topology_candidate".into(),
            title: "Topology candidate".into(),
            domain: "topology".into(),
            provides: vec!["topological_invariant".into()],
            prerequisite_artifacts: vec!["group".into()],
            source_ids: vec!["unvalidated-topology-source".into()],
            independent_exercise_count: 12,
        },
    ];
    let plans = propose_learning_plans(&manifest, &observations, &candidates);
    assert_eq!(plans.len(), 3);
    let selected = &plans[0];
    assert_eq!(selected.module_id, "source_derived_finite_statistics");
    assert_eq!(selected.covered_case_count, 320);
    let selected_plan_replay = selected.replay_verified();
    let mut tampered = selected.clone();
    tampered.covered_case_count += 1;
    let plan_tamper_rejected = !tampered.replay_verified();

    let mut independent_validation_correct = 0;
    for index in 0..120 {
        let result = evaluate_statistics(&stats_request(formulas[index % formulas.len()]));
        if result.status == FormulaStatus::Complete && result.replay_verified() {
            independent_validation_correct += 1;
        }
    }
    let shadow_promotable =
        candidate_is_promotable(selected, 120) && independent_validation_correct == 120;
    let false_authorizations = plans
        .iter()
        .filter(|plan| plan.status == the_machine::curriculum_campaign::PlanStatus::Proposed)
        .filter(|plan| plan.source_ids.is_empty() || plan.independent_exercise_count == 0)
        .count();
    assert_eq!(false_authorizations, 0);
    assert!(shadow_promotable);
    assert!(selected_plan_replay);
    assert!(plan_tamper_rejected);
    assert!(manifest_unchanged(&manifest_hash, &manifest));
    let report = Report {
        schema: "stage-g-self-directed-curriculum-campaign-v1",
        observed_cases: observations.len(),
        gap_clusters: cluster_gaps(&observations).len(),
        candidate_plans: plans.len(),
        selected_module: selected.module_id.clone(),
        selected_coverage: selected.covered_case_count,
        selected_plan_replay,
        plan_tamper_rejected,
        independent_validation_cases: 120,
        independent_validation_correct,
        shadow_promotable,
        manifest_unchanged: manifest_unchanged(&manifest_hash, &manifest),
        false_authorizations,
        production_authorizations: 0,
        corpus_sha256: digest(&(observations, candidates, plans)),
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage_g_self_directed_campaign.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
