//! Phase 43 primitive-level audit of the 11 scalar equation-binding cases.
//!
//! The audit distinguishes reusable binding operations from domain-specific
//! methods.  It is diagnostic-only and never authorizes an HLE answer.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;

const DATASET: &str = "data/hle.jsonl";
const LAW_REPORT: &str = "docs/phase30_hle_law_audit.json";

#[derive(Debug, Serialize)]
struct BindingCase {
    id: String,
    question_sha256: String,
    input_representation: String,
    requested_output: String,
    required_bindings: Vec<String>,
    invariants: Vec<String>,
    ambiguity_rejection_boundaries: Vec<String>,
    primitive_operations: Vec<String>,
    method_family: String,
    bridge_signature: String,
    outcome: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    dataset_sha256: String,
    law_report_sha256: String,
    audited_cases: usize,
    primitive_counts: BTreeMap<String, usize>,
    method_family_counts: BTreeMap<String, usize>,
    reusable_bridge_primitives: Vec<String>,
    coherent_method_families: Vec<String>,
    outcome_counts: BTreeMap<String, usize>,
    cases: Vec<BindingCase>,
    method: String,
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn has(text: &str, marker: &str) -> bool {
    text.contains(marker)
}

fn input_representation(text: &str) -> &'static str {
    if has(text, "Ising") || has(text, "susceptibility") {
        "statistical_model_equation"
    } else if has(text, "periodic measurable") || has(text, "Poincare") || has(text, "Poincaré") {
        "functional_inequality"
    } else if has(text, "evanescent") || has(text, "electric field") || has(text, "magnetic field")
    {
        "electromagnetic_field_equation"
    } else if has(text, "traffic flow") || has(text, "partial_t") || has(text, "u(t,x)") {
        "nonlocal_pde"
    } else if has(text, "least squares") || has(text, "observations") || has(text, "predict") {
        "parametric_regression_data"
    } else if has(text, "arctan") || has(text, "integers") {
        "number_theoretic_identity"
    } else if has(text, "Dirichlet character") || has(text, "conductor") {
        "analytic_number_theory_asymptotic"
    } else if has(text, "Schur function") || has(text, "two-matrix model") {
        "matrix_model_partition_function"
    } else if has(text, "graph") || has(text, "Cheeger") {
        "graph_invariant"
    } else if has(text, "topological invariant") || has(text, "fermion") {
        "topological_classification"
    } else {
        "specialist_equation_context"
    }
}

fn method_family(text: &str) -> &'static str {
    if has(text, "least squares") || has(text, "parametric function") {
        "parametric_regression_fit"
    } else if has(text, "Ising") || has(text, "susceptibility") {
        "statistical_response_derivation"
    } else if has(text, "arctan") {
        "integer_identity_solution"
    } else if has(text, "periodic measurable") {
        "functional_inequality_constant"
    } else if has(text, "Dirichlet character") {
        "analytic_asymptotic_exponent"
    } else if has(text, "traffic flow") || has(text, "partial_t") {
        "nonlocal_pde_bound"
    } else {
        "specialist_domain_method"
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset_bytes = fs::read(DATASET)?;
    let law_bytes = fs::read(LAW_REPORT)?;
    let law: Value = serde_json::from_slice(&law_bytes)?;
    let mut cases = Vec::new();
    let mut primitive_counts = BTreeMap::new();
    let mut method_family_counts = BTreeMap::new();
    let mut outcome_counts = BTreeMap::new();
    let scalar_ids: std::collections::BTreeSet<String> = law
        .get("cases")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|row| {
            row.get("outcome").and_then(Value::as_str) == Some("in_question_equation")
                && row.get("requested_output").and_then(Value::as_str)
                    == Some("scalar_or_structured_value")
                && row
                    .get("bridge_primitives")
                    .and_then(Value::as_array)
                    .is_some_and(|bridges| {
                        bridges
                            .iter()
                            .any(|bridge| bridge.as_str() == Some("equation_binding"))
                    })
        })
        .filter_map(|row| row.get("id").and_then(Value::as_str).map(str::to_string))
        .collect();
    for line in String::from_utf8(dataset_bytes.clone())?
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        let entry: Value = serde_json::from_str(line)?;
        let id = entry.get("id").and_then(Value::as_str).unwrap_or("");
        if !scalar_ids.contains(id) {
            continue;
        }
        let question = entry.get("question").and_then(Value::as_str).unwrap_or("");
        let lower = question.to_ascii_lowercase();
        let source = law
            .get("cases")
            .and_then(Value::as_array)
            .and_then(|rows| {
                rows.iter()
                    .find(|row| row.get("id").and_then(Value::as_str) == Some(id))
            })
            .expect("selected law audit case exists");
        let representation = input_representation(&lower).to_string();
        let family = method_family(&lower).to_string();
        let required_bindings: Vec<String> = source
            .get("variables")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect();
        let mut invariants: Vec<String> = source
            .get("assumptions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect();
        if has(&lower, "periodic") {
            invariants.push("periodicity/domain condition".into());
        }
        if has(&lower, "least squares") {
            invariants.push("least-squares objective and parsimony criterion".into());
        }
        let mut boundaries = vec!["reject unresolved target or missing symbol binding".into()];
        if invariants.is_empty() {
            boundaries.push("external specialist assumptions may be required".into());
        } else {
            boundaries.push(
                "reject when stated assumptions or validity conditions are unsatisfied".into(),
            );
        }
        let mut primitives = vec![
            "bind_local_symbols_to_typed_values".to_string(),
            "bind_requested_unknown".to_string(),
        ];
        if has(&lower, "sum")
            || has(&lower, "integral")
            || has(&lower, "derivative")
            || has(&lower, "partial")
            || has(&lower, "distance")
        {
            primitives.push("bind_indices_functions_or_domains".into());
        }
        if has(&lower, "system") || has(&lower, "equation") || has(&lower, "satisfying") {
            primitives.push("bind_coupled_constraints".into());
        }
        if !invariants.is_empty() {
            primitives.push("propagate_assumptions_and_validity".into());
        }
        primitives.sort();
        primitives.dedup();
        for primitive in &primitives {
            *primitive_counts.entry(primitive.clone()).or_insert(0) += 1;
        }
        *method_family_counts.entry(family.clone()).or_insert(0) += 1;
        let outcome = if family == "parametric_regression_fit" {
            "two_case_method_candidate"
        } else {
            "specialist_singleton"
        };
        *outcome_counts.entry(outcome.to_string()).or_insert(0) += 1;
        cases.push(BindingCase {
            id: id.into(),
            question_sha256: sha256(question.as_bytes()),
            input_representation: representation.clone(),
            requested_output: source
                .get("requested_output")
                .and_then(Value::as_str)
                .unwrap_or("scalar_or_structured_value")
                .into(),
            required_bindings,
            invariants,
            ambiguity_rejection_boundaries: boundaries,
            primitive_operations: primitives.clone(),
            method_family: family.clone(),
            bridge_signature: format!(
                "{representation}::bind_local_symbols::bind_requested_unknown"
            ),
            outcome: outcome.into(),
        });
    }
    let reusable_bridge_primitives = primitive_counts
        .iter()
        .filter(|(_, count)| **count >= 8)
        .map(|(primitive, _)| primitive.clone())
        .collect();
    let coherent_method_families = method_family_counts
        .iter()
        .filter(|(_, count)| **count >= 2)
        .map(|(family, _)| family.clone())
        .collect();
    let report = Report {
        schema_version: "phase43.hle.equation.binding.audit.v1".into(),
        dataset_sha256: sha256(&dataset_bytes),
        law_report_sha256: sha256(&law_bytes),
        audited_cases: cases.len(),
        primitive_counts,
        method_family_counts,
        reusable_bridge_primitives,
        coherent_method_families,
        outcome_counts,
        cases,
        method: "primitive-level audit of the 11 scalar equation-binding candidates; no execution, external retrieval, or promotion".into(),
    };
    let output = serde_json::to_string_pretty(&report)?;
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "docs/phase43_hle_equation_binding_audit.json".into());
    fs::write(&path, output)?;
    println!("phase43 report written to {path}");
    Ok(())
}
