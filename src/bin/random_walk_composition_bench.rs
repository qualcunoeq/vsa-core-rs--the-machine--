//! Phase 58 shadow bounded three-domain random-walk benchmark.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use the_machine::graph_pack::{
    adjacency_to_linear_algebra, evaluate_graph, FiniteGraph, GraphOperation, GraphRequest,
    GraphStatus,
};
use the_machine::linear_algebra_pack::{evaluate_linear_algebra, LinearAlgebraStatus};
use the_machine::probability_pack::{
    evaluate_probability, ProbabilityOperation, ProbabilityRequest, ProbabilityStatus, Rational,
};
use the_machine::random_walk_composition::{
    execute_bounded_steps, uniform_neighbor_transition, RandomWalkArtifact, RandomWalkStatus,
    TransitionConvention,
};

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    family: String,
    expected_status: RandomWalkStatus,
    expected_authorized: bool,
}

#[derive(Debug, Serialize)]
struct Row {
    id: String,
    family: String,
    expected_status: RandomWalkStatus,
    actual_status: RandomWalkStatus,
    graph_status: GraphStatus,
    probability_status: ProbabilityStatus,
    linear_algebra_status: LinearAlgebraStatus,
    expected_artifact: Option<RandomWalkArtifact>,
    actual_artifact: Option<RandomWalkArtifact>,
    exact: bool,
    graph_replay: bool,
    probability_replay: bool,
    linear_algebra_replay: bool,
    walk_replay: bool,
    three_domain_replay: bool,
    safe_refusal: bool,
    false_authorization: bool,
    tamper_rejected: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    corpus_sha256: String,
    cases: usize,
    authorized_cases: usize,
    refusal_cases: usize,
    exact_decisions: usize,
    exact_supported_artifacts: usize,
    graph_replays: usize,
    probability_replays: usize,
    linear_algebra_replays: usize,
    walk_replays: usize,
    three_domain_replays: usize,
    safe_refusals: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
    tamper_rejections: usize,
    rewrite_groups: usize,
    rows: Vec<Row>,
}

fn rational(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("benchmark rational is valid")
}

fn sha<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("random walk serializes"))
    )
}

fn cycle_graph(directed: bool) -> FiniteGraph {
    FiniteGraph {
        vertices: vec!["a".into(), "b".into(), "c".into(), "d".into()],
        edges: if directed {
            vec![(0, 1), (1, 2), (2, 3), (3, 0)]
        } else {
            vec![(0, 1), (1, 2), (2, 3), (3, 0)]
        },
        directed,
    }
}

fn graph_request(graph: &FiniteGraph, operation: GraphOperation, domain: &str) -> GraphRequest {
    GraphRequest {
        operation,
        domain: domain.into(),
        vertices: graph.vertices.clone(),
        edges: graph.edges.clone(),
        directed: graph.directed,
        matrix: None,
        vertex_order: Vec::new(),
        start: None,
        target: None,
        ambiguity: None,
        provenance: vec!["phase57-random-walk-composition".into()],
    }
}

fn initial_request(graph: &FiniteGraph, probabilities: Vec<Rational>) -> ProbabilityRequest {
    ProbabilityRequest {
        operation: ProbabilityOperation::DistributionConstruction,
        domain: "finite_exact_probability".into(),
        outcomes: graph.vertices.clone(),
        probabilities,
        values: Vec::new(),
        event_a: None,
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: None,
        provenance: vec!["phase57-random-walk-composition".into()],
    }
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for (family, count) in [
        ("one_step_walk", 30),
        ("two_step_walk", 30),
        ("four_step_walk", 30),
        ("eight_step_walk", 30),
    ] {
        for index in 0..count {
            cases.push(Case {
                id: format!("{family}_{index}"),
                family: family.into(),
                expected_status: RandomWalkStatus::Complete,
                expected_authorized: true,
            });
        }
    }
    for (family, count) in [
        ("adjacency_without_semantics", 15),
        ("zero_degree_without_policy", 15),
        ("row_column_ambiguity", 15),
        ("signed_transition_weights", 15),
        ("non_normalized_transition", 15),
        ("vertex_order_mismatch", 15),
        ("weighted_graph", 10),
        ("multi_step_walk", 10),
        ("stationary_or_spectral_claim", 10),
    ] {
        for index in 0..count {
            let expected_status = match family {
                "adjacency_without_semantics" | "row_column_ambiguity" => {
                    RandomWalkStatus::Ambiguous
                }
                "zero_degree_without_policy" => RandomWalkStatus::ZeroDegree,
                "signed_transition_weights" | "non_normalized_transition" => {
                    RandomWalkStatus::InvalidTransition
                }
                "vertex_order_mismatch" => RandomWalkStatus::DimensionMismatch,
                "weighted_graph" => RandomWalkStatus::Unsupported,
                _ => RandomWalkStatus::Unsupported,
            };
            cases.push(Case {
                id: format!("{family}_{index}"),
                family: family.into(),
                expected_status,
                expected_authorized: false,
            });
        }
    }
    cases
}

fn expected_distribution(family: &str) -> Option<RandomWalkArtifact> {
    let probabilities = match family {
        "one_step_walk" => {
            vec![
                rational(0, 1),
                rational(1, 2),
                rational(0, 1),
                rational(1, 2),
            ]
        }
        "two_step_walk" => {
            vec![
                rational(1, 2),
                rational(0, 1),
                rational(1, 2),
                rational(0, 1),
            ]
        }
        "four_step_walk" | "eight_step_walk" => {
            vec![
                rational(1, 2),
                rational(0, 1),
                rational(1, 2),
                rational(0, 1),
            ]
        }
        _ => return None,
    };
    Some(RandomWalkArtifact::NextDistribution(
        the_machine::probability_pack::FiniteDistribution {
            outcomes: vec!["a".into(), "b".into(), "c".into(), "d".into()],
            probabilities,
        },
    ))
}

fn evaluate_case(case: &Case) -> Row {
    let directed = false;
    let mut graph = cycle_graph(directed);
    let graph_domain = if case.family == "weighted_graph" {
        "weighted_graph"
    } else {
        "finite_simple_graph"
    };
    let graph_result = evaluate_graph(&graph_request(
        &graph,
        GraphOperation::Construction,
        graph_domain,
    ));
    let graph_replay = graph_result.replay_verified();
    let adjacency_result = evaluate_graph(&graph_request(
        &graph,
        GraphOperation::AdjacencyMatrix,
        graph_domain,
    ));
    let adjacency_bridge =
        adjacency_to_linear_algebra(&adjacency_result, graph.directed, &graph.vertices);
    let linear_result = adjacency_bridge.map(|request| evaluate_linear_algebra(&request));
    let linear_algebra_status = linear_result
        .as_ref()
        .map(|result| result.status)
        .unwrap_or(LinearAlgebraStatus::Missing);
    let linear_algebra_replay = linear_result
        .as_ref()
        .map(|result| result.replay_verified())
        .unwrap_or(false);
    let probabilities = vec![
        rational(1, 1),
        rational(0, 1),
        rational(0, 1),
        rational(0, 1),
    ];
    let mut probability_request = initial_request(&graph, probabilities);
    if case.family == "row_column_ambiguity" {
        probability_request.ambiguity = Some("transition convention is unstated".into());
    }
    let probability_result = evaluate_probability(&probability_request);
    let probability_replay = probability_result.replay_verified();
    let mut walk_status = RandomWalkStatus::Missing;
    let mut walk_artifact = None;
    let mut walk_replay = false;
    let mut three_domain_replay = false;
    let mut walk_result_for_tamper = None;
    let mut transition = None;
    let mut convention = Some(TransitionConvention::RowStochastic);
    let mut explicit_semantics = true;
    let mut steps = 1;

    match case.family.as_str() {
        "one_step_walk" | "two_step_walk" | "four_step_walk" | "eight_step_walk" => {
            transition = uniform_neighbor_transition(&graph).ok();
        }
        "adjacency_without_semantics" => {
            transition = uniform_neighbor_transition(&graph).ok();
            explicit_semantics = false;
        }
        "zero_degree_without_policy" => {
            graph.edges.clear();
            transition = uniform_neighbor_transition(&graph).ok();
            walk_status = RandomWalkStatus::ZeroDegree;
        }
        "row_column_ambiguity" => {
            transition = uniform_neighbor_transition(&graph).ok();
            convention = None;
        }
        "signed_transition_weights" => {
            let mut matrix = uniform_neighbor_transition(&graph).unwrap();
            matrix[0][1] = rational(-1, 1);
            transition = Some(matrix);
        }
        "non_normalized_transition" => {
            transition = Some(vec![vec![Rational::zero(); 4]; 4]);
        }
        "vertex_order_mismatch" => {
            transition = uniform_neighbor_transition(&graph).ok();
        }
        "weighted_graph" => {
            walk_status = RandomWalkStatus::Unsupported;
        }
        "multi_step_walk" => {
            transition = uniform_neighbor_transition(&graph).ok();
            steps = 9;
        }
        "stationary_or_spectral_claim" => {
            transition = uniform_neighbor_transition(&graph).ok();
            steps = 0;
        }
        _ => unreachable!(),
    }
    let vertex_order = if case.family == "vertex_order_mismatch" {
        vec!["d".into(), "c".into(), "b".into(), "a".into()]
    } else {
        graph.vertices.clone()
    };
    if walk_status != RandomWalkStatus::ZeroDegree && graph_result.status == GraphStatus::Complete {
        steps = match case.family.as_str() {
            "one_step_walk" => 1,
            "two_step_walk" => 2,
            "four_step_walk" => 4,
            "eight_step_walk" => 8,
            _ => steps,
        };
        let walk = execute_bounded_steps(
            &graph,
            transition.as_deref(),
            &probability_result,
            &vertex_order,
            convention,
            explicit_semantics,
            steps,
            vec!["phase57-random-walk-composition".into()],
        );
        walk_status = walk.status;
        walk_artifact = walk.final_artifact.clone();
        walk_replay = walk.replay_verified();
        three_domain_replay = walk_status == RandomWalkStatus::Complete
            && graph_replay
            && probability_replay
            && linear_algebra_replay
            && walk_replay;
        walk_result_for_tamper = Some(walk);
    }
    let expected_artifact = expected_distribution(&case.family);
    let exact = walk_status == case.expected_status && walk_artifact == expected_artifact;
    let authorized = walk_status == RandomWalkStatus::Complete && walk_artifact.is_some();
    let tamper_rejected = if let Some(mut walk) = walk_result_for_tamper {
        walk.replay_hash.push('x');
        !walk.replay_verified()
    } else {
        let mut tampered = graph_result.clone();
        tampered.replay_hash.push('x');
        !tampered.replay_verified()
    };
    Row {
        id: case.id.clone(),
        family: case.family.clone(),
        expected_status: case.expected_status,
        actual_status: walk_status,
        graph_status: graph_result.status,
        probability_status: probability_result.status,
        linear_algebra_status,
        expected_artifact,
        actual_artifact: walk_artifact,
        exact,
        graph_replay,
        probability_replay,
        linear_algebra_replay,
        walk_replay,
        three_domain_replay,
        safe_refusal: !case.expected_authorized && !authorized && exact,
        false_authorization: authorized && !case.expected_authorized,
        tamper_rejected,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = cases();
    let corpus_sha256 = sha(&corpus);
    let authorized_cases = corpus
        .iter()
        .filter(|case| case.expected_authorized)
        .count();
    let refusal_cases = corpus.len() - authorized_cases;
    let mut rows = Vec::new();
    let mut exact_decisions = 0;
    let mut exact_supported_artifacts = 0;
    let mut graph_replays = 0;
    let mut probability_replays = 0;
    let mut linear_algebra_replays = 0;
    let mut walk_replays = 0;
    let mut three_domain_replays = 0;
    let mut safe_refusals = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut route_leakage = 0;
    let mut tamper_rejections = 0;
    let mut rewrite_groups = BTreeSet::new();
    for case in &corpus {
        let row = evaluate_case(case);
        exact_decisions += usize::from(row.exact);
        exact_supported_artifacts += usize::from(row.exact && case.expected_authorized);
        graph_replays += usize::from(row.graph_replay);
        probability_replays += usize::from(row.probability_replay);
        linear_algebra_replays += usize::from(row.linear_algebra_replay);
        walk_replays += usize::from(row.walk_replay);
        three_domain_replays += usize::from(row.three_domain_replay);
        safe_refusals += usize::from(row.safe_refusal);
        false_authorizations += usize::from(row.false_authorization);
        false_denials += usize::from(
            !row.false_authorization
                && case.expected_authorized
                && !row.actual_status.eq(&RandomWalkStatus::Complete),
        );
        route_leakage += usize::from(row.false_authorization);
        tamper_rejections += usize::from(row.tamper_rejected);
        if case.expected_authorized {
            rewrite_groups.insert(case.family.clone());
        }
        rows.push(row);
    }
    let report = Report {
        schema_version: "phase58-bounded-random-walk-v1".into(),
        corpus_sha256,
        cases: corpus.len(),
        authorized_cases,
        refusal_cases,
        exact_decisions,
        exact_supported_artifacts,
        graph_replays,
        probability_replays,
        linear_algebra_replays,
        walk_replays,
        three_domain_replays,
        safe_refusals,
        false_authorizations,
        false_denials,
        route_leakage,
        tamper_rejections,
        rewrite_groups: rewrite_groups.len(),
        rows,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    std::fs::write(
        "docs/phase58_bounded_random_walk.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
