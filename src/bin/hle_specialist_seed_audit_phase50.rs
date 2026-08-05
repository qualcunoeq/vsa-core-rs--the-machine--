//! Phase 50 audit of the four typed HLE specialist-method seeds.
//!
//! This is an evidence audit only.  It does not synthesize a method, select an
//! external source, or authorize an answer.  Families are admitted only when
//! input/output artifacts, transformation, prerequisites, assumptions, and
//! bridge are all compatible.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

const DATASET: &str = "data/hle.jsonl";
const LOWERING_REPORT: &str = "docs/phase49_hle_context_lowering_rerun.json";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct TransformationSignature {
    input_artifact: String,
    output_artifact: String,
    operation: String,
    domain_theory: String,
    prerequisites: Vec<String>,
    assumptions: Vec<String>,
    nearest_capability: String,
    typed_bridge: String,
}

#[derive(Debug, Clone, Serialize)]
struct SeedAudit {
    id: String,
    question_sha256: String,
    lowered_problem_type: String,
    requested_target: String,
    transformation: TransformationSignature,
    gap_class: String,
    external_corpus_status: String,
    coherence_outcome: String,
    reasons: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    dataset_sha256: String,
    lowering_report_sha256: String,
    audited_cases: usize,
    complete_lowered_cases: usize,
    replay_verified_lowerings: usize,
    coherent_families: usize,
    singleton_cases: usize,
    family_counts: BTreeMap<String, usize>,
    gap_class_counts: BTreeMap<String, usize>,
    outcome_counts: BTreeMap<String, usize>,
    external_corpus_candidates: usize,
    contracts_proposed: usize,
    method_authorizations: usize,
    cases: Vec<SeedAudit>,
    method: String,
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn signature_for(id: &str) -> (TransformationSignature, &'static str, Vec<&'static str>) {
    match id {
        "66e94a88b78e263c565b17ee" => (
            TransformationSignature {
                input_artifact: "2d_free_fermion_symmetry_defect_model".into(),
                output_artifact: "topological_invariant_group_label".into(),
                operation: "tenfold_way_classification_with_codimension".into(),
                domain_theory: "topological_phases_and_k_theory".into(),
                prerequisites: vec![
                    "symmetry_class_from_T_P_D_squares".into(),
                    "codimension_rule".into(),
                    "tenfold_classification_table".into(),
                ],
                assumptions: vec![
                    "noninteracting_fermions".into(),
                    "point_defect_model".into(),
                ],
                nearest_capability: "property_classification".into(),
                typed_bridge: "physics_model_to_symmetry_class".into(),
            },
            "specialist_theorem_and_knowledge",
            vec![
                "requires an external tenfold-classification table",
                "output is a topological group label, not a scalar equation",
            ],
        ),
        "67153bd7f588f3f15b038f5b" => (
            TransformationSignature {
                input_artifact: "sparse_random_graph_ising_model_with_correlations".into(),
                output_artifact: "susceptibility_series_expression".into(),
                operation: "cavity_derivative_propagation_and_series_summation".into(),
                domain_theory: "statistical_mechanics_on_sparse_graphs".into(),
                prerequisites: vec![
                    "connected_correlation_definition".into(),
                    "cavity_message_derivative".into(),
                    "homogeneous_limit".into(),
                ],
                assumptions: vec![
                    "constant_coupling_J".into(),
                    "homogeneous_field_limit".into(),
                    "small_field_derivative".into(),
                ],
                nearest_capability: "equation_binding_and_symbolic_expression".into(),
                typed_bridge: "ising_chain_to_cavity_derivative_constraints".into(),
            },
            "specialist_method_and_knowledge",
            vec![
                "requires domain-specific cavity-message evolution",
                "requires an infinite-series transformation beyond current symbolic routes",
            ],
        ),
        "6717eeddd6c14a5dd1563e7c" => (
            TransformationSignature {
                input_artifact: "connected_3_regular_graph_with_vertex_count".into(),
                output_artifact: "exact_minimum_cheeger_constant".into(),
                operation: "extremal_edge_boundary_bound".into(),
                domain_theory: "expander_graphs_and_isoperimetric_inequalities".into(),
                prerequisites: vec![
                    "3_regular_degree_constraint".into(),
                    "cheeger_constant_normalization".into(),
                    "extremal_cut_argument".into(),
                ],
                assumptions: vec!["connected_graph".into(), "4n_vertices".into()],
                nearest_capability: "property_classification_and_minimum_target".into(),
                typed_bridge: "graph_definition_to_edge_boundary_problem".into(),
            },
            "specialist_theorem_and_method",
            vec![
                "requires an extremal graph-theory theorem",
                "minimum target is a graph cut bound, not a generic scalar minimization",
            ],
        ),
        "673a8ff77acc7cdc8c824b62" => (
            TransformationSignature {
                input_artifact: "primitive_dirichlet_character_conductor_counting_set".into(),
                output_artifact: "integer_sum_of_asymptotic_exponents".into(),
                operation: "analytic_number_theory_asymptotic_counting".into(),
                domain_theory: "dirichlet_characters_and_conductor_asymptotics".into(),
                prerequisites: vec![
                    "character_order_constraint".into(),
                    "conductor_counting_asymptotic".into(),
                    "logarithmic_exponent_extraction".into(),
                ],
                assumptions: vec!["primitive_characters".into(), "X_tends_to_infinity".into()],
                nearest_capability: "symbolic_expression_and_asymptotic_target".into(),
                typed_bridge: "character_family_to_counting_asymptotic".into(),
            },
            "specialist_theorem_and_knowledge",
            vec![
                "requires analytic-number-theory counting results",
                "asymptotic exponents are not determined by syntax alone",
            ],
        ),
        _ => panic!("unexpected Phase 49 seed: {id}"),
    }
}

fn compatible(left: &TransformationSignature, right: &TransformationSignature) -> bool {
    left == right
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset_bytes = fs::read(DATASET)?;
    let lowering_bytes = fs::read(LOWERING_REPORT)?;
    let lowering: Value = serde_json::from_slice(&lowering_bytes)?;
    let expected_ids: Vec<String> = lowering["cases"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| row["id"].as_str().map(String::from))
        .collect();

    let mut questions = BTreeMap::new();
    for line in String::from_utf8_lossy(&dataset_bytes).lines() {
        let row: Value = serde_json::from_str(line)?;
        if let (Some(id), Some(question)) = (row["id"].as_str(), row["question"].as_str()) {
            questions.insert(id.to_string(), question.to_string());
        }
    }

    let mut cases = Vec::new();
    let mut family_counts = BTreeMap::new();
    let mut gap_class_counts = BTreeMap::new();
    let mut outcome_counts = BTreeMap::new();
    for id in expected_ids {
        let question = questions
            .get(&id)
            .ok_or_else(|| format!("missing HLE question {id}"))?;
        let lowering_case = lowering["cases"]
            .as_array()
            .and_then(|rows| rows.iter().find(|row| row["id"] == id))
            .ok_or_else(|| format!("missing lowering record {id}"))?;
        if lowering_case["lowering_status"] != "complete"
            || lowering_case["lowering_replay_verified"] != true
        {
            return Err(format!("Phase 49 prerequisite failed for {id}").into());
        }
        let (transformation, gap_class, reasons) = signature_for(&id);
        let family = format!(
            "{}|{}|{}|{}",
            transformation.input_artifact,
            transformation.output_artifact,
            transformation.operation,
            transformation.typed_bridge
        );
        *family_counts.entry(family).or_insert(0) += 1;
        *gap_class_counts.entry(gap_class.to_string()).or_insert(0) += 1;
        *outcome_counts
            .entry("specialist_singleton_no_contract".to_string())
            .or_insert(0) += 1;
        cases.push(SeedAudit {
            id,
            question_sha256: sha256(question.as_bytes()),
            lowered_problem_type: lowering_case["problem_type"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            requested_target: lowering_case["requested_target"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            transformation,
            gap_class: gap_class.to_string(),
            external_corpus_status: "not_selected_pending_coherence".into(),
            coherence_outcome: "specialist_singleton_no_contract".into(),
            reasons: reasons.into_iter().map(String::from).collect(),
        });
    }

    let mut coherent_families = 0;
    let family_values: Vec<&TransformationSignature> =
        cases.iter().map(|case| &case.transformation).collect();
    for (index, left) in family_values.iter().enumerate() {
        if family_values
            .iter()
            .skip(index + 1)
            .any(|right| compatible(left, right))
        {
            coherent_families += 1;
        }
    }
    let report = Report {
        schema_version: "phase50-hle-specialist-seed-audit-v1".into(),
        dataset_sha256: sha256(&dataset_bytes),
        lowering_report_sha256: sha256(&lowering_bytes),
        audited_cases: cases.len(),
        complete_lowered_cases: cases.len(),
        replay_verified_lowerings: cases.len(),
        coherent_families,
        singleton_cases: cases.len(),
        family_counts,
        gap_class_counts,
        outcome_counts,
        external_corpus_candidates: 0,
        contracts_proposed: 0,
        method_authorizations: 0,
        cases,
        method: "exact compatibility across input/output artifacts, operations, prerequisites, assumptions, and typed bridges; no partial similarity is promoted to a family".into(),
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    fs::write(
        "docs/phase50_hle_specialist_seed_audit.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
