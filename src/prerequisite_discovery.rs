//! Shadow prerequisite discovery over the governed curriculum DAG.
//!
//! The discoverer maps required typed artifacts to owning curriculum packs,
//! computes prerequisite closure, and rejects unknown artifacts or cyclic
//! candidate edges. It proposes plans only; it never mutates the manifest.

use crate::curriculum::{CurriculumManifest, CurriculumPack};
use serde::{Deserialize, Serialize};
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
