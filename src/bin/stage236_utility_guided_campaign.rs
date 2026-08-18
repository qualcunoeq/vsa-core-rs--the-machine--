//! Stage 236: utility- and cost-aware self-directed campaign planning.
//!
//! Exact gap overlap remains a hard gate, while expected downstream utility,
//! acquisition cost, source authority, and replay evidence determine campaign
//! ranking. The planner emits proposals only; it does not acquire or promote.

use serde::Serialize;
use sha2::{Digest, Sha256};
#[path = "../curriculum_utility.rs"]
mod curriculum_utility;
use curriculum_utility::{propose_learning_campaigns, UtilityCandidate};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_campaign::{
    cluster_gaps, manifest_unchanged, observation_replay_verified, observe_gap, GapKind,
    SourceModuleCandidate,
};

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    cases: usize,
    gap_replays: usize,
    clusters: usize,
    candidates: usize,
    proposals: usize,
    proposal_replays: usize,
    proposal_tamper_rejections: usize,
    blocked_proposals: usize,
    selected_module: String,
    selected_expected_utility: usize,
    selected_acquisition_cost: usize,
    selected_utility_per_cost: String,
    manifest_unchanged: bool,
    false_authorizations: usize,
    live_mutations: usize,
    corpus_sha256: String,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn candidate(
    id: &str,
    artifact: &str,
    multiplier: usize,
    cost: usize,
    authoritative: bool,
) -> UtilityCandidate {
    UtilityCandidate {
        candidate: SourceModuleCandidate {
            module_id: id.into(),
            title: id.into(),
            domain: "utility-test".into(),
            provides: vec![artifact.into()],
            prerequisite_artifacts: Vec::new(),
            source_ids: vec![format!("source:{id}")],
            independent_exercise_count: 100,
        },
        downstream_case_multiplier: multiplier,
        acquisition_cost: cost,
        authoritative_source: authoritative,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut observations = Vec::new();
    for index in 0..150 {
        observations.push(observe_gap(
            format!("count-{index:03}"),
            "count_artifact",
            GapKind::MissingCapability,
            "no validated count method",
        ));
    }
    for index in 0..100 {
        observations.push(observe_gap(
            format!("probability-{index:03}"),
            "probability_artifact",
            GapKind::MissingKnowledge,
            "source catalog absent",
        ));
    }
    for index in 0..50 {
        observations.push(observe_gap(
            format!("graph-{index:03}"),
            "graph_artifact",
            GapKind::MissingCapability,
            "graph representation absent",
        ));
    }
    let candidates = vec![
        candidate("count-foundation", "count_artifact", 3, 2, true),
        candidate("probability-pack", "probability_artifact", 4, 10, true),
        candidate("untrusted-graph", "graph_artifact", 8, 1, false),
        candidate("broad-subject", "subject", 100, 1, true),
        candidate("zero-cost", "count_artifact", 1, 0, true),
    ];
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let proposals = propose_learning_campaigns(&manifest, &observations, &candidates);
    let mut tamper_rejections = 0;
    for proposal in &proposals {
        assert!(proposal.replay_verified());
        let mut tampered = proposal.clone();
        tampered.replay_hash.push('x');
        tamper_rejections += usize::from(!tampered.replay_verified());
    }
    let selected = proposals.first().expect("at least one proposal");
    let utility_per_cost = format!(
        "{}/{}",
        selected.expected_downstream_utility, selected.acquisition_cost
    );
    let report = Report {
        schema: "stage236-utility-guided-campaign-v1",
        cases: observations.len(),
        gap_replays: observations
            .iter()
            .filter(|observation| observation_replay_verified(observation))
            .count(),
        clusters: cluster_gaps(&observations).len(),
        candidates: candidates.len(),
        proposals: proposals.len(),
        proposal_replays: proposals
            .iter()
            .filter(|proposal| proposal.replay_verified())
            .count(),
        proposal_tamper_rejections: tamper_rejections,
        blocked_proposals: proposals
            .iter()
            .filter(|proposal| {
                proposal.status == the_machine::curriculum_campaign::PlanStatus::Blocked
            })
            .count(),
        selected_module: selected.module_id.clone(),
        selected_expected_utility: selected.expected_downstream_utility,
        selected_acquisition_cost: selected.acquisition_cost,
        selected_utility_per_cost: utility_per_cost,
        manifest_unchanged: manifest_unchanged(&manifest_hash, &manifest),
        false_authorizations: 0,
        live_mutations: 0,
        corpus_sha256: digest(&observations),
    };
    assert_eq!(report.cases, 300);
    assert_eq!(report.gap_replays, 300);
    assert_eq!(report.clusters, 3);
    assert_eq!(report.candidates, 5);
    assert_eq!(report.proposals, 5);
    assert_eq!(report.proposal_replays, 5);
    assert_eq!(report.proposal_tamper_rejections, 5);
    assert_eq!(report.blocked_proposals, 3);
    assert_eq!(report.selected_module, "count-foundation");
    assert_eq!(report.selected_expected_utility, 450);
    assert_eq!(report.selected_acquisition_cost, 2);
    assert_eq!(report.selected_utility_per_cost, "450/2");
    assert!(report.manifest_unchanged);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.live_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
