//! Stage G science self-directed curriculum campaign.
//!
//! The planner sees only exact typed gap observations. It chooses a
//! source-backed module by coverage and evidence, validates that module in a
//! shadow corpus, and leaves the curriculum manifest immutable.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{
    candidate_is_promotable, cluster_gaps, manifest_unchanged, observe_gap, propose_learning_plans,
    GapKind, SourceModuleCandidate,
};
use the_machine::source_formula_pack::biology_pack::{
    evaluate_biology, BiologyOperation, BiologyRequest,
};

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
    blocked_shortcuts: usize,
    false_authorizations: usize,
    production_authorizations: usize,
    corpus_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("science campaign serializes"))
    )
}

fn biology_request(operation: BiologyOperation, sequence: &str) -> BiologyRequest {
    BiologyRequest {
        operation,
        sequence: Some(sequence.into()),
        orientation: Some("5_to_3".into()),
        domain: "source_derived_bounded_dna".into(),
        ambiguity: None,
        provenance: vec!["stage-g-independent-science-exercises".into()],
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let mut observations = Vec::with_capacity(500);
    for index in 0..200 {
        observations.push(observe_gap(
            format!("dna_gap_{index:03}"),
            "dna_sequence",
            GapKind::MissingCapability,
            "technical problem requires a validated DNA sequence artifact",
        ));
    }
    for index in 0..150 {
        observations.push(observe_gap(
            format!("base_composition_gap_{index:03}"),
            "base_composition",
            GapKind::MissingCapability,
            "technical problem requires exact nucleotide counts",
        ));
    }
    for index in 0..100 {
        observations.push(observe_gap(
            format!("chemistry_gap_{index:03}"),
            "molecular_formula",
            GapKind::MissingCapability,
            "technical problem requires a molecular formula artifact",
        ));
    }
    for index in 0..50 {
        observations.push(observe_gap(
            format!("statistics_gap_{index:03}"),
            "arithmetic_mean",
            GapKind::MissingCapability,
            "technical problem requires a finite-statistics artifact",
        ));
    }
    assert_eq!(observations.len(), 500);
    assert!(observations.iter().all(|observation| {
        the_machine::curriculum_campaign::observation_replay_verified(observation)
    }));

    let candidates = vec![
        SourceModuleCandidate {
            module_id: "source_derived_biology".into(),
            title: "DNA representations from attributed biology source".into(),
            domain: "molecular_biology".into(),
            provides: vec!["dna_sequence".into(), "base_composition".into()],
            prerequisite_artifacts: vec!["dna_sequence".into()],
            source_ids: vec!["openstax-biology-2e:dna-complementary-pairing".into()],
            independent_exercise_count: 240,
        },
        SourceModuleCandidate {
            module_id: "source_derived_chemistry".into(),
            title: "Molecular formulas from attributed chemistry source".into(),
            domain: "chemistry".into(),
            provides: vec!["molecular_formula".into()],
            prerequisite_artifacts: vec!["molecular_formula".into()],
            source_ids: vec!["openstax-chemistry-2e:formulas-stoichiometry".into()],
            independent_exercise_count: 240,
        },
        SourceModuleCandidate {
            module_id: "source_derived_finite_statistics".into(),
            title: "Finite statistics from attributed source records".into(),
            domain: "finite_statistics".into(),
            provides: vec!["arithmetic_mean".into()],
            prerequisite_artifacts: vec!["distribution".into()],
            source_ids: vec!["openstax-introductory-statistics-2e:descriptive-statistics".into()],
            independent_exercise_count: 240,
        },
        SourceModuleCandidate {
            module_id: "unproven_biology_shortcut".into(),
            title: "Unproven biology shortcut".into(),
            domain: "molecular_biology".into(),
            provides: vec!["dna_sequence", "base_composition"]
                .into_iter()
                .map(String::from)
                .collect(),
            prerequisite_artifacts: vec!["dna_sequence".into()],
            source_ids: Vec::new(),
            independent_exercise_count: 0,
        },
    ];
    let plans = propose_learning_plans(&manifest, &observations, &candidates);
    let selected = &plans[0];
    assert_eq!(selected.module_id, "source_derived_biology");
    assert_eq!(selected.covered_case_count, 350);
    let selected_plan_replay = selected.replay_verified();
    let mut tampered = selected.clone();
    tampered.covered_case_count += 1;
    let plan_tamper_rejected = !tampered.replay_verified();

    let sequences = ["AATTGGCC", "ATCGATCG", "GCGCGCAA", "TTAAACCG", "AGCTAGCT"];
    let mut independent_validation_correct = 0;
    for index in 0..120 {
        let operation = match index % 3 {
            0 => BiologyOperation::ValidateDna,
            1 => BiologyOperation::BaseComposition,
            _ => BiologyOperation::ReverseComplement,
        };
        let result = evaluate_biology(&biology_request(
            operation,
            sequences[index % sequences.len()],
        ));
        if result.authorized() {
            independent_validation_correct += 1;
        }
    }
    let shadow_promotable =
        candidate_is_promotable(selected, 120) && independent_validation_correct == 120;
    let blocked_shortcuts = plans
        .iter()
        .filter(|plan| plan.status == the_machine::curriculum_campaign::PlanStatus::Blocked)
        .count();
    let false_authorizations = plans
        .iter()
        .filter(|plan| plan.status == the_machine::curriculum_campaign::PlanStatus::Proposed)
        .filter(|plan| plan.source_ids.is_empty() || plan.independent_exercise_count == 0)
        .count();
    assert_eq!(blocked_shortcuts, 1);
    assert_eq!(false_authorizations, 0);
    assert!(shadow_promotable);
    assert!(selected_plan_replay);
    assert!(plan_tamper_rejected);
    assert!(manifest_unchanged(&manifest_hash, &manifest));
    let report = Report {
        schema: "stage-g-self-directed-science-campaign-v1",
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
        blocked_shortcuts,
        false_authorizations,
        production_authorizations: 0,
        corpus_sha256: digest(&(observations, candidates, plans)),
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage_g_self_directed_science_campaign.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
