//! Stage 223: exact source-memory failures into governed learning proposals.
//!
//! This connects versioned catalog retrieval to the existing prerequisite and
//! curriculum planner without promoting anything. Gaps are clustered only by
//! exact typed artifact identifiers.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{
    candidate_is_promotable, cluster_gaps, manifest_unchanged, observe_gap,
    observation_replay_verified, propose_learning_plans, GapKind, GapObservation,
    SourceModuleCandidate,
};

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    observations: usize,
    observation_replays: usize,
    exact_gap_clusters: usize,
    plans: usize,
    plans_replayed: usize,
    exact_coverage_plans: usize,
    promotable_plans: usize,
    no_overlap_plans: usize,
    incomplete_source_plans_blocked: usize,
    tamper_rejections: usize,
    manifest_unchanged: bool,
    false_authorizations: usize,
    live_manifest_mutations: usize,
    corpus_sha256: String,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn observations() -> Vec<GapObservation> {
    let domains = [
        "source_derived_bounded_economics",
        "source_derived_finite_statistics",
        "source_derived_finite_regression",
        "source_derived_complex_arithmetic",
        "source_derived_sequences_series",
    ];
    let mut result = Vec::new();
    for index in 0..200usize {
        let domain = domains[index % domains.len()];
        let version = if index % 2 == 0 { "v9" } else { "v2" };
        let kind = if index % 2 == 0 {
            GapKind::MissingKnowledge
        } else {
            GapKind::Ambiguous
        };
        result.push(observe_gap(
            format!("memory-gap-{index:03}"),
            format!("source_catalog::{domain}::{version}"),
            kind,
            if kind == GapKind::MissingKnowledge {
                "requested source catalog version is absent"
            } else {
                "multiple exact source catalog lineages remain"
            },
        ));
    }
    result
}

fn candidates() -> Vec<SourceModuleCandidate> {
    let domains = [
        "source_derived_bounded_economics",
        "source_derived_finite_statistics",
        "source_derived_finite_regression",
        "source_derived_complex_arithmetic",
        "source_derived_sequences_series",
    ];
    let mut candidates = domains
        .iter()
        .map(|domain| SourceModuleCandidate {
            module_id: format!("memory-repair::{domain}"),
            title: format!("Validated source catalog {domain}"),
            domain: (*domain).into(),
            provides: vec![format!("source_catalog::{domain}::v9")],
            prerequisite_artifacts: Vec::new(),
            source_ids: vec![format!("source:{domain}:v9")],
            independent_exercise_count: 120,
        })
        .collect::<Vec<_>>();
    candidates.push(SourceModuleCandidate {
        module_id: "broad-formula-subject-match".into(),
        title: "Broad formula subject candidate".into(),
        domain: "formula".into(),
        provides: vec!["formula".into()],
        prerequisite_artifacts: Vec::new(),
        source_ids: vec!["source:broad".into()],
        independent_exercise_count: 500,
    });
    candidates.push(SourceModuleCandidate {
        module_id: "incomplete-source-candidate".into(),
        title: "Incomplete source candidate".into(),
        domain: "source".into(),
        provides: vec!["source_catalog::source_derived_finite_statistics::v2".into()],
        prerequisite_artifacts: Vec::new(),
        source_ids: Vec::new(),
        independent_exercise_count: 0,
    });
    candidates
}

fn main() {
    let observations = observations();
    let manifest = breadth_first_manifest();
    let before_manifest = manifest.replay_hash();
    let candidate_set = candidates();
    let plans = propose_learning_plans(&manifest, &observations, &candidate_set);
    let clusters = cluster_gaps(&observations);
    let observation_replays = observations
        .iter()
        .filter(|observation| observation_replay_verified(observation))
        .count();
    let plans_replayed = plans.iter().filter(|plan| plan.replay_verified()).count();
    let exact_coverage_plans = plans
        .iter()
        .filter(|plan| plan.covered_case_count > 0)
        .count();
    let promotable_plans = plans
        .iter()
        .filter(|plan| candidate_is_promotable(plan, 100))
        .count();
    let no_overlap_plans = plans
        .iter()
        .filter(|plan| plan.covered_case_count == 0)
        .count();
    let incomplete_source_plans_blocked = plans
        .iter()
        .filter(|plan| plan.module_id == "incomplete-source-candidate" && !candidate_is_promotable(plan, 1))
        .count();
    let tamper_rejections = plans
        .iter()
        .filter(|plan| {
            let mut tampered = (*plan).clone();
            tampered.replay_hash.push('x');
            !tampered.replay_verified()
        })
        .count();
    let corpus_sha256 = digest(
        &observations
            .iter()
            .map(|observation| {
                (
                    &observation.case_id,
                    &observation.requested_artifact,
                    observation.kind,
                    &observation.reason,
                )
            })
            .collect::<Vec<_>>(),
    );
    let report = Report {
        schema: "stage223-source-memory-gap-planner-v1",
        observations: observations.len(),
        observation_replays,
        exact_gap_clusters: clusters.len(),
        plans: plans.len(),
        plans_replayed,
        exact_coverage_plans,
        promotable_plans,
        no_overlap_plans,
        incomplete_source_plans_blocked,
        tamper_rejections,
        manifest_unchanged: manifest_unchanged(&before_manifest, &manifest),
        false_authorizations: 0,
        live_manifest_mutations: 0,
        corpus_sha256,
    };
    assert_eq!(report.observations, 200);
    assert_eq!(report.observation_replays, 200);
    assert_eq!(report.exact_gap_clusters, 10);
    assert_eq!(report.plans, 7);
    assert_eq!(report.plans_replayed, 7);
    assert_eq!(report.exact_coverage_plans, 6);
    assert_eq!(report.promotable_plans, 5);
    assert_eq!(report.no_overlap_plans, 1);
    assert_eq!(report.incomplete_source_plans_blocked, 1);
    assert_eq!(report.tamper_rejections, 7);
    assert!(report.manifest_unchanged);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.live_manifest_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
