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
            "finite_markov",
            "Bounded finite Markov chains",
            vec!["probability_stochastic", "linear_algebra_spectral"],
            vec![
                "row_stochastic_transition",
                "finite_horizon_trace",
                "two_state_stationary",
            ],
            "extend finite probability with explicit exact transition semantics",
        ),
        (
            "finite_markov_stationary_general",
            "Bounded exact finite stationary distributions",
            vec!["finite_markov", "linear_algebra_spectral"],
            vec![
                "stationary_distribution_up_to_four_states",
                "exact_rank_uniqueness",
                "stationary_residual_certificate",
            ],
            "separate exact linear-system extension; the historical two-state Markov contract remains immutable",
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
            "elementary_number_theory",
            "Elementary number theory",
            vec!["abstract_algebra"],
            vec!["gcd_bezout", "congruence_class", "crt_class", "totient"],
            "reuse finite modular and cyclic artifacts before advanced number theory",
        ),
        (
            "combinatorics",
            "Bounded combinatorics",
            vec![
                "probability_stochastic",
                "graph_theory",
                "elementary_number_theory",
            ],
            vec![
                "permutation_count",
                "combination_count",
                "multinomial_count",
                "inclusion_exclusion_count",
                "surjection_count",
            ],
            "finite exact counting substrate for probability, graph, and arithmetic composition",
        ),
        (
            "ordinary_differential_equations",
            "Bounded ordinary differential equations",
            vec!["linear_algebra_spectral", "real_complex_analysis"],
            vec![
                "exact_constant_derivative",
                "affine_linear_solution",
                "ode_trace",
            ],
            "bounded continuous-time substrate after exact calculus and theorem contracts",
        ),
        (
            "bounded_calculus",
            "Bounded exact one-variable calculus",
            vec!["real_complex_analysis"],
            vec!["derivative", "integral", "limit"],
            "exact symbolic continuous operations under explicit domain contracts",
        ),
        (
            "source_formula_sequences",
            "Source-derived sequences and series",
            vec!["combinatorics", "bounded_calculus"],
            vec![
                "arithmetic_nth_term",
                "arithmetic_partial_sum",
                "geometric_nth_term",
                "geometric_partial_sum",
            ],
            "declarative source records executed by a generic expression interpreter",
        ),
        (
            "source_derived_science",
            "Source-derived bounded classical science laws",
            vec![
                "classical_mechanics",
                "ordinary_differential_equations",
                "bounded_calculus",
            ],
            vec![
                "ideal_gas_pressure",
                "first_law_delta_u",
                "kinetic_energy",
                "hooke_force",
            ],
            "source-cited law records with generic exact rational execution",
        ),
        (
            "source_derived_chemistry",
            "Source-derived bounded chemistry",
            vec!["source_derived_science"],
            vec![
                "molecular_formula",
                "balanced_reaction",
                "stoichiometric_ratio",
                "element_count_vector",
            ],
            "first source-derived domain with molecular and reaction representations",
        ),
        (
            "source_derived_biology",
            "Source-derived bounded molecular biology",
            vec!["source_derived_chemistry"],
            vec!["dna_sequence", "complementary_pair", "base_composition"],
            "first source-derived biology domain with explicit nucleotide representations",
        ),
        (
            "source_derived_finite_statistics",
            "Source-derived finite statistics",
            vec!["probability_stochastic", "source_formula_sequences"],
            vec![
                "arithmetic_mean",
                "weighted_mean",
                "bernoulli_variance",
                "binomial_expected_value",
                "binomial_variance",
            ],
            "first new source-derived domain executed by the generic formula catalog runtime",
        ),
        (
            "source_derived_finite_regression",
            "Source-derived finite regression diagnostics",
            vec!["source_derived_finite_statistics"],
            vec![
                "regression_slope",
                "regression_intercept",
                "regression_fitted_value",
                "regression_residual",
                "regression_r_squared",
            ],
            "source-derived regression relations executed by the generic formula catalog runtime",
        ),
        (
            "source_derived_linear_interpolation",
            "Source-derived bounded linear interpolation",
            vec!["bounded_calculus"],
            vec!["linear_interpolation", "bounded_affine_formula"],
            "first source-derived interpolation domain validated on an untouched transfer partition",
        ),
        (
            "source_derived_bayes_rule",
            "Source-derived bounded Bayes rule",
            vec!["probability_stochastic"],
            vec!["prior_probability", "likelihood", "evidence", "posterior_probability"],
            "second independent source-derived domain with an exact finite-probability bridge",
        ),
        (
            "source_derived_finite_set_operations",
            "Source-derived bounded finite-set operations",
            vec!["combinatorics", "probability_stochastic"],
            vec![
                "finite_set",
                "set_union",
                "set_intersection",
                "set_difference",
                "set_complement",
                "set_cardinality",
            ],
            "OpenStax-derived finite set semantics with explicit universe and deterministic operations",
        ),
        (
            "source_derived_bounded_counting",
            "Source-derived bounded counting principles",
            vec!["combinatorics", "source_derived_finite_set_operations"],
            vec![
                "exact_product_count",
                "factorial",
                "ordered_permutation_count",
                "unordered_combination_count",
            ],
            "OpenStax-derived distinction between ordered and unordered finite selection",
        ),
        (
            "source_derived_bounded_truth_tables",
            "Source-derived bounded propositional truth tables",
            vec!["source_derived_bounded_counting"],
            vec![
                "boolean_expression",
                "truth_table",
                "tautology",
                "contradiction",
                "logical_equivalence",
            ],
            "OpenStax-derived finite Boolean evaluation and exhaustive validity checks",
        ),
        (
            "polynomial_algebra",
            "Bounded polynomial algebra over prime fields",
            vec![
                "abstract_algebra",
                "elementary_number_theory",
                "linear_algebra_spectral",
            ],
            vec![
                "polynomial_arithmetic",
                "polynomial_division",
                "polynomial_gcd",
                "finite_field_roots",
                "quadratic_factorization",
            ],
            "finite-field polynomial artifacts bridge algebra, arithmetic, and linear maps",
        ),
        (
            "source_derived_complex_arithmetic",
            "Source-derived bounded complex arithmetic",
            vec!["abstract_algebra"],
            vec![
                "complex_pair",
                "complex_conjugate",
                "complex_norm_squared",
                "complex_division",
            ],
            "first externally sourced domain acquired after the hand-built foundations; generic paired formula execution",
        ),
        (
            "bounded_complex_analysis",
            "Bounded exact complex-analysis theorem contracts",
            vec!["source_derived_complex_arithmetic", "bounded_calculus"],
            vec![
                "complex_polynomial_value",
                "complex_polynomial_derivative",
                "cauchy_riemann_certificate",
                "affine_holomorphic_derivative",
            ],
            "source-attributed rectangular theorem boundary; polar, contour, and infinite semantics remain separate",
        ),
        (
            "source_derived_finite_topology",
            "Source-derived bounded finite topology",
            vec!["abstract_algebra"],
            vec![
                "finite_topology",
                "open_set",
                "closed_set",
                "interior",
                "closure",
            ],
            "first source-derived domain acquired from an attributed topology definition; finite-set operations only",
        ),
        (
            "source_derived_finite_metric",
            "Source-derived bounded finite metric spaces",
            vec!["source_derived_finite_topology"],
            vec!["finite_metric", "distance", "open_ball", "diameter"],
            "source-attributed metric axioms executed over explicit finite distance tables",
        ),
        (
            "topology",
            "Bounded simplicial topology and homology",
            vec!["source_derived_finite_topology", "linear_algebra_spectral"],
            vec![
                "finite_simplicial_complex",
                "boundary_matrix_f2",
                "betti_numbers",
                "euler_characteristic",
            ],
            "validated finite simplicial extension with explicit F_2 and dimension bounds",
        ),
        (
            "bounded_dirichlet_characters",
            "Bounded finite Dirichlet characters",
            vec!["elementary_number_theory", "abstract_algebra"],
            vec![
                "finite_character",
                "root_of_unity_value",
                "character_partial_sum",
                "orthogonality_certificate",
            ],
            "finite exact character foundation; asymptotic number theory remains separate",
        ),
        (
            "bounded_arithmetic_functions",
            "Bounded arithmetic functions",
            vec!["elementary_number_theory", "real_complex_analysis"],
            vec![
                "divisor_certificate",
                "mobius_value",
                "prime_counting_value",
            ],
            "source-attributed finite arithmetic functions with explicit trial-factorization bounds",
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
        let (status, validation_gates) = if matches!(
            id,
            "linear_algebra_spectral"
                | "abstract_algebra"
                | "elementary_number_theory"
                | "combinatorics"
                | "probability_stochastic"
                | "finite_markov"
                | "finite_markov_stationary_general"
                | "real_complex_analysis"
                | "graph_theory"
                | "ordinary_differential_equations"
                | "bounded_calculus"
                | "source_formula_sequences"
                | "source_derived_science"
                | "source_derived_chemistry"
                | "source_derived_biology"
                | "source_derived_finite_statistics"
                | "source_derived_finite_regression"
                | "source_derived_linear_interpolation"
                | "source_derived_bayes_rule"
                | "source_derived_finite_set_operations"
                | "source_derived_bounded_counting"
                | "source_derived_bounded_truth_tables"
                | "polynomial_algebra"
                | "source_derived_complex_arithmetic"
                | "bounded_complex_analysis"
                | "source_derived_finite_topology"
                | "source_derived_finite_metric"
                | "topology"
                | "bounded_dirichlet_characters"
                | "bounded_arithmetic_functions"
        ) {
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
        assert_eq!(manifest.packs.len(), 32);
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
