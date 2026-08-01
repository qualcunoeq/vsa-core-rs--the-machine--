//! Phase 42 semantic-coherence audit for the 17 scalar matrix candidates.
//!
//! The Phase 41 signature is only a mining signal.  This pass records the
//! actual representation, operation, prerequisites, and bridge so unrelated
//! matrix-shaped questions cannot become one capability by name alone.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;

const DATASET: &str = "data/hle.jsonl";
const METHOD_REPORT: &str = "docs/phase29_hle_reasoning_method_audit.json";

#[derive(Debug, Serialize)]
struct MatrixCase {
    id: String,
    question_sha256: String,
    input_representation: String,
    requested_output: String,
    dimensions: String,
    entry_domain: String,
    numeric_or_symbolic: String,
    required_operation: String,
    theorem_or_identity: String,
    parameter_dependence: String,
    proof_or_computation: String,
    existing_solver_compatibility: String,
    typed_bridge: String,
    coherence_signature: String,
    outcome: String,
    reasons: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    dataset_sha256: String,
    method_report_sha256: String,
    audited_cases: usize,
    exact_matrix_signature_cases: usize,
    coherence_family_counts: BTreeMap<String, usize>,
    reason_counts: BTreeMap<String, usize>,
    outcome_counts: BTreeMap<String, usize>,
    coherent_families: usize,
    cases: Vec<MatrixCase>,
    method: String,
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn has(text: &str, marker: &str) -> bool {
    text.contains(marker)
}

fn dimensions(text: &str) -> String {
    for marker in [
        "271 \\times 271",
        "101 \\times 101",
        "7×7",
        "7 x 7",
        "2 \\times 2",
        "3 \\times 3",
        "N\\times N",
        "N×N",
    ] {
        if has(text, marker) {
            return marker.into();
        }
    }
    if has(text, "matrix") {
        "unspecified_or_embedded".into()
    } else {
        "not_matrix_explicit".into()
    }
}

fn classify(
    text: &str,
) -> (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    Vec<String>,
) {
    let input = if has(text, "adjacency matrix") || has(text, "graph") || has(text, "quiver") {
        "graph_or_adjacency_matrix"
    } else if has(text, "transfer function") || has(text, "observer") || has(text, "state-space") {
        "control_system_matrices"
    } else if has(text, "covariance") || has(text, "mutual information") || has(text, "gaussian") {
        "probabilistic_matrix"
    } else if has(text, "GHZ") || has(text, "qubit") || has(text, "Bell state") {
        "quantum_state_operator"
    } else if has(text, "SO(n)")
        || has(text, "symmetric positive definite")
        || has(text, "normal cone")
    {
        "matrix_manifold_or_optimization"
    } else if has(text, "eigenvalue") || has(text, "singular value") || has(text, "spectrum") {
        "spectral_matrix"
    } else if has(text, "det(") || has(text, "determinant") || has(text, "matrix equation") {
        "explicit_matrix_expression"
    } else {
        "specialist_matrix_context"
    };
    let output = if has(text, "determinant") || has(text, "det(") {
        "scalar_determinant"
    } else if has(text, "rank") {
        "scalar_rank_or_rank_classification"
    } else if has(text, "eigenvalue") || has(text, "spectrum") {
        "spectral_quantity_or_count"
    } else if has(text, "observer") || has(text, "factorization") {
        "matrix_gain_or_factorization"
    } else if has(text, "how many") || has(text, "count") {
        "count_or_classification"
    } else {
        "expression_or_specialist_answer"
    };
    let operation =
        if has(text, "compute the determinant") || has(text, "calculate the determinant") {
            "direct_determinant_evaluation"
        } else if has(text, "determinant") || has(text, "det(") {
            "determinant_in_identity_or_specialist_formula"
        } else if has(text, "rank") {
            "rank_or_rank_constraint"
        } else if has(text, "eigenvalue") || has(text, "spectrum") {
            "spectral_analysis"
        } else if has(text, "observer") || has(text, "transfer function") {
            "control_design_or_factorization"
        } else if has(text, "adjacency") || has(text, "graph") || has(text, "quiver") {
            "graph_to_matrix_invariant"
        } else if has(text, "mutual information") || has(text, "covariance") {
            "matrix_optimization_information"
        } else if has(text, "normal cone") || has(text, "Brockett") {
            "manifold_or_constrained_optimization"
        } else {
            "specialist_matrix_transformation"
        };
    let theorem = if has(text, "theorem") || has(text, "identity") || has(text, "Brockett") {
        "explicit_theorem_or_identity"
    } else if has(text, "defined") || has(text, "given that") {
        "local_definitions_or_assumptions"
    } else {
        "none_explicit"
    };
    let parameter = if has(text, "parameter")
        || has(text, "k_i")
        || has(text, "k_1")
        || has(text, "x_1")
        || has(text, "alpha")
    {
        "parameterized_or_random"
    } else if has(text, "n ") || has(text, "N ") {
        "dimension_parameterized"
    } else {
        "fixed_or_unspecified"
    };
    let proof =
        if has(text, "prove") || has(text, "show") || has(text, "why") || has(text, "is it true") {
            "proof_or_explanation"
        } else {
            "computation_or_construction"
        };
    let compatibility = if input == "explicit_matrix_expression"
        && (operation == "direct_determinant_evaluation" || operation == "rank_or_rank_constraint")
        && dimensions(text) == "3 \\times 3"
    {
        "existing_small_matrix_algebra_candidate"
    } else if input == "explicit_matrix_expression" && operation == "direct_determinant_evaluation"
    {
        "requires_bounded_symbolic_matrix_extension"
    } else {
        "no_existing_exact_route"
    };
    let bridge = if input == "graph_or_adjacency_matrix" {
        "graph_to_matrix_bridge"
    } else if input == "control_system_matrices" {
        "state_space_matrix_bridge"
    } else if input == "probabilistic_matrix" {
        "covariance_to_matrix_bridge"
    } else if input == "explicit_matrix_expression" {
        "matrix_literal_or_symbol_binding"
    } else if input == "spectral_matrix" {
        "spectral_object_binding"
    } else {
        "specialist_matrix_bridge"
    };
    let mut reasons = Vec::new();
    if input != "explicit_matrix_expression" {
        reasons.push("input representation is not a bounded explicit matrix artifact".into());
    }
    if parameter == "parameterized_or_random" {
        reasons.push("parameters or random variables require additional binding semantics".into());
    }
    if operation != "direct_determinant_evaluation" && operation != "rank_or_rank_constraint" {
        reasons.push("operation is not the bounded direct rank/determinant primitive".into());
    }
    (
        input.into(),
        output.into(),
        operation.into(),
        theorem.into(),
        parameter.into(),
        proof.into(),
        compatibility.into(),
        bridge.into(),
        reasons,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset_bytes = fs::read(DATASET)?;
    let method_bytes = fs::read(METHOD_REPORT)?;
    let method: Value = serde_json::from_slice(&method_bytes)?;
    let ids: Vec<String> = method
        .get("cases")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|row| {
            row.get("signature").and_then(Value::as_str) == Some("matrix_rank_or_determinant")
                && row.get("output_artifact").and_then(Value::as_str)
                    == Some("scalar_or_structured_answer")
        })
        .filter_map(|row| row.get("id").and_then(Value::as_str).map(str::to_string))
        .collect();
    let id_set: std::collections::BTreeSet<String> = ids.iter().cloned().collect();
    let mut cases = Vec::new();
    let mut family_counts = BTreeMap::new();
    let mut outcome_counts = BTreeMap::new();
    let mut reason_counts = BTreeMap::new();
    for line in String::from_utf8(dataset_bytes.clone())?
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        let entry: Value = serde_json::from_str(line)?;
        let id = entry.get("id").and_then(Value::as_str).unwrap_or("");
        if !id_set.contains(id) {
            continue;
        }
        let question = entry.get("question").and_then(Value::as_str).unwrap_or("");
        let lower = question.to_ascii_lowercase();
        let (input, output, operation, theorem, parameter, proof, compatibility, bridge, reasons) =
            classify(&lower);
        let signature = format!("{input}::{operation}::{output}::{proof}");
        let outcome = if input == "explicit_matrix_expression"
            && (operation == "direct_determinant_evaluation"
                || operation == "rank_or_rank_constraint")
            && reasons.is_empty()
        {
            "potentially_coherent_direct_matrix_family"
        } else if reasons.len() == 1 && reasons[0].contains("parameters") {
            "parameterized_or_random_specialist"
        } else {
            "specialist_or_incompatible"
        };
        *family_counts.entry(signature.clone()).or_insert(0) += 1;
        *outcome_counts.entry(outcome.to_string()).or_insert(0) += 1;
        for reason in &reasons {
            *reason_counts.entry(reason.clone()).or_insert(0) += 1;
        }
        cases.push(MatrixCase {
            id: id.into(),
            question_sha256: sha256(question.as_bytes()),
            input_representation: input,
            requested_output: output,
            dimensions: dimensions(&lower),
            entry_domain: if has(&lower, "complex") {
                "complex".into()
            } else if has(&lower, "real") {
                "real".into()
            } else if has(&lower, "integer") || has(&lower, "rational") {
                "integer_or_rational".into()
            } else if has(&lower, "normal(") || has(&lower, "gaussian") {
                "random_distribution".into()
            } else {
                "symbolic_or_mixed".into()
            },
            numeric_or_symbolic: if has(&lower, "symbolic")
                || has(&lower, "parameter")
                || has(&lower, "lambda")
                || has(&lower, "x_1")
            {
                "symbolic_or_parameterized".into()
            } else if has(&lower, "random") || has(&lower, "normal(") {
                "random".into()
            } else {
                "numeric_or_mixed".into()
            },
            required_operation: operation,
            theorem_or_identity: theorem,
            parameter_dependence: parameter,
            proof_or_computation: proof,
            existing_solver_compatibility: compatibility,
            typed_bridge: bridge,
            coherence_signature: signature,
            outcome: outcome.into(),
            reasons,
        });
    }
    let coherent_families = 0;
    let report = Report {
        schema_version: "phase42.hle.matrix.coherence.audit.v1".into(),
        dataset_sha256: sha256(&dataset_bytes),
        method_report_sha256: sha256(&method_bytes),
        audited_cases: cases.len(),
        exact_matrix_signature_cases: ids.len(),
        coherence_family_counts: family_counts,
        reason_counts,
        outcome_counts,
        coherent_families,
        cases,
        method: "semantic-coherence audit of the 17 scalar matrix candidates; lexical cues are evidence for review only and never authorize execution".into(),
    };
    let output = serde_json::to_string_pretty(&report)?;
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "docs/phase42_hle_matrix_coherence_audit.json".into());
    fs::write(&path, output)?;
    println!("phase42 report written to {path}");
    Ok(())
}
