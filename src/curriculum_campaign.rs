//! Shadow self-directed curriculum campaign planning.
//!
//! A campaign consumes failure observations, clusters them by requested typed
//! artifact, and ranks externally sourced modules by expected coverage and
//! prerequisite cost. It proposes immutable learning plans; it never mutates
//! the curriculum manifest or authorizes a capability.

use crate::curriculum::CurriculumManifest;
use crate::prerequisite_discovery::{discover, DiscoveryStatus};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GapKind {
    MissingCapability,
    MissingKnowledge,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GapObservation {
    pub case_id: String,
    pub requested_artifact: String,
    pub kind: GapKind,
    pub reason: String,
    pub replay_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GapCluster {
    pub artifact: String,
    pub case_ids: Vec<String>,
    pub kinds: Vec<GapKind>,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceModuleCandidate {
    pub module_id: String,
    pub title: String,
    pub domain: String,
    pub provides: Vec<String>,
    pub prerequisite_artifacts: Vec<String>,
    pub source_ids: Vec<String>,
    pub independent_exercise_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlanStatus {
    Proposed,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LearningPlan {
    pub module_id: String,
    pub status: PlanStatus,
    pub covered_artifacts: Vec<String>,
    pub covered_case_count: usize,
    pub prerequisite_packs: Vec<String>,
    pub source_ids: Vec<String>,
    pub independent_exercise_count: usize,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("campaign serializes"))
    )
}

fn observation_hash(observation: &GapObservation) -> String {
    digest(&(
        &observation.case_id,
        &observation.requested_artifact,
        observation.kind,
        &observation.reason,
    ))
}

/// Construct a tamper-evident gap observation from a diagnostic event.
pub fn observe_gap(
    case_id: impl Into<String>,
    requested_artifact: impl Into<String>,
    kind: GapKind,
    reason: impl Into<String>,
) -> GapObservation {
    let mut observation = GapObservation {
        case_id: case_id.into(),
        requested_artifact: requested_artifact.into(),
        kind,
        reason: reason.into(),
        replay_hash: String::new(),
    };
    observation.replay_hash = observation_hash(&observation);
    observation
}

pub fn observation_replay_verified(observation: &GapObservation) -> bool {
    observation.replay_hash == observation_hash(observation)
}

/// Cluster failures only by exact requested artifact, never by broad subject
/// labels or lexical similarity.
pub fn cluster_gaps(observations: &[GapObservation]) -> Vec<GapCluster> {
    let mut grouped: BTreeMap<String, (Vec<String>, Vec<GapKind>)> = BTreeMap::new();
    for observation in observations {
        if !observation_replay_verified(observation) {
            continue;
        }
        let entry = grouped
            .entry(observation.requested_artifact.clone())
            .or_default();
        entry.0.push(observation.case_id.clone());
        if !entry.1.contains(&observation.kind) {
            entry.1.push(observation.kind);
        }
    }
    grouped
        .into_iter()
        .map(|(artifact, (case_ids, kinds))| GapCluster {
            count: case_ids.len(),
            artifact,
            case_ids,
            kinds,
        })
        .collect()
}

fn prerequisite_closure(
    manifest: &CurriculumManifest,
    artifacts: &[String],
) -> Result<Vec<String>, String> {
    let result = discover(manifest, artifacts);
    if result.status != DiscoveryStatus::Complete {
        return Err(format!(
            "prerequisite discovery failed: {:?}",
            result.status
        ));
    }
    Ok(result.packs)
}

fn plan_hash(plan: &LearningPlan) -> String {
    digest(&(
        &plan.module_id,
        &plan.status,
        &plan.covered_artifacts,
        plan.covered_case_count,
        &plan.prerequisite_packs,
        &plan.source_ids,
        plan.independent_exercise_count,
        &plan.reasons,
    ))
}

impl LearningPlan {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == plan_hash(self)
    }
}

/// Rank source modules by exact gap coverage, then exercise evidence, then
/// acquisition cost. The returned plans are proposals only.
pub fn propose_learning_plans(
    manifest: &CurriculumManifest,
    observations: &[GapObservation],
    candidates: &[SourceModuleCandidate],
) -> Vec<LearningPlan> {
    let clusters = cluster_gaps(observations);
    let counts: BTreeMap<&str, usize> = clusters
        .iter()
        .map(|cluster| (cluster.artifact.as_str(), cluster.count))
        .collect();
    let mut plans = Vec::new();
    for candidate in candidates {
        let covered_artifacts: Vec<String> = candidate
            .provides
            .iter()
            .filter(|artifact| counts.contains_key(artifact.as_str()))
            .cloned()
            .collect();
        let covered_case_count = covered_artifacts
            .iter()
            .map(|artifact| counts[artifact.as_str()])
            .sum();
        let mut reasons = Vec::new();
        let (mut status, prerequisite_packs) =
            match prerequisite_closure(manifest, &candidate.prerequisite_artifacts) {
                Ok(packs) => {
                    reasons.push(
                        "all declared prerequisites are present in the immutable manifest".into(),
                    );
                    (PlanStatus::Proposed, packs)
                }
                Err(reason) => (PlanStatus::Blocked, vec![reason]),
            };
        if covered_case_count == 0 {
            reasons.push("candidate has no exact artifact overlap with observed gaps".into());
        } else {
            reasons.push(format!(
                "exactly covers {covered_case_count} observed cases"
            ));
        }
        if candidate.source_ids.is_empty() || candidate.independent_exercise_count == 0 {
            reasons
                .push("source provenance and independent exercise evidence are incomplete".into());
            status = PlanStatus::Blocked;
        }
        let mut plan = LearningPlan {
            module_id: candidate.module_id.clone(),
            status,
            covered_artifacts,
            covered_case_count,
            prerequisite_packs,
            source_ids: candidate.source_ids.clone(),
            independent_exercise_count: candidate.independent_exercise_count,
            reasons,
            replay_hash: String::new(),
        };
        plan.replay_hash = plan_hash(&plan);
        plans.push(plan);
    }
    plans.sort_by(|left, right| {
        right
            .covered_case_count
            .cmp(&left.covered_case_count)
            .then_with(|| {
                right
                    .independent_exercise_count
                    .cmp(&left.independent_exercise_count)
            })
            .then_with(|| left.module_id.cmp(&right.module_id))
    });
    plans
}

/// Verify that a plan remains proposal-only and has not altered the manifest.
pub fn manifest_unchanged(before: &str, manifest: &CurriculumManifest) -> bool {
    before == &manifest.replay_hash()
}

/// Check a candidate source module without promoting it.
pub fn candidate_is_promotable(plan: &LearningPlan, minimum_exercises: usize) -> bool {
    plan.status == PlanStatus::Proposed
        && plan.covered_case_count > 0
        && !plan.source_ids.is_empty()
        && plan.independent_exercise_count >= minimum_exercises
        && plan.replay_verified()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curriculum::breadth_first_manifest;

    #[test]
    fn exact_gap_clustering_and_plan_are_replayable() {
        let observations = vec![
            observe_gap("a", "mean", GapKind::MissingCapability, "no pack"),
            observe_gap("b", "mean", GapKind::MissingKnowledge, "source absent"),
        ];
        let candidate = SourceModuleCandidate {
            module_id: "stats".into(),
            title: "Finite statistics".into(),
            domain: "statistics".into(),
            provides: vec!["mean".into()],
            prerequisite_artifacts: vec!["distribution".into()],
            source_ids: vec!["source".into()],
            independent_exercise_count: 20,
        };
        let plans = propose_learning_plans(&breadth_first_manifest(), &observations, &[candidate]);
        assert_eq!(plans[0].covered_case_count, 2);
        assert!(candidate_is_promotable(&plans[0], 10));
    }

    #[test]
    fn tampered_gap_is_excluded() {
        let mut observation = observe_gap("a", "mean", GapKind::MissingCapability, "no pack");
        observation.reason = "tampered".into();
        assert!(cluster_gaps(&[observation]).is_empty());
    }
}
