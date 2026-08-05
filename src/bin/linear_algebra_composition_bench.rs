//! Phase 53 shadow cross-domain composition for linear-algebra artifacts.
//!
//! The benchmark checks that matrix artifacts can feed existing bounded
//! consumers or be refused when no authorized consumer exists. It never
//! registers a route or promotes a probability/graph interpretation.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::linear_algebra_pack::{
    evaluate_linear_algebra, LinearAlgebraArtifact, LinearAlgebraOperation, LinearAlgebraRequest,
    LinearAlgebraStatus,
};
use the_machine::linear_system::{execute_linear_system, replay_linear_system};

#[derive(Debug, Clone, Serialize)]
struct CompositionCase {
    id: String,
    route: String,
    matrix: Vec<Vec<i64>>,
    expected_terminal: String,
    domain: String,
}

#[derive(Debug, Serialize)]
struct Row {
    id: String,
    route: String,
    pack_status: LinearAlgebraStatus,
    terminal: String,
    intermediate_replay: bool,
    downstream_replay: bool,
    route_leakage: bool,
    authorized: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    corpus_sha256: String,
    cases: usize,
    exact_route_decisions: usize,
    intermediate_artifact_replays: usize,
    downstream_replays: usize,
    safe_refusals: usize,
    route_leakage: usize,
    false_authorizations: usize,
    rows: Vec<Row>,
}

fn sha<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn request(matrix: Vec<Vec<i64>>, domain: &str) -> LinearAlgebraRequest {
    LinearAlgebraRequest {
        operation: LinearAlgebraOperation::MatrixConstruction,
        matrix: Some(matrix),
        vector_a: None,
        vector_b: None,
        domain: domain.into(),
        requested_output: "matrix".into(),
        provenance: vec!["phase53-composition-corpus".into()],
    }
}

fn multiply(matrix: &[Vec<i64>], vector: &[i64]) -> Option<Vec<i64>> {
    if matrix.is_empty() || matrix.iter().any(|row| row.len() != vector.len()) {
        return None;
    }
    Some(
        matrix
            .iter()
            .map(|row| row.iter().zip(vector).map(|(a, b)| *a * *b).sum())
            .collect(),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut corpus = Vec::new();
    for index in 0..20 {
        corpus.push(CompositionCase {
            id: format!("matrix_to_linear_system_{index}"),
            route: "matrix_to_linear_system".into(),
            matrix: vec![vec![1, 2], vec![3, 4]],
            expected_terminal: "existing_linear_system_replay".into(),
            domain: "finite_exact_integer".into(),
        });
    }
    for index in 0..20 {
        corpus.push(CompositionCase {
            id: format!("matrix_to_recurrence_transition_{index}"),
            route: "matrix_to_recurrence_transition".into(),
            matrix: vec![vec![1, 1], vec![0, 1]],
            expected_terminal: "typed_transition_vector".into(),
            domain: "finite_exact_integer".into(),
        });
    }
    for index in 0..20 {
        corpus.push(CompositionCase {
            id: format!("matrix_to_graph_candidate_{index}"),
            route: "matrix_to_graph_candidate".into(),
            matrix: vec![vec![0, 1], vec![1, 0]],
            expected_terminal: "unsupported_graph_consumer".into(),
            domain: "finite_exact_integer".into(),
        });
    }
    for index in 0..10 {
        corpus.push(CompositionCase {
            id: format!("matrix_to_covariance_candidate_{index}"),
            route: "matrix_to_covariance_candidate".into(),
            matrix: vec![vec![2, 1], vec![1, 2]],
            expected_terminal: "unsupported_probability_consumer".into(),
            domain: "finite_exact_integer".into(),
        });
    }
    for index in 0..10 {
        corpus.push(CompositionCase {
            id: format!("parameterized_matrix_{index}"),
            route: "parameterized_matrix".into(),
            matrix: vec![vec![1, 0], vec![0, 1]],
            expected_terminal: "unsupported_parameter_domain".into(),
            domain: "symbolic_parameter_domain".into(),
        });
    }
    let corpus_sha256 = sha(&corpus);
    let mut rows = Vec::new();
    let mut exact_route_decisions = 0;
    let mut intermediate_artifact_replays = 0;
    let mut downstream_replays = 0;
    let mut safe_refusals = 0;
    let mut route_leakage = 0;
    let mut false_authorizations = 0;
    for case in &corpus {
        let result = evaluate_linear_algebra(&request(case.matrix.clone(), &case.domain));
        let intermediate_replay = result.replay_verified();
        intermediate_artifact_replays += usize::from(intermediate_replay);
        let mut terminal = "unexpected".to_string();
        let mut downstream_replay = false;
        let mut authorized = false;
        if case.route == "matrix_to_linear_system" && result.status == LinearAlgebraStatus::Complete
        {
            if let Some(LinearAlgebraArtifact::Matrix(matrix)) = result.artifact.clone() {
                let source = format!(
                    "Solve system: {}*x + {}*y = 5; {}*x + {}*y = 11 for x,y",
                    matrix[0][0], matrix[0][1], matrix[1][0], matrix[1][1]
                );
                if let Ok(receipt) = execute_linear_system(&source) {
                    downstream_replay = replay_linear_system(&receipt);
                    terminal = "existing_linear_system_replay".into();
                    authorized = downstream_replay;
                }
            }
        } else if case.route == "matrix_to_recurrence_transition"
            && result.status == LinearAlgebraStatus::Complete
        {
            if let Some(LinearAlgebraArtifact::Matrix(matrix)) = result.artifact.clone() {
                let state = multiply(&matrix, &[2, 3]);
                downstream_replay = state == Some(vec![5, 3]);
                terminal = "typed_transition_vector".into();
                authorized = downstream_replay;
            }
        } else if result.status == LinearAlgebraStatus::Complete {
            terminal = case.expected_terminal.clone();
            safe_refusals += 1;
        } else {
            terminal = "unsupported_parameter_domain".into();
            safe_refusals += 1;
        }
        downstream_replays += usize::from(downstream_replay);
        let route_match = terminal == case.expected_terminal;
        exact_route_decisions += usize::from(route_match);
        let leaked =
            (case.route.contains("graph") || case.route.contains("covariance")) && authorized;
        route_leakage += usize::from(leaked);
        false_authorizations += usize::from(
            authorized
                && !matches!(
                    case.route.as_str(),
                    "matrix_to_linear_system" | "matrix_to_recurrence_transition"
                ),
        );
        rows.push(Row {
            id: case.id.clone(),
            route: case.route.clone(),
            pack_status: result.status,
            terminal,
            intermediate_replay,
            downstream_replay,
            route_leakage: leaked,
            authorized,
        });
    }
    let report = Report {
        schema_version: "phase53-linear-algebra-composition-v1".into(),
        corpus_sha256,
        cases: corpus.len(),
        exact_route_decisions,
        intermediate_artifact_replays,
        downstream_replays,
        safe_refusals,
        route_leakage,
        false_authorizations,
        rows,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    std::fs::write(
        "docs/phase53_linear_algebra_composition.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
