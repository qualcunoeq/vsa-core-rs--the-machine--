//! Shadow finite graph-theory curriculum pack.
//!
//! The pack covers simple finite directed and undirected graphs with explicit
//! vertex identity and ordering. Weighted, multi-, hyper-, infinite, random,
//! asymptotic, and specialist spectral graph semantics remain outside this
//! curriculum boundary.

use crate::linear_algebra_pack::{LinearAlgebraOperation, LinearAlgebraRequest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, VecDeque};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FiniteGraph {
    pub vertices: Vec<String>,
    pub edges: Vec<(usize, usize)>,
    pub directed: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GraphOperation {
    Construction,
    EdgeCount,
    Degrees,
    Reachability,
    ConnectedComponents,
    IsTree,
    AdjacencyMatrix,
    IncidenceMatrix,
    GraphFromAdjacency,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GraphStatus {
    Complete,
    Missing,
    Ambiguous,
    InvalidGraph,
    DimensionMismatch,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GraphArtifact {
    Graph(FiniteGraph),
    Scalar(usize),
    Boolean(bool),
    Degrees(Vec<usize>),
    Components(Vec<Vec<usize>>),
    Matrix(Vec<Vec<i64>>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphRequest {
    pub operation: GraphOperation,
    pub domain: String,
    pub vertices: Vec<String>,
    pub edges: Vec<(usize, usize)>,
    pub directed: bool,
    pub matrix: Option<Vec<Vec<i64>>>,
    pub vertex_order: Vec<String>,
    pub start: Option<usize>,
    pub target: Option<usize>,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphSource {
    pub source_id: String,
    pub title: String,
    pub section: String,
    pub url: String,
    pub license: String,
    pub retrieved_utc: String,
    pub evidence_span: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphResult {
    pub status: GraphStatus,
    pub artifact: Option<GraphArtifact>,
    pub operation: GraphOperation,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub source: GraphSource,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn source() -> GraphSource {
    GraphSource {
        source_id: "mit-ocw-6-042j:finite-graphs".into(),
        title: "Mathematics for Computer Science".into(),
        section: "finite graphs and adjacency representations".into(),
        url: "https://ocw.mit.edu/courses/6-042j-mathematics-for-computer-science-fall-2010/"
            .into(),
        license: "CC BY-NC-SA 4.0; MIT attribution required".into(),
        retrieved_utc: "2026-08-05".into(),
        evidence_span: "finite vertex/edge definitions and adjacency matrix construction".into(),
    }
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("graph serializes"))
    )
}

fn replay_payload(result: &GraphResult) -> impl Serialize + '_ {
    (
        result.status,
        result.artifact.as_ref(),
        result.operation,
        &result.assumptions,
        &result.reasons,
        &result.source,
        &result.provenance,
    )
}

fn result(
    request: &GraphRequest,
    status: GraphStatus,
    artifact: Option<GraphArtifact>,
    assumptions: Vec<String>,
    reasons: Vec<String>,
) -> GraphResult {
    let mut result = GraphResult {
        status,
        artifact,
        operation: request.operation,
        assumptions,
        reasons,
        source: source(),
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    let replay_hash = digest(&replay_payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn validate_graph(request: &GraphRequest) -> Result<FiniteGraph, GraphStatus> {
    if request.vertices.is_empty() {
        return Err(GraphStatus::Missing);
    }
    let mut names = BTreeSet::new();
    if request.vertices.iter().any(|vertex| !names.insert(vertex)) {
        return Err(GraphStatus::InvalidGraph);
    }
    let mut seen = BTreeSet::new();
    for &(left, right) in &request.edges {
        if left >= request.vertices.len() || right >= request.vertices.len() || left == right {
            return Err(GraphStatus::InvalidGraph);
        }
        let key = if request.directed {
            (left, right)
        } else {
            (left.min(right), left.max(right))
        };
        if !seen.insert(key) {
            return Err(GraphStatus::InvalidGraph);
        }
    }
    Ok(FiniteGraph {
        vertices: request.vertices.clone(),
        edges: request.edges.clone(),
        directed: request.directed,
    })
}

fn adjacency(graph: &FiniteGraph) -> Vec<Vec<i64>> {
    let mut matrix = vec![vec![0; graph.vertices.len()]; graph.vertices.len()];
    for &(left, right) in &graph.edges {
        matrix[left][right] = 1;
        if !graph.directed {
            matrix[right][left] = 1;
        }
    }
    matrix
}

fn degrees(graph: &FiniteGraph) -> Vec<usize> {
    let mut degrees = vec![0; graph.vertices.len()];
    for &(left, right) in &graph.edges {
        degrees[left] += 1;
        if graph.directed {
            degrees[right] += 1;
        } else {
            degrees[right] += 1;
        }
    }
    degrees
}

fn reachable(graph: &FiniteGraph, start: usize, target: usize) -> Option<bool> {
    if start >= graph.vertices.len() || target >= graph.vertices.len() {
        return None;
    }
    let matrix = adjacency(graph);
    let mut seen = vec![false; graph.vertices.len()];
    let mut queue = VecDeque::from([start]);
    seen[start] = true;
    while let Some(current) = queue.pop_front() {
        if current == target {
            return Some(true);
        }
        for next in 0..graph.vertices.len() {
            if matrix[current][next] != 0 && !seen[next] {
                seen[next] = true;
                queue.push_back(next);
            }
        }
    }
    Some(false)
}

fn components(graph: &FiniteGraph) -> Vec<Vec<usize>> {
    let matrix = adjacency(graph);
    let mut visited = vec![false; graph.vertices.len()];
    let mut output = Vec::new();
    for root in 0..graph.vertices.len() {
        if visited[root] {
            continue;
        }
        let mut component = Vec::new();
        let mut queue = VecDeque::from([root]);
        visited[root] = true;
        while let Some(current) = queue.pop_front() {
            component.push(current);
            for next in 0..graph.vertices.len() {
                if (matrix[current][next] != 0 || matrix[next][current] != 0) && !visited[next] {
                    visited[next] = true;
                    queue.push_back(next);
                }
            }
        }
        component.sort_unstable();
        output.push(component);
    }
    output
}

fn incidence(graph: &FiniteGraph) -> Option<Vec<Vec<i64>>> {
    if graph.directed {
        return None;
    }
    let mut matrix = vec![vec![0; graph.edges.len()]; graph.vertices.len()];
    for (edge_index, &(left, right)) in graph.edges.iter().enumerate() {
        matrix[left][edge_index] = 1;
        matrix[right][edge_index] = 1;
    }
    Some(matrix)
}

fn graph_from_adjacency(request: &GraphRequest) -> Result<FiniteGraph, GraphStatus> {
    let matrix = request.matrix.as_ref().ok_or(GraphStatus::Missing)?;
    if request.vertex_order.is_empty() {
        return Err(GraphStatus::Ambiguous);
    }
    if matrix.len() != request.vertex_order.len()
        || matrix.iter().any(|row| row.len() != matrix.len())
    {
        return Err(GraphStatus::DimensionMismatch);
    }
    if matrix.iter().flatten().any(|value| ![0, 1].contains(value)) {
        return Err(GraphStatus::InvalidGraph);
    }
    if !request.directed
        && matrix.iter().enumerate().any(|(row, values)| {
            values
                .iter()
                .enumerate()
                .any(|(column, value)| *value != matrix[column][row])
        })
    {
        return Err(GraphStatus::Ambiguous);
    }
    let edges = matrix
        .iter()
        .enumerate()
        .flat_map(|(left, row)| {
            row.iter().enumerate().filter_map(move |(right, value)| {
                if *value == 1 && left != right && (request.directed || left < right) {
                    Some((left, right))
                } else {
                    None
                }
            })
        })
        .collect();
    Ok(FiniteGraph {
        vertices: request.vertex_order.clone(),
        edges,
        directed: request.directed,
    })
}

pub fn evaluate_graph(request: &GraphRequest) -> GraphResult {
    if request.domain != "finite_simple_graph" {
        return result(
            request,
            GraphStatus::Unsupported,
            None,
            vec![],
            vec!["domain is outside the finite simple graph boundary".into()],
        );
    }
    if let Some(ambiguity) = &request.ambiguity {
        return result(
            request,
            GraphStatus::Ambiguous,
            None,
            vec![],
            vec![ambiguity.clone()],
        );
    }
    let graph = if request.operation == GraphOperation::GraphFromAdjacency {
        match graph_from_adjacency(request) {
            Ok(graph) => graph,
            Err(status) => {
                return result(
                    request,
                    status,
                    None,
                    vec!["explicit vertex order".into()],
                    vec!["adjacency-to-graph reconstruction failed or is ambiguous".into()],
                )
            }
        }
    } else {
        match validate_graph(request) {
            Ok(graph) => graph,
            Err(status) => {
                return result(
                    request,
                    status,
                    None,
                    vec!["finite simple graph".into()],
                    vec!["graph is missing or violates simple finite graph invariants".into()],
                )
            }
        }
    };
    let assumptions = vec![
        "finite vertex set with stable identity".into(),
        "simple graph without loops or duplicate edges".into(),
    ];
    let (status, artifact, reasons) = match request.operation {
        GraphOperation::Construction => (
            GraphStatus::Complete,
            Some(GraphArtifact::Graph(graph)),
            Vec::new(),
        ),
        GraphOperation::EdgeCount => (
            GraphStatus::Complete,
            Some(GraphArtifact::Scalar(graph.edges.len())),
            Vec::new(),
        ),
        GraphOperation::Degrees => (
            GraphStatus::Complete,
            Some(GraphArtifact::Degrees(degrees(&graph))),
            Vec::new(),
        ),
        GraphOperation::Reachability => match (request.start, request.target) {
            (Some(start), Some(target)) => match reachable(&graph, start, target) {
                Some(value) => (
                    GraphStatus::Complete,
                    Some(GraphArtifact::Boolean(value)),
                    Vec::new(),
                ),
                None => (
                    GraphStatus::DimensionMismatch,
                    None,
                    vec!["reachability endpoints are outside the vertex set".into()],
                ),
            },
            _ => (
                GraphStatus::Missing,
                None,
                vec!["reachability requires explicit start and target vertices".into()],
            ),
        },
        GraphOperation::ConnectedComponents => {
            if graph.directed {
                (
                    GraphStatus::Unsupported,
                    None,
                    vec!["directed component semantics require an explicit policy".into()],
                )
            } else {
                (
                    GraphStatus::Complete,
                    Some(GraphArtifact::Components(components(&graph))),
                    Vec::new(),
                )
            }
        }
        GraphOperation::IsTree => {
            if graph.directed {
                (
                    GraphStatus::Unsupported,
                    None,
                    vec!["tree semantics are bounded to undirected graphs".into()],
                )
            } else {
                let is_tree = graph.edges.len() == graph.vertices.len().saturating_sub(1)
                    && components(&graph).len() == 1;
                (
                    GraphStatus::Complete,
                    Some(GraphArtifact::Boolean(is_tree)),
                    Vec::new(),
                )
            }
        }
        GraphOperation::AdjacencyMatrix => (
            GraphStatus::Complete,
            Some(GraphArtifact::Matrix(adjacency(&graph))),
            Vec::new(),
        ),
        GraphOperation::IncidenceMatrix => match incidence(&graph) {
            Some(matrix) => (
                GraphStatus::Complete,
                Some(GraphArtifact::Matrix(matrix)),
                Vec::new(),
            ),
            None => (
                GraphStatus::Unsupported,
                None,
                vec!["directed incidence orientation is not in this pack".into()],
            ),
        },
        GraphOperation::GraphFromAdjacency => (
            GraphStatus::Complete,
            Some(GraphArtifact::Graph(graph)),
            Vec::new(),
        ),
    };
    result(request, status, artifact, assumptions, reasons)
}

impl GraphResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&replay_payload(self))
            && !self.provenance.is_empty()
            && self.source.source_id.starts_with("mit-ocw-6-042j:")
            && (self.status != GraphStatus::Complete || self.artifact.is_some())
    }
}

/// Build a linear-algebra request only when graph identity and vertex order
/// remain explicit in provenance.
pub fn adjacency_to_linear_algebra(
    result: &GraphResult,
    directed: bool,
    vertex_order: &[String],
) -> Option<LinearAlgebraRequest> {
    let GraphArtifact::Matrix(matrix) = result.artifact.as_ref()? else {
        return None;
    };
    if vertex_order.is_empty() || matrix.len() != vertex_order.len() {
        return None;
    }
    Some(LinearAlgebraRequest {
        operation: LinearAlgebraOperation::MatrixConstruction,
        matrix: Some(matrix.clone()),
        vector_a: None,
        vector_b: None,
        domain: "finite_exact_integer".into(),
        requested_output: if directed {
            "directed_adjacency_matrix"
        } else {
            "undirected_adjacency_matrix"
        }
        .into(),
        provenance: result.provenance.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(operation: GraphOperation) -> GraphRequest {
        GraphRequest {
            operation,
            domain: "finite_simple_graph".into(),
            vertices: vec!["a".into(), "b".into(), "c".into()],
            edges: vec![(0, 1), (1, 2)],
            directed: false,
            matrix: None,
            vertex_order: Vec::new(),
            start: Some(0),
            target: Some(2),
            ambiguity: None,
            provenance: vec!["test".into()],
        }
    }

    #[test]
    fn finite_graph_operations_replay() {
        let result = evaluate_graph(&request(GraphOperation::IsTree));
        assert_eq!(result.artifact, Some(GraphArtifact::Boolean(true)));
        assert!(result.replay_verified());
        let matrix = evaluate_graph(&request(GraphOperation::AdjacencyMatrix));
        assert_eq!(matrix.status, GraphStatus::Complete);
        assert!(matrix.replay_verified());
    }

    #[test]
    fn graph_boundaries_fail_closed() {
        let mut duplicate = request(GraphOperation::Construction);
        duplicate.edges.push((1, 0));
        assert_eq!(evaluate_graph(&duplicate).status, GraphStatus::InvalidGraph);
        let mut directed = request(GraphOperation::ConnectedComponents);
        directed.directed = true;
        assert_eq!(evaluate_graph(&directed).status, GraphStatus::Unsupported);
    }
}
