//! Stage G self-directed acquisition of the source-derived topology module.
//!
//! The planner sees only exact typed gap observations and candidate source
//! metadata. It selects the topology source module, validates it in a shadow
//! copy, and leaves the immutable curriculum manifest untouched.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{
    candidate_is_promotable, cluster_gaps, manifest_unchanged, observe_gap, propose_learning_plans,
    GapKind, SourceModuleCandidate,
};
use the_machine::source_topology_pack::{
    evaluate_topology, extract_topology_definitions, TopologyOperation, TopologyRequest,
};

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    observed_cases: usize,
    gap_clusters: usize,
    candidate_plans: usize,
    selected_module: String,
    selected_coverage: usize,
    plan_replay: bool,
    plan_tamper_rejected: bool,
    source_records: usize,
    source_mutations: usize,
    source_mutations_rejected: usize,
    independent_cases: usize,
    independent_correct: usize,
    independent_replay: usize,
    independent_tamper: usize,
    shadow_promotable: bool,
    blocked_shortcuts: usize,
    false_authorizations: usize,
    production_authorizations: usize,
    manifest_unchanged: bool,
    corpus_sha256: String,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn topology_request(index: usize) -> TopologyRequest {
    let points = vec!["a".into(), "b".into(), "c".into()];
    let open_sets = if index % 2 == 0 {
        vec![Vec::new(), points.clone()]
    } else {
        vec![Vec::new(), vec!["a".into()], points.clone()]
    };
    TopologyRequest {
        operation: match index % 5 {
            0 => TopologyOperation::ValidateTopology,
            1 => TopologyOperation::IsOpen,
            2 => TopologyOperation::IsClosed,
            3 => TopologyOperation::Interior,
            _ => TopologyOperation::Closure,
        },
        topology: "finite_topology_axioms".into(),
        points,
        open_sets,
        target_set: if index % 5 == 0 {
            None
        } else {
            Some(vec!["a".into(), "b".into()])
        },
        domain: "source_derived_finite_topology".into(),
        ambiguity: None,
        provenance: vec!["stage-g-self-directed-topology".into()],
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    let before = manifest.replay_hash();
    let mut observations = Vec::with_capacity(500);
    for index in 0..300 {
        observations.push(observe_gap(
            format!("topology_gap_{index:03}"),
            "finite_topology",
            GapKind::MissingCapability,
            "finite topology artifacts are absent from the current route",
        ));
    }
    for index in 0..120 {
        observations.push(observe_gap(
            format!("open_set_gap_{index:03}"),
            "open_set",
            GapKind::MissingCapability,
            "open-set classification is required",
        ));
    }
    for index in 0..80 {
        observations.push(observe_gap(
            format!("matrix_gap_{index:03}"),
            "matrix_artifact",
            GapKind::MissingCapability,
            "matrix artifact is already covered by another pack",
        ));
    }
    let candidates = vec![
        SourceModuleCandidate {
            module_id: "source_derived_finite_topology_candidate".into(),
            title: "Source-derived finite topology definition and operations".into(),
            domain: "topology.finite".into(),
            provides: vec![
                "finite_topology".into(),
                "open_set".into(),
                "closed_set".into(),
                "interior".into(),
                "closure".into(),
            ],
            prerequisite_artifacts: vec!["group".into()],
            source_ids: vec!["topology-without-tears:definition-1.3.1".into()],
            independent_exercise_count: 240,
        },
        SourceModuleCandidate {
            module_id: "existing_linear_algebra_candidate".into(),
            title: "Existing linear algebra substrate".into(),
            domain: "linear_algebra".into(),
            provides: vec!["matrix_artifact".into()],
            prerequisite_artifacts: vec![],
            source_ids: vec!["existing-validated-curriculum".into()],
            independent_exercise_count: 240,
        },
        SourceModuleCandidate {
            module_id: "unproven_topology_shortcut".into(),
            title: "Lexical topology shortcut without source evidence".into(),
            domain: "topology.finite".into(),
            provides: vec!["finite_topology".into(), "open_set".into()],
            prerequisite_artifacts: vec!["group".into()],
            source_ids: Vec::new(),
            independent_exercise_count: 0,
        },
    ];
    let plans = propose_learning_plans(&manifest, &observations, &candidates);
    let selected = &plans[0];
    assert_eq!(
        selected.module_id,
        "source_derived_finite_topology_candidate"
    );
    assert_eq!(selected.covered_case_count, 420);
    let plan_replay = selected.replay_verified();
    let mut tampered_plan = selected.clone();
    tampered_plan.covered_case_count += 1;
    let plan_tamper_rejected = !tampered_plan.replay_verified();

    let document = include_str!("../../docs/sources/topology_without_tears_finite_definition.txt");
    let records = extract_topology_definitions(document).unwrap();
    let mutations = vec![
        document.replace("TOPOLOGY_ID: finite_topology_axioms", "TOPOLOGY_ID: "),
        document.replace("URL: https://", "URL: http://"),
        document.replace(
            "AXIOMS: empty;whole;unions;finite_intersections",
            "AXIOMS: empty;whole",
        ),
        document.replace("MAX_POINTS: 8", "MAX_POINTS: 0"),
        document.replace("END TOPOLOGY", "BEGIN TOPOLOGY"),
        document.replace(
            "ALIASES: finite topology|topological space",
            "ALIASES: duplicate|duplicate",
        ),
    ];
    let mutations_rejected = mutations
        .iter()
        .filter(|mutation| extract_topology_definitions(mutation).is_err())
        .count();
    let mut independent_correct = 0;
    let mut independent_replay = 0;
    let mut independent_tamper = 0;
    for index in 0..120 {
        let result = evaluate_topology(&topology_request(index), &records);
        let complete = result.authorized();
        independent_correct += usize::from(complete);
        independent_replay += usize::from(result.replay_verified());
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        independent_tamper += usize::from(!tampered.replay_verified());
    }
    let blocked_shortcuts = plans
        .iter()
        .filter(|plan| plan.status == the_machine::curriculum_campaign::PlanStatus::Blocked)
        .count();
    let false_authorizations = plans
        .iter()
        .filter(|plan| {
            plan.status == the_machine::curriculum_campaign::PlanStatus::Proposed
                && (plan.source_ids.is_empty() || plan.independent_exercise_count == 0)
        })
        .count();
    let shadow_promotable = candidate_is_promotable(selected, 120)
        && independent_correct == 120
        && mutations_rejected == 6;
    assert_eq!(records.len(), 1);
    assert_eq!(mutations_rejected, 6);
    assert_eq!(independent_correct, 120);
    assert_eq!(independent_replay, 120);
    assert_eq!(independent_tamper, 120);
    assert_eq!(blocked_shortcuts, 1);
    assert_eq!(false_authorizations, 0);
    assert!(shadow_promotable);
    assert!(plan_replay);
    assert!(plan_tamper_rejected);
    assert!(manifest_unchanged(&before, &manifest));
    let report = Report {
        schema: "stage-g-self-directed-topology-acquisition-v1",
        observed_cases: observations.len(),
        gap_clusters: cluster_gaps(&observations).len(),
        candidate_plans: plans.len(),
        selected_module: selected.module_id.clone(),
        selected_coverage: selected.covered_case_count,
        plan_replay,
        plan_tamper_rejected,
        source_records: records.len(),
        source_mutations: mutations.len(),
        source_mutations_rejected: mutations_rejected,
        independent_cases: 120,
        independent_correct,
        independent_replay,
        independent_tamper,
        shadow_promotable,
        blocked_shortcuts,
        false_authorizations,
        production_authorizations: 0,
        manifest_unchanged: manifest_unchanged(&before, &manifest),
        corpus_sha256: hash(&(observations, candidates, plans)),
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage-g-self-directed-topology-acquisition.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
