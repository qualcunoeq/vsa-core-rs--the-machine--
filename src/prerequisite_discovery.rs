//! Shadow prerequisite discovery over the governed curriculum DAG.
//!
//! The discoverer maps required typed artifacts to owning curriculum packs,
//! computes prerequisite closure, and rejects unknown artifacts or cyclic
//! candidate edges. It proposes plans only; it never mutates the manifest.

use crate::curriculum::{CurriculumManifest, CurriculumPack};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DiscoveryStatus {
    Complete,
    UnknownArtifact,
    CycleRejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveryResult {
    pub status: DiscoveryStatus,
    pub artifacts: Vec<String>,
    pub packs: Vec<String>,
    pub missing_prerequisites: Vec<String>,
    pub reasons: Vec<String>,
}

/// Classification of a bounded failure that can be carried into curriculum
/// planning.  A proposal is diagnostic only; it never authorizes execution or
/// mutates the curriculum manifest.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityGapStatus {
    MissingPrerequisite,
    AmbiguousBoundary,
    UnsupportedBoundary,
}

/// A replayable proposal describing what an observed failure appears to need.
/// The fields intentionally separate method, knowledge, and representation so
/// a planner cannot hide a missing semantic bridge inside a vague method name.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityGap {
    pub failure_gate: String,
    pub status: CapabilityGapStatus,
    pub desired_transformation: String,
    pub missing_prerequisite: String,
    pub nearest_available_capability: String,
    pub external_knowledge_needed: String,
    pub representation_needed: String,
    pub suggested_dependency: String,
    pub triggering_case_ids: Vec<String>,
    pub replay_hash: String,
}

fn capability_gap_hash(gap: &CapabilityGap) -> String {
    let payload = (
        &gap.failure_gate,
        gap.status,
        &gap.desired_transformation,
        &gap.missing_prerequisite,
        &gap.nearest_available_capability,
        &gap.external_knowledge_needed,
        &gap.representation_needed,
        &gap.suggested_dependency,
        &gap.triggering_case_ids,
    );
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&payload).unwrap())
    )
}

/// Convert an exact failure gate into a bounded diagnostic proposal.
/// Unknown gates are rejected instead of being assigned a broad catch-all
/// capability, preserving the semantic-coherence rule.
pub fn propose_capability_gap(
    failure_gate: &str,
    status: CapabilityGapStatus,
    triggering_case_ids: Vec<String>,
) -> Option<CapabilityGap> {
    let (desired, prerequisite, nearest, knowledge, representation, dependency) = match failure_gate
    {
        "combinatorics" => (
            "typed finite count to downstream arithmetic or dynamics",
            "explicit counting-model and operand scope",
            "combinatorics_frontend",
            "finite counting identities and labeled/unlabeled conventions",
            "exact bounded scalar count",
            "combinatorics",
        ),
        "graph" => (
            "typed finite graph to adjacency and stochastic evolution",
            "stable vertex identity, ordering, and edge semantics",
            "graph_pack",
            "finite graph and transition-convention definitions",
            "vertex-ordered adjacency matrix",
            "graph_theory",
        ),
        "probability" => (
            "finite distribution to exact expectation and algebraic representation",
            "normalized probabilities and value binding",
            "finite_probability_pack",
            "finite random-variable and expectation definitions",
            "rational probability vector with outcome ordering",
            "finite_probability",
        ),
        "ode" => (
            "continuous-time scalar equation to exact solution and derivative",
            "ODE form, initial condition, and time-domain semantics",
            "ode_pack",
            "bounded exact ODE solution contract",
            "typed scalar ODE artifact",
            "ordinary_differential_equations",
        ),
        "dynamics" => (
            "finite state update to bounded replayable trajectory",
            "explicit transition, horizon, and state representation",
            "discrete_dynamics",
            "finite-horizon recurrence semantics",
            "typed state trace",
            "discrete_dynamics",
        ),
        "stationary_graph_boundary" => (
            "typed graph plus row-stochastic transition to stationary distribution",
            "stable vertex identity, state ordering, and explicit transition semantics",
            "finite_markov_stationary_general",
            "finite stationary-distribution definitions and uniqueness conditions",
            "vertex-ordered graph and exact stationary request",
            "finite_markov_stationary_general",
        ),
        "hitting_graph_boundary" => (
            "typed graph plus target/avoid transition to hitting probability",
            "explicit target, avoid, initial distribution, and transition support",
            "finite_markov_hitting",
            "finite target-before-avoid semantics and transient-state equations",
            "vertex-ordered graph and exact hitting request",
            "finite_markov_hitting",
        ),
        "frontend_missing_required_field" => (
            "technical text to a complete finite Markov request",
            "explicit operation, transition convention, and required state bindings",
            "finite_markov_frontend",
            "bounded stationary and hitting problem forms",
            "replayable typed Markov request",
            "finite_markov",
        ),
        "frontend_ambiguity" => (
            "technical text to a uniquely identified Markov operation",
            "operation and row/column convention evidence",
            "finite_markov_frontend",
            "stationary versus hitting interpretation boundaries",
            "alternative typed requests with provenance",
            "finite_markov",
        ),
        _ => return None,
    };
    let mut gap = CapabilityGap {
        failure_gate: failure_gate.into(),
        status,
        desired_transformation: desired.into(),
        missing_prerequisite: prerequisite.into(),
        nearest_available_capability: nearest.into(),
        external_knowledge_needed: knowledge.into(),
        representation_needed: representation.into(),
        suggested_dependency: dependency.into(),
        triggering_case_ids,
        replay_hash: String::new(),
    };
    gap.replay_hash = capability_gap_hash(&gap);
    Some(gap)
}

/// Verify a proposal independently of its source failure record.
pub fn capability_gap_replay_verified(gap: &CapabilityGap) -> bool {
    gap.replay_hash == capability_gap_hash(gap)
        && !gap.triggering_case_ids.is_empty()
        && !gap.missing_prerequisite.is_empty()
}

fn pack_map(manifest: &CurriculumManifest) -> BTreeMap<String, &CurriculumPack> {
    manifest
        .packs
        .iter()
        .map(|pack| (pack.id.clone(), pack))
        .collect()
}

fn artifact_map(manifest: &CurriculumManifest) -> BTreeMap<String, String> {
    manifest
        .packs
        .iter()
        .flat_map(|pack| {
            pack.reusable_artifacts
                .iter()
                .map(|artifact| (artifact.clone(), pack.id.clone()))
        })
        .collect()
}

/// Compute the transitive prerequisite plan for required typed artifacts.
pub fn discover(manifest: &CurriculumManifest, artifacts: &[String]) -> DiscoveryResult {
    let packs = pack_map(manifest);
    let owners = artifact_map(manifest);
    let mut required = BTreeSet::new();
    let mut missing = Vec::new();
    for artifact in artifacts {
        if let Some(owner) = owners.get(artifact) {
            required.insert(owner.clone());
        } else {
            missing.push(artifact.clone());
        }
    }
    if !missing.is_empty() {
        return DiscoveryResult {
            status: DiscoveryStatus::UnknownArtifact,
            artifacts: artifacts.to_vec(),
            packs: required.into_iter().collect(),
            missing_prerequisites: missing,
            reasons: vec!["artifact has no governed curriculum owner".into()],
        };
    }
    let mut closure = required.clone();
    let mut queue: VecDeque<String> = required.into_iter().collect();
    while let Some(pack_id) = queue.pop_front() {
        let Some(pack) = packs.get(&pack_id) else {
            return DiscoveryResult {
                status: DiscoveryStatus::CycleRejected,
                artifacts: artifacts.to_vec(),
                packs: closure.into_iter().collect(),
                missing_prerequisites: Vec::new(),
                reasons: vec![format!("pack {pack_id} is not present in the manifest")],
            };
        };
        for prerequisite in &pack.prerequisites {
            if !closure.insert(prerequisite.clone()) {
                continue;
            }
            queue.push_back(prerequisite.clone());
        }
    }
    let cycle = manifest
        .validate()
        .iter()
        .any(|error| error.contains("cycle"));
    DiscoveryResult {
        status: if cycle {
            DiscoveryStatus::CycleRejected
        } else {
            DiscoveryStatus::Complete
        },
        artifacts: artifacts.to_vec(),
        packs: closure.into_iter().collect(),
        missing_prerequisites: Vec::new(),
        reasons: if cycle {
            vec!["manifest contains a prerequisite cycle".into()]
        } else {
            Vec::new()
        },
    }
}

/// Check a proposed edge without changing the source manifest.
pub fn proposed_edge_is_acyclic(
    manifest: &CurriculumManifest,
    dependent: &str,
    prerequisite: &str,
) -> bool {
    let candidate = CurriculumManifest {
        schema_version: manifest.schema_version.clone(),
        policy: manifest.policy.clone(),
        packs: manifest
            .packs
            .iter()
            .map(|pack| {
                let mut clone = pack.clone();
                if clone.id == dependent {
                    clone.prerequisites.push(prerequisite.into());
                }
                clone
            })
            .collect(),
    };
    !candidate
        .validate()
        .iter()
        .any(|error| error.contains("cycle"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_gap_proposals_are_typed_and_tamper_evident() {
        let gap = propose_capability_gap(
            "graph",
            CapabilityGapStatus::MissingPrerequisite,
            vec!["case-1".into(), "case-2".into()],
        )
        .expect("known gate has a bounded proposal");
        assert_eq!(gap.suggested_dependency, "graph_theory");
        assert!(capability_gap_replay_verified(&gap));
        let mut tampered = gap.clone();
        tampered.representation_needed.push_str("-tampered");
        assert!(!capability_gap_replay_verified(&tampered));
        assert!(propose_capability_gap(
            "unknown_gate",
            CapabilityGapStatus::MissingPrerequisite,
            vec!["case-3".into()]
        )
        .is_none());
        let markov = propose_capability_gap(
            "hitting_graph_boundary",
            CapabilityGapStatus::MissingPrerequisite,
            vec!["case-markov".into()],
        )
        .expect("Markov boundary has a bounded proposal");
        assert!(markov.desired_transformation.contains("hitting"));
        assert!(capability_gap_replay_verified(&markov));
    }
}
