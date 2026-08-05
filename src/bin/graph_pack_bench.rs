//! Phase 56 shadow bounded graph-theory curriculum benchmark.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use the_machine::graph_pack::{
    adjacency_to_linear_algebra, evaluate_graph, FiniteGraph, GraphArtifact, GraphOperation,
    GraphRequest, GraphStatus,
};
use the_machine::linear_algebra_pack::{evaluate_linear_algebra, LinearAlgebraStatus};

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    family: String,
    request: GraphRequest,
    expected_status: GraphStatus,
    expected_artifact: Option<GraphArtifact>,
    expected_authorized: bool,
    rewrite_group: Option<String>,
}

#[derive(Debug, Serialize)]
struct Row {
    id: String,
    family: String,
    expected_status: GraphStatus,
    actual_status: GraphStatus,
    expected_artifact: Option<GraphArtifact>,
    actual_artifact: Option<GraphArtifact>,
    exact: bool,
    replay_verified: bool,
    bridge_replay: bool,
    bridge_status: Option<LinearAlgebraStatus>,
    false_authorization: bool,
    rewrite_group: Option<String>,
    tamper_rejected: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    source: String,
    corpus_sha256: String,
    case_count: usize,
    supported_cases: usize,
    boundary_cases: usize,
    unsupported_cases: usize,
    exact_decisions: usize,
    exact_supported_artifacts: usize,
    replay_verified: usize,
    bridge_replays: usize,
    safe_refusals: usize,
    false_authorizations: usize,
    false_denials: usize,
    route_leakage: usize,
    tamper_rejections: usize,
    rewrite_groups: usize,
    rows: Vec<Row>,
}

fn sha<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("graph benchmark serializes"))
    )
}

fn provenance() -> Vec<String> {
    vec!["phase56-graph-pack-corpus".into()]
}

fn path_graph_request(operation: GraphOperation) -> GraphRequest {
    GraphRequest {
        operation,
        domain: "finite_simple_graph".into(),
        vertices: vec!["a".into(), "b".into(), "c".into(), "d".into()],
        edges: vec![(0, 1), (1, 2), (2, 3)],
        directed: false,
        matrix: None,
        vertex_order: Vec::new(),
        start: Some(0),
        target: Some(3),
        ambiguity: None,
        provenance: provenance(),
    }
}

fn path_graph() -> FiniteGraph {
    FiniteGraph {
        vertices: vec!["a".into(), "b".into(), "c".into(), "d".into()],
        edges: vec![(0, 1), (1, 2), (2, 3)],
        directed: false,
    }
}

fn path_adjacency() -> Vec<Vec<i64>> {
    vec![
        vec![0, 1, 0, 0],
        vec![1, 0, 1, 0],
        vec![0, 1, 0, 1],
        vec![0, 0, 1, 0],
    ]
}

fn path_incidence() -> Vec<Vec<i64>> {
    vec![vec![1, 0, 0], vec![1, 1, 0], vec![0, 1, 1], vec![0, 0, 1]]
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    let supported = [
        (
            "construction",
            GraphOperation::Construction,
            GraphArtifact::Graph(path_graph()),
        ),
        (
            "edge_count",
            GraphOperation::EdgeCount,
            GraphArtifact::Scalar(3),
        ),
        (
            "degrees",
            GraphOperation::Degrees,
            GraphArtifact::Degrees(vec![1, 2, 2, 1]),
        ),
        (
            "reachability",
            GraphOperation::Reachability,
            GraphArtifact::Boolean(true),
        ),
        (
            "components",
            GraphOperation::ConnectedComponents,
            GraphArtifact::Components(vec![vec![0, 1, 2, 3]]),
        ),
        ("tree", GraphOperation::IsTree, GraphArtifact::Boolean(true)),
        (
            "adjacency_matrix",
            GraphOperation::AdjacencyMatrix,
            GraphArtifact::Matrix(path_adjacency()),
        ),
        (
            "incidence_matrix",
            GraphOperation::IncidenceMatrix,
            GraphArtifact::Matrix(path_incidence()),
        ),
    ];
    for (family, operation, artifact) in supported {
        let count = match family {
            "construction" => 20,
            "adjacency_matrix" | "incidence_matrix" => 15,
            _ => 10,
        };
        for index in 0..count {
            let request = path_graph_request(operation);
            let rewrite_group = match family {
                "adjacency_matrix" => Some("adjacency_rewrites".into()),
                "degrees" => Some("degree_rewrites".into()),
                _ => None,
            };
            cases.push(Case {
                id: format!("{family}_{index}"),
                family: family.into(),
                request,
                expected_status: GraphStatus::Complete,
                expected_artifact: Some(artifact.clone()),
                expected_authorized: true,
                rewrite_group,
            });
        }
    }
    for index in 0..10 {
        let mut request = GraphRequest {
            operation: GraphOperation::GraphFromAdjacency,
            domain: "finite_simple_graph".into(),
            vertices: Vec::new(),
            edges: Vec::new(),
            directed: false,
            matrix: Some(path_adjacency()),
            vertex_order: vec!["a".into(), "b".into(), "c".into(), "d".into()],
            start: None,
            target: None,
            ambiguity: None,
            provenance: provenance(),
        };
        request.vertex_order.reverse();
        let graph = FiniteGraph {
            vertices: vec!["d".into(), "c".into(), "b".into(), "a".into()],
            edges: vec![(0, 1), (1, 2), (2, 3)],
            directed: false,
        };
        cases.push(Case {
            id: format!("graph_from_adjacency_{index}"),
            family: "graph_from_adjacency".into(),
            request,
            expected_status: GraphStatus::Complete,
            expected_artifact: Some(GraphArtifact::Graph(graph)),
            expected_authorized: true,
            rewrite_group: Some("adjacency_rewrites".into()),
        });
    }
    for index in 0..10 {
        let mut request = path_graph_request(GraphOperation::EdgeCount);
        request.edges.clear();
        cases.push(Case {
            id: format!("empty_graph_edge_count_{index}"),
            family: "empty_graph_edge_count".into(),
            request,
            expected_status: GraphStatus::Complete,
            expected_artifact: Some(GraphArtifact::Scalar(0)),
            expected_authorized: true,
            rewrite_group: None,
        });
    }

    for index in 0..10 {
        let mut request = path_graph_request(GraphOperation::Construction);
        request.vertices.clear();
        request.edges.clear();
        cases.push(Case {
            id: format!("missing_graph_{index}"),
            family: "missing_graph".into(),
            request,
            expected_status: GraphStatus::Missing,
            expected_artifact: None,
            expected_authorized: false,
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        let mut request = path_graph_request(GraphOperation::Construction);
        request.edges.push((1, 0));
        cases.push(Case {
            id: format!("duplicate_edge_{index}"),
            family: "duplicate_edge".into(),
            request,
            expected_status: GraphStatus::InvalidGraph,
            expected_artifact: None,
            expected_authorized: false,
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        let mut request = path_graph_request(GraphOperation::ConnectedComponents);
        request.ambiguity =
            Some("directed weak versus strong component semantics are unstated".into());
        request.directed = true;
        cases.push(Case {
            id: format!("component_semantics_ambiguity_{index}"),
            family: "component_semantics_ambiguity".into(),
            request,
            expected_status: GraphStatus::Ambiguous,
            expected_artifact: None,
            expected_authorized: false,
            rewrite_group: None,
        });
    }
    for index in 0..10 {
        let mut request = path_graph_request(GraphOperation::GraphFromAdjacency);
        request.vertices.clear();
        request.edges.clear();
        request.matrix = Some(path_adjacency());
        request.vertex_order.clear();
        cases.push(Case {
            id: format!("vertex_order_ambiguity_{index}"),
            family: "vertex_order_ambiguity".into(),
            request,
            expected_status: GraphStatus::Ambiguous,
            expected_artifact: None,
            expected_authorized: false,
            rewrite_group: None,
        });
    }

    for (family, domain) in [
        ("weighted_graph", "weighted_graph"),
        ("multigraph", "finite_multigraph"),
        ("hypergraph", "finite_hypergraph"),
        ("infinite_graph", "infinite_graph"),
        ("graph_limits", "graph_limits"),
        ("spectral_graph", "spectral_graph"),
        ("random_graph_asymptotics", "random_graph_asymptotics"),
        ("cheeger_invariant", "specialist_graph_invariant"),
    ] {
        for index in 0..10 {
            let mut request = path_graph_request(GraphOperation::Construction);
            request.domain = domain.into();
            cases.push(Case {
                id: format!("{family}_{index}"),
                family: family.into(),
                request,
                expected_status: GraphStatus::Unsupported,
                expected_artifact: None,
                expected_authorized: false,
                rewrite_group: None,
            });
        }
    }
    cases
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = cases();
    let corpus_sha256 = sha(&corpus);
    let supported_cases = corpus
        .iter()
        .filter(|case| case.expected_authorized)
        .count();
    let boundary_cases = corpus
        .iter()
        .filter(|case| {
            matches!(
                case.expected_status,
                GraphStatus::Missing | GraphStatus::Ambiguous | GraphStatus::InvalidGraph
            )
        })
        .count();
    let unsupported_cases = corpus.len() - supported_cases - boundary_cases;
    let mut rows = Vec::new();
    let mut exact_decisions = 0;
    let mut exact_supported_artifacts = 0;
    let mut replay_verified = 0;
    let mut bridge_replays = 0;
    let mut safe_refusals = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut route_leakage = 0;
    let mut tamper_rejections = 0;
    let mut rewrite_groups = BTreeSet::new();
    for case in &corpus {
        let result = evaluate_graph(&case.request);
        let exact =
            result.status == case.expected_status && result.artifact == case.expected_artifact;
        let authorized = result.status == GraphStatus::Complete && result.artifact.is_some();
        let bridge_replay = if matches!(
            case.family.as_str(),
            "adjacency_matrix" | "graph_from_adjacency"
        ) && result.status == GraphStatus::Complete
        {
            adjacency_to_linear_algebra(&result, case.request.directed, &case.request.vertices)
                .or_else(|| {
                    adjacency_to_linear_algebra(
                        &result,
                        case.request.directed,
                        &case.request.vertex_order,
                    )
                })
                .map(|request| evaluate_linear_algebra(&request).replay_verified())
                .unwrap_or(false)
        } else {
            false
        };
        let tamper_rejected = {
            let mut tampered = result.clone();
            tampered.replay_hash.push('x');
            !tampered.replay_verified()
        };
        let replay = result.replay_verified();
        exact_decisions += usize::from(exact);
        exact_supported_artifacts +=
            usize::from(exact && case.expected_status == GraphStatus::Complete);
        replay_verified += usize::from(replay);
        bridge_replays += usize::from(bridge_replay);
        safe_refusals += usize::from(!case.expected_authorized && !authorized && replay);
        false_authorizations += usize::from(authorized && !case.expected_authorized);
        false_denials += usize::from(!authorized && case.expected_authorized);
        route_leakage += usize::from(
            !case.expected_authorized && case.family.contains("spectral") && authorized,
        );
        tamper_rejections += usize::from(tamper_rejected);
        if let Some(group) = &case.rewrite_group {
            rewrite_groups.insert(group.clone());
        }
        rows.push(Row {
            id: case.id.clone(),
            family: case.family.clone(),
            expected_status: case.expected_status,
            actual_status: result.status,
            expected_artifact: case.expected_artifact.clone(),
            actual_artifact: result.artifact.clone(),
            exact,
            replay_verified: replay,
            bridge_replay,
            bridge_status: if matches!(
                case.family.as_str(),
                "adjacency_matrix" | "graph_from_adjacency"
            ) {
                Some(
                    adjacency_to_linear_algebra(
                        &result,
                        case.request.directed,
                        &case.request.vertices,
                    )
                    .or_else(|| {
                        adjacency_to_linear_algebra(
                            &result,
                            case.request.directed,
                            &case.request.vertex_order,
                        )
                    })
                    .map(|request| evaluate_linear_algebra(&request).status)
                    .unwrap_or(LinearAlgebraStatus::Missing),
                )
            } else {
                None
            },
            false_authorization: authorized && !case.expected_authorized,
            rewrite_group: case.rewrite_group.clone(),
            tamper_rejected,
        });
    }
    let report = Report {
        schema_version: "phase56-graph-pack-v1".into(),
        source: "MIT OCW 6.042J Mathematics for Computer Science (shadow citation; no production registration)".into(),
        corpus_sha256,
        case_count: corpus.len(),
        supported_cases,
        boundary_cases,
        unsupported_cases,
        exact_decisions,
        exact_supported_artifacts,
        replay_verified,
        bridge_replays,
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
        "docs/phase56_graph_pack_bench.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
