//! Governed breadth-first domain curriculum planning.
//!
//! The curriculum is a planning artifact, not a live registry.  It records
//! which externally sourced knowledge packs may be developed, the prerequisites
//! and evidence gates they must satisfy, and the rule that HLE remains a frozen
//! holdout rather than a training corpus.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurriculumStatus {
    Planned,
    SourceAudit,
    ShadowValidated,
    PressureTested,
    HoldoutEvaluated,
    Promotable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationGates {
    pub authoritative_sources: bool,
    pub independent_development_corpus: bool,
    pub boundary_corpus: bool,
    pub pressure_corpus: bool,
    pub replay_verified: bool,
    pub zero_false_authorization: bool,
    pub frozen_hle_holdout: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurriculumPack {
    pub id: String,
    pub title: String,
    pub status: CurriculumStatus,
    pub prerequisites: Vec<String>,
    pub reusable_artifacts: Vec<String>,
    pub source_requirements: Vec<String>,
    pub validation_gates: ValidationGates,
    pub hle_policy: String,
    pub selection_reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurriculumManifest {
    pub schema_version: String,
    pub policy: String,
    pub packs: Vec<CurriculumPack>,
}

impl CurriculumManifest {
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        let ids: BTreeSet<String> = self.packs.iter().map(|pack| pack.id.clone()).collect();
        if ids.len() != self.packs.len() {
            errors.push("duplicate curriculum pack id".into());
        }
        for pack in &self.packs {
            for prerequisite in &pack.prerequisites {
                if !ids.contains(prerequisite) {
                    errors.push(format!(
                        "{} has unknown prerequisite {}",
                        pack.id, prerequisite
                    ));
                }
            }
            if pack.status == CurriculumStatus::Promotable
                && (!pack.validation_gates.authoritative_sources
                    || !pack.validation_gates.independent_development_corpus
                    || !pack.validation_gates.boundary_corpus
                    || !pack.validation_gates.pressure_corpus
                    || !pack.validation_gates.replay_verified
                    || !pack.validation_gates.zero_false_authorization
                    || !pack.validation_gates.frozen_hle_holdout)
            {
                errors.push(format!(
                    "{} is promotable without every validation gate",
                    pack.id
                ));
            }
            if !pack.hle_policy.contains("frozen") {
                errors.push(format!("{} does not declare a frozen HLE policy", pack.id));
            }
        }
        errors.extend(self.cycle_errors());
        errors
    }

    fn cycle_errors(&self) -> Vec<String> {
        fn visit(
            id: &str,
            graph: &BTreeMap<String, Vec<String>>,
            visiting: &mut BTreeSet<String>,
            visited: &mut BTreeSet<String>,
            errors: &mut Vec<String>,
        ) {
            if visited.contains(id) {
                return;
            }
            if !visiting.insert(id.to_string()) {
                errors.push(format!("curriculum prerequisite cycle at {id}"));
                return;
            }
            for prerequisite in graph.get(id).into_iter().flatten() {
                visit(prerequisite, graph, visiting, visited, errors);
            }
            visiting.remove(id);
            visited.insert(id.to_string());
        }
        let graph: BTreeMap<String, Vec<String>> = self
            .packs
            .iter()
            .map(|pack| (pack.id.clone(), pack.prerequisites.clone()))
            .collect();
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        let mut errors = Vec::new();
        for id in graph.keys() {
            visit(id, &graph, &mut visiting, &mut visited, &mut errors);
        }
        errors
    }

    pub fn replay_hash(&self) -> String {
        let bytes = serde_json::to_vec(self).expect("curriculum serializes");
        format!("{:x}", Sha256::digest(bytes))
    }
}

fn gates() -> ValidationGates {
    ValidationGates {
        authoritative_sources: false,
        independent_development_corpus: false,
        boundary_corpus: false,
        pressure_corpus: false,
        replay_verified: false,
        zero_false_authorization: false,
        frozen_hle_holdout: true,
    }
}

/// The breadth-first curriculum deliberately stops at planning.  No pack is
/// registered, promoted, or allowed to alter production routing.
pub fn breadth_first_manifest() -> CurriculumManifest {
    let planned = gates();
    let mut packs = vec![CurriculumPack {
        id: "classical_mechanics".into(),
        title: "Classical mechanics foundations".into(),
        status: CurriculumStatus::ShadowValidated,
        prerequisites: Vec::new(),
        reusable_artifacts: vec!["typed_physical_law".into(), "unit_checked_equation".into()],
        source_requirements: vec!["authoritative textbook or primary source".into()],
        validation_gates: ValidationGates {
            authoritative_sources: true,
            independent_development_corpus: true,
            boundary_corpus: true,
            pressure_corpus: true,
            replay_verified: true,
            zero_false_authorization: true,
            frozen_hle_holdout: true,
        },
        hle_policy: "HLE remains a frozen diagnostic holdout; never development data".into(),
        selection_reason: "existing externally validated pack retained as substrate evidence"
            .into(),
    }];
    let domains = [
        (
            "linear_algebra_spectral",
            "Linear algebra and spectral theory",
            vec![],
            vec!["matrix_artifact", "linear_map", "spectrum"],
            "first planned pack; maximizes bridge reuse",
        ),
        (
            "probability_stochastic",
            "Probability and stochastic processes",
            vec!["linear_algebra_spectral"],
            vec!["random_variable", "distribution", "expectation"],
            "build after typed linear objects and transformations",
        ),
        (
            "real_complex_analysis",
            "Real and complex analysis",
            vec!["linear_algebra_spectral"],
            vec!["limit", "series", "analytic_function"],
            "requires explicit domain and convergence artifacts",
        ),
        (
            "graph_theory",
            "Graph theory and spectral inequalities",
            vec!["linear_algebra_spectral"],
            vec!["finite_graph", "cut", "graph_spectrum"],
            "supports reusable combinatorial and spectral bridges",
        ),
        (
            "abstract_algebra",
            "Abstract algebra",
            vec!["linear_algebra_spectral"],
            vec!["group", "ring", "homomorphism"],
            "requires typed algebraic structures rather than labels",
        ),
        (
            "topology",
            "Topology and geometric invariants",
            vec!["abstract_algebra"],
            vec!["topological_space", "invariant", "homology"],
            "defer until algebraic structure and theorem provenance exist",
        ),
        (
            "number_theory",
            "Number theory",
            vec!["real_complex_analysis", "abstract_algebra"],
            vec!["integer_relation", "character", "asymptotic_count"],
            "defer until asymptotic and algebraic prerequisites are validated",
        ),
    ];
    for (id, title, prerequisites, artifacts, reason) in domains {
        let (status, validation_gates) = if id == "linear_algebra_spectral" {
            (
                CurriculumStatus::ShadowValidated,
                ValidationGates {
                    authoritative_sources: true,
                    independent_development_corpus: true,
                    boundary_corpus: true,
                    pressure_corpus: true,
                    replay_verified: true,
                    zero_false_authorization: true,
                    frozen_hle_holdout: true,
                },
            )
        } else {
            (CurriculumStatus::Planned, planned.clone())
        };
        packs.push(CurriculumPack {
            id: id.into(),
            title: title.into(),
            status,
            prerequisites: prerequisites.into_iter().map(String::from).collect(),
            reusable_artifacts: artifacts.into_iter().map(String::from).collect(),
            source_requirements: vec![
                "independently selected authoritative sources".into(),
                "explicit assumptions, validity domains, and notation".into(),
            ],
            validation_gates,
            hle_policy: "HLE remains a frozen diagnostic holdout; never development data".into(),
            selection_reason: reason.into(),
        });
    }
    CurriculumManifest {
        schema_version: "breadth-first-curriculum-v1".into(),
        policy: "shadow planning only; source education, promotion, and routing require later governed gates".into(),
        packs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_curriculum_is_acyclic_and_holdout_safe() {
        let manifest = breadth_first_manifest();
        assert!(manifest.validate().is_empty());
        assert_eq!(manifest.packs.len(), 8);
        assert!(manifest
            .packs
            .iter()
            .all(|pack| pack.hle_policy.contains("frozen")));
    }

    #[test]
    fn incomplete_pack_cannot_be_promotable() {
        let mut manifest = breadth_first_manifest();
        manifest
            .packs
            .iter_mut()
            .find(|pack| pack.status == CurriculumStatus::Planned)
            .expect("planned pack")
            .status = CurriculumStatus::Promotable;
        assert!(manifest
            .validate()
            .iter()
            .any(|error| error.contains("promotable without every validation gate")));
    }

    #[test]
    fn replay_hash_is_deterministic() {
        let manifest = breadth_first_manifest();
        assert_eq!(manifest.replay_hash(), manifest.replay_hash());
    }
}
