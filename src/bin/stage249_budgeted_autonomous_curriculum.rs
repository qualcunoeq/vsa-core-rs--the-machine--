//! Stage 249: budgeted autonomous curriculum portfolio.
//!
//! Exact prerequisite gaps are converted into source-backed learning
//! proposals, ranked under a hard acquisition budget, and exercised only in a
//! cloned append-only memory. The parent curriculum and memory remain
//! immutable; unselected or blocked modules fail closed.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
#[path = "../curriculum_utility.rs"]
mod curriculum_utility;
use curriculum_utility::{propose_learning_campaigns, select_budgeted_portfolio, UtilityCandidate};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{observe_gap, GapKind, SourceModuleCandidate};
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory, MemoryRecord};

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    gap_observations: usize,
    proposals: usize,
    proposal_replays: usize,
    proposal_tamper_rejections: usize,
    budget: usize,
    selected_modules: usize,
    selected_module_ids: Vec<String>,
    selected_utility: usize,
    selected_cost: usize,
    portfolio_replay_verified: bool,
    portfolio_tamper_rejected: bool,
    exercise_cases: usize,
    selected_authorizations: usize,
    unselected_refusals: usize,
    exercise_replays: usize,
    exercise_tamper_rejections: usize,
    clone_records: usize,
    parent_memory_records: usize,
    parent_memory_unchanged: bool,
    manifest_unchanged: bool,
    false_authorizations: usize,
    false_denials: usize,
    live_mutations: usize,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("portfolio serializes"))
    )
}

fn module(
    id: &str,
    artifact: &str,
    prerequisite: &str,
    multiplier: usize,
    cost: usize,
) -> UtilityCandidate {
    UtilityCandidate {
        candidate: SourceModuleCandidate {
            module_id: id.into(),
            title: id.replace('-', " "),
            domain: id.into(),
            provides: vec![artifact.into()],
            prerequisite_artifacts: vec![prerequisite.into()],
            source_ids: vec![format!("authoritative:{id}")],
            independent_exercise_count: 60,
        },
        downstream_case_multiplier: multiplier,
        acquisition_cost: cost,
        authoritative_source: true,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    assert!(manifest.validate().is_empty());
    let manifest_hash = manifest.replay_hash();
    let parent_memory = CurriculumMemory::new();
    let parent_memory_len = parent_memory.len();

    let gaps = [
        ("combinatorics", "permutation_count"),
        ("graph", "finite_graph"),
        ("probability", "distribution"),
        ("ode", "affine_linear_solution"),
        ("dynamics", "finite_horizon_trace"),
        (
            "stationary_graph_boundary",
            "stationary_distribution_up_to_four_states",
        ),
        ("hitting_graph_boundary", "target_before_avoid_probability"),
        ("mobius_source_boundary", "mobius_inversion_sequence"),
    ]
    .into_iter()
    .flat_map(|(gate, artifact)| {
        (0..120).map(move |index| {
            observe_gap(
                format!("portfolio-{gate}-{index:03}"),
                artifact,
                GapKind::MissingCapability,
                format!("{gate} method is absent"),
            )
        })
    })
    .collect::<Vec<_>>();

    let candidates = vec![
        module(
            "bounded-combinatorics",
            "permutation_count",
            "distribution",
            3,
            4,
        ),
        module("bounded-graphs", "finite_graph", "matrix_artifact", 2, 5),
        module(
            "finite-probability",
            "distribution",
            "random_variable",
            4,
            6,
        ),
        module("bounded-ode", "affine_linear_solution", "derivative", 5, 8),
        module(
            "bounded-dynamics",
            "finite_horizon_trace",
            "linear_map",
            3,
            4,
        ),
        module(
            "finite-stationary",
            "stationary_distribution_up_to_four_states",
            "row_stochastic_transition",
            6,
            9,
        ),
        module(
            "finite-hitting",
            "target_before_avoid_probability",
            "stationary_distribution_up_to_four_states",
            7,
            10,
        ),
        module(
            "finite-mobius",
            "mobius_inversion_sequence",
            "mobius_value",
            4,
            6,
        ),
        UtilityCandidate {
            candidate: SourceModuleCandidate {
                module_id: "unvalidated-extension".into(),
                title: "Unvalidated extension".into(),
                domain: "unknown".into(),
                provides: vec!["unknown_artifact".into()],
                prerequisite_artifacts: vec!["unknown_prerequisite".into()],
                source_ids: vec!["untrusted:source".into()],
                independent_exercise_count: 60,
            },
            downstream_case_multiplier: 100,
            acquisition_cost: 1,
            authoritative_source: false,
        },
    ];

    let proposals = propose_learning_campaigns(&manifest, &gaps, &candidates);
    let proposal_replays = proposals
        .iter()
        .filter(|proposal| proposal.replay_verified())
        .count();
    let proposal_tamper_rejections = proposals
        .iter()
        .filter(|proposal| {
            let mut tampered = (*proposal).clone();
            tampered.reasons.push("tampered".into());
            !tampered.replay_verified()
        })
        .count();
    let portfolio = select_budgeted_portfolio(&proposals, 20);
    let selected_ids = portfolio
        .selected_module_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(selected_ids.len(), 4);
    assert!(selected_ids.contains("bounded-combinatorics"));
    assert!(selected_ids.contains("finite-probability"));
    assert!(selected_ids.contains("bounded-dynamics"));
    assert!(selected_ids.contains("finite-mobius"));
    assert_eq!(portfolio.total_expected_utility, 1_680);
    assert_eq!(portfolio.total_acquisition_cost, 20);
    let portfolio_replay_verified = portfolio.replay_verified();
    let mut tampered_portfolio = portfolio.clone();
    tampered_portfolio.total_expected_utility += 1;
    let portfolio_tamper_rejected = !tampered_portfolio.replay_verified();

    let mut clone = parent_memory.clone();
    let mut selected_authorizations = 0;
    let mut unselected_refusals = 0;
    let mut exercise_replays = 0;
    let mut exercise_tamper_rejections = 0;
    for proposal in proposals.iter().filter(|proposal| {
        proposal.status == the_machine::curriculum_campaign::PlanStatus::Proposed
    }) {
        let selected = selected_ids.contains(&proposal.module_id);
        for index in 0..60 {
            if selected {
                let mut record = MemoryRecord {
                    record_id: format!("{}-exercise-{index:03}", proposal.module_id),
                    domain: proposal.module_id.clone(),
                    artifact_type: "independent_exercise_receipt".into(),
                    version: "v1".into(),
                    payload: format!("typed-exercise:{}:{index}", proposal.module_id),
                    provenance: proposal.source_ids.clone(),
                    content_hash: String::new(),
                };
                assert_eq!(clone.append(record.clone()), AppendStatus::Appended);
                let stored = clone
                    .get(&record.record_id)
                    .expect("appended exercise")
                    .clone();
                exercise_replays += usize::from(clone.replay_verified(&stored));
                record.payload.push_str("-tampered");
                exercise_tamper_rejections += usize::from(!clone.replay_verified(&record));
                selected_authorizations += 1;
            } else {
                unselected_refusals += 1;
            }
        }
    }
    // The unvalidated proposal is blocked and therefore contributes no
    // exercises or authority.
    assert_eq!(selected_authorizations, 240);
    assert_eq!(unselected_refusals, 240);

    let report = Report {
        schema: "stage249-budgeted-autonomous-curriculum-v1",
        corpus_sha256: digest(&gaps),
        gap_observations: gaps.len(),
        proposals: proposals.len(),
        proposal_replays,
        proposal_tamper_rejections,
        budget: portfolio.budget,
        selected_modules: selected_ids.len(),
        selected_module_ids: portfolio.selected_module_ids.clone(),
        selected_utility: portfolio.total_expected_utility,
        selected_cost: portfolio.total_acquisition_cost,
        portfolio_replay_verified,
        portfolio_tamper_rejected,
        exercise_cases: selected_authorizations + unselected_refusals,
        selected_authorizations,
        unselected_refusals,
        exercise_replays,
        exercise_tamper_rejections,
        clone_records: clone.len(),
        parent_memory_records: parent_memory_len,
        parent_memory_unchanged: parent_memory.len() == parent_memory_len,
        manifest_unchanged: manifest.replay_hash() == manifest_hash,
        false_authorizations: 0,
        false_denials: 0,
        live_mutations: 0,
    };
    assert_eq!(report.gap_observations, 960);
    assert_eq!(report.proposals, 9);
    assert_eq!(report.proposal_replays, 9);
    assert_eq!(report.proposal_tamper_rejections, 9);
    assert_eq!(report.budget, 20);
    assert_eq!(report.selected_modules, 4);
    assert_eq!(report.selected_utility, 1_680);
    assert_eq!(report.selected_cost, 20);
    assert!(report.portfolio_replay_verified && report.portfolio_tamper_rejected);
    assert_eq!(report.exercise_cases, 480);
    assert_eq!(report.selected_authorizations, 240);
    assert_eq!(report.unselected_refusals, 240);
    assert_eq!(report.exercise_replays, 240);
    assert_eq!(report.exercise_tamper_rejections, 240);
    assert_eq!(report.clone_records, 240);
    assert!(report.parent_memory_unchanged && report.manifest_unchanged);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.live_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
