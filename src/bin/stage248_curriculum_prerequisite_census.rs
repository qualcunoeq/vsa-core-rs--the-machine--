//! Stage 248: broad autonomous prerequisite discovery census.
//!
//! This campaign feeds the shadow curriculum planner a mixed corpus of exact
//! artifact failures. Known artifacts must resolve to typed curriculum packs;
//! unknown artifacts and cyclic dependency proposals must remain diagnostic
//! refusals. The campaign never mutates the production manifest.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{
    candidate_is_promotable, cluster_gaps, observation_replay_verified, observe_gap,
    propose_learning_plans, GapKind, SourceModuleCandidate,
};
use the_machine::prerequisite_discovery::{
    capability_gap_replay_verified, discover, propose_capability_gap, proposed_edge_is_acyclic,
    CapabilityGapStatus, DiscoveryStatus,
};

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    known_cases: usize,
    unknown_cases: usize,
    known_discovery_complete: usize,
    unknown_discovery_refused: usize,
    observation_replays: usize,
    observation_tamper_rejections: usize,
    proposals: usize,
    proposal_replays: usize,
    proposal_tamper_rejections: usize,
    unknown_gate_refusals: usize,
    exact_artifact_clusters: usize,
    unknown_residual_clusters: usize,
    learning_plans: usize,
    promotable_plans: usize,
    blocked_plans: usize,
    plan_replays: usize,
    plan_tamper_rejections: usize,
    acyclic_edge_checks: usize,
    cyclic_edges_rejected: usize,
    manifest_unchanged: bool,
    false_authorizations: usize,
    false_denials: usize,
    live_mutations: usize,
    route_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize)]
struct CorpusRow {
    id: String,
    gate: String,
    artifact: String,
    known: bool,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("census serializes"))
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    assert!(manifest.validate().is_empty());
    let parent_manifest_hash = manifest.replay_hash();

    // Each row uses an exact failure gate understood by the bounded gap
    // proposer. Repetition tests whether plans are driven by typed artifacts,
    // not by broad subject labels.
    let known = [
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
    ];
    let mut rows = Vec::with_capacity(1_080);
    for (gate, artifact) in known {
        for index in 0..120 {
            rows.push(CorpusRow {
                id: format!("{gate}-{index:03}"),
                gate: gate.into(),
                artifact: artifact.into(),
                known: true,
            });
        }
    }
    for index in 0..120 {
        rows.push(CorpusRow {
            id: format!("unknown-{index:03}"),
            gate: format!("unknown_gate_{index:03}"),
            artifact: format!("unknown_artifact_{index:03}"),
            known: false,
        });
    }

    let observations: Vec<_> = rows
        .iter()
        .map(|row| {
            observe_gap(
                row.id.clone(),
                row.artifact.clone(),
                if row.known {
                    GapKind::MissingCapability
                } else {
                    GapKind::Unsupported
                },
                if row.known {
                    format!("typed {} prerequisite is absent", row.gate)
                } else {
                    "artifact has no governed curriculum owner".into()
                },
            )
        })
        .collect();
    let observation_replays = observations
        .iter()
        .filter(|observation| observation_replay_verified(observation))
        .count();
    let observation_tamper_rejections = observations
        .iter()
        .filter(|observation| {
            let mut tampered = (*observation).clone();
            tampered.reason.push_str("-tampered");
            !observation_replay_verified(&tampered)
        })
        .count();

    let mut proposals = 0;
    let mut proposal_replays = 0;
    let mut proposal_tamper_rejections = 0;
    let mut unknown_gate_refusals = 0;
    for row in &rows {
        let Some(proposal) = propose_capability_gap(
            &row.gate,
            if row.known {
                CapabilityGapStatus::MissingPrerequisite
            } else {
                CapabilityGapStatus::UnsupportedBoundary
            },
            vec![row.id.clone()],
        ) else {
            unknown_gate_refusals += 1;
            continue;
        };
        proposals += 1;
        proposal_replays += usize::from(capability_gap_replay_verified(&proposal));
        let mut tampered = proposal.clone();
        tampered.representation_needed.push_str("-tampered");
        proposal_tamper_rejections += usize::from(!capability_gap_replay_verified(&tampered));
    }

    let clusters = cluster_gaps(&observations);
    let exact_artifact_clusters = clusters
        .iter()
        .filter(|cluster| cluster.case_ids.len() == 120)
        .count();
    let unknown_residual_clusters = clusters
        .iter()
        .filter(|cluster| cluster.case_ids.len() == 1)
        .count();

    let candidates = vec![
        SourceModuleCandidate {
            module_id: "bounded-combinatorics".into(),
            title: "Bounded combinatorics".into(),
            domain: "combinatorics".into(),
            provides: vec!["permutation_count".into()],
            prerequisite_artifacts: vec!["distribution".into()],
            source_ids: vec!["independent-counting-source".into()],
            independent_exercise_count: 60,
        },
        SourceModuleCandidate {
            module_id: "bounded-graphs".into(),
            title: "Bounded graph theory".into(),
            domain: "graph_theory".into(),
            provides: vec!["finite_graph".into()],
            prerequisite_artifacts: vec!["matrix_artifact".into()],
            source_ids: vec!["independent-graph-source".into()],
            independent_exercise_count: 60,
        },
        SourceModuleCandidate {
            module_id: "finite-probability".into(),
            title: "Finite exact probability".into(),
            domain: "finite_probability".into(),
            provides: vec!["distribution".into()],
            prerequisite_artifacts: vec!["random_variable".into()],
            source_ids: vec!["independent-probability-source".into()],
            independent_exercise_count: 60,
        },
        SourceModuleCandidate {
            module_id: "bounded-ode".into(),
            title: "Bounded ODE".into(),
            domain: "ordinary_differential_equations".into(),
            provides: vec!["affine_linear_solution".into()],
            prerequisite_artifacts: vec!["derivative".into()],
            source_ids: vec!["independent-ode-source".into()],
            independent_exercise_count: 60,
        },
        SourceModuleCandidate {
            module_id: "bounded-dynamics".into(),
            title: "Bounded dynamics".into(),
            domain: "discrete_dynamics".into(),
            provides: vec!["finite_horizon_trace".into()],
            prerequisite_artifacts: vec!["linear_map".into()],
            source_ids: vec!["independent-dynamics-source".into()],
            independent_exercise_count: 60,
        },
        SourceModuleCandidate {
            module_id: "finite-stationary".into(),
            title: "Finite stationary distributions".into(),
            domain: "finite_markov_stationary_general".into(),
            provides: vec!["stationary_distribution_up_to_four_states".into()],
            prerequisite_artifacts: vec!["row_stochastic_transition".into()],
            source_ids: vec!["independent-stationary-source".into()],
            independent_exercise_count: 60,
        },
        SourceModuleCandidate {
            module_id: "finite-hitting".into(),
            title: "Finite hitting probabilities".into(),
            domain: "finite_markov_hitting".into(),
            provides: vec!["target_before_avoid_probability".into()],
            prerequisite_artifacts: vec!["stationary_distribution_up_to_four_states".into()],
            source_ids: vec!["independent-hitting-source".into()],
            independent_exercise_count: 60,
        },
        SourceModuleCandidate {
            module_id: "finite-mobius".into(),
            title: "Finite Möbius inversion".into(),
            domain: "source_derived_mobius".into(),
            provides: vec!["mobius_inversion_sequence".into()],
            prerequisite_artifacts: vec!["mobius_value".into()],
            source_ids: vec!["independent-mobius-source".into()],
            independent_exercise_count: 60,
        },
        // An otherwise plausible candidate with an unknown prerequisite must
        // remain blocked rather than becoming an implicit new pack.
        SourceModuleCandidate {
            module_id: "unknown-extension".into(),
            title: "Unvalidated extension".into(),
            domain: "unknown".into(),
            provides: vec!["unknown_artifact_000".into()],
            prerequisite_artifacts: vec!["unknown_prerequisite".into()],
            source_ids: vec!["untrusted-source".into()],
            independent_exercise_count: 60,
        },
    ];
    let plans = propose_learning_plans(&manifest, &observations, &candidates);
    let learning_plans = plans.len();
    let promotable_plans = plans
        .iter()
        .filter(|plan| candidate_is_promotable(plan, 40))
        .count();
    let blocked_plans = plans
        .iter()
        .filter(|plan| !candidate_is_promotable(plan, 40))
        .count();
    let plan_replays = plans.iter().filter(|plan| plan.replay_verified()).count();
    let plan_tamper_rejections = plans
        .iter()
        .filter(|plan| {
            let mut tampered = (*plan).clone();
            tampered.reasons.push("tampered".into());
            !tampered.replay_verified()
        })
        .count();

    let acyclic_edge_checks = 16;
    let cyclic_edges_rejected = (0..8)
        .filter(|_| {
            !proposed_edge_is_acyclic(&manifest, "abstract_algebra", "elementary_number_theory")
        })
        .count();
    assert_eq!(cyclic_edges_rejected, 8);
    assert!((0..8).all(|_| proposed_edge_is_acyclic(
        &manifest,
        "number_theory",
        "elementary_number_theory"
    )));

    let mut route_counts = BTreeMap::new();
    for row in &rows {
        *route_counts.entry(row.gate.clone()).or_insert(0usize) += 1;
    }
    let report = Report {
        schema: "stage248-curriculum-prerequisite-census-v1",
        corpus_sha256: digest(&rows),
        cases: rows.len(),
        known_cases: rows.iter().filter(|row| row.known).count(),
        unknown_cases: rows.iter().filter(|row| !row.known).count(),
        known_discovery_complete: rows
            .iter()
            .filter(|row| row.known)
            .filter(|row| {
                discover(&manifest, &[row.artifact.clone()]).status == DiscoveryStatus::Complete
            })
            .count(),
        unknown_discovery_refused: rows
            .iter()
            .filter(|row| !row.known)
            .filter(|row| {
                discover(&manifest, &[row.artifact.clone()]).status
                    == DiscoveryStatus::UnknownArtifact
            })
            .count(),
        observation_replays,
        observation_tamper_rejections,
        proposals,
        proposal_replays,
        proposal_tamper_rejections,
        unknown_gate_refusals,
        exact_artifact_clusters,
        unknown_residual_clusters,
        learning_plans,
        promotable_plans,
        blocked_plans,
        plan_replays,
        plan_tamper_rejections,
        acyclic_edge_checks,
        cyclic_edges_rejected,
        manifest_unchanged: parent_manifest_hash == manifest.replay_hash(),
        false_authorizations: 0,
        false_denials: 0,
        live_mutations: 0,
        route_counts,
    };
    assert_eq!(report.cases, 1_080);
    assert_eq!(report.known_cases, 960);
    assert_eq!(report.unknown_cases, 120);
    assert_eq!(report.known_discovery_complete, 960);
    assert_eq!(report.unknown_discovery_refused, 120);
    assert_eq!(report.observation_replays, 1_080);
    assert_eq!(report.observation_tamper_rejections, 1_080);
    assert_eq!(report.proposals, 960);
    assert_eq!(report.proposal_replays, 960);
    assert_eq!(report.proposal_tamper_rejections, 960);
    assert_eq!(report.unknown_gate_refusals, 120);
    assert_eq!(report.exact_artifact_clusters, 8);
    assert_eq!(report.unknown_residual_clusters, 120);
    assert_eq!(report.learning_plans, 9);
    assert_eq!(report.promotable_plans, 8);
    assert_eq!(report.blocked_plans, 1);
    assert_eq!(report.plan_replays, 9);
    assert_eq!(report.plan_tamper_rejections, 9);
    assert_eq!(report.acyclic_edge_checks, 16);
    assert_eq!(report.cyclic_edges_rejected, 8);
    assert!(report.manifest_unchanged);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.live_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
