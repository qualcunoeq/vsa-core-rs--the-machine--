//! Explicit graph/probability/algebra composition for stationary distributions.
//!
//! A graph is used only to validate support and stable vertex identity. The
//! stationary solver still receives a separately validated row-stochastic
//! matrix; graph structure never silently becomes probabilistic semantics.

use crate::finite_markov_stationary_pack::{
    evaluate as evaluate_stationary, StationaryArtifact, StationaryRequest, StationaryStatus,
};
use crate::graph_pack::{evaluate_graph, FiniteGraph, GraphArtifact, GraphOperation, GraphRequest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompositionStatus {
    Complete,
    Ambiguous,
    Unsupported,
    InvalidGraph,
    IncompatibleSemantics,
    NonUniqueStationary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompositionRequest {
    pub graph: GraphRequest,
    pub transition: StationaryRequest,
    pub allow_self_transitions: Option<bool>,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompositionResult {
    pub status: CompositionStatus,
    pub graph: Option<FiniteGraph>,
    pub adjacency: Option<Vec<Vec<i64>>>,
    pub stationary: Option<StationaryArtifact>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("composition serializes"))
    )
}

fn payload(result: &CompositionResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.graph,
        &result.adjacency,
        &result.stationary,
        &result.reasons,
        &result.provenance,
    )
}

fn finish(
    request: &CompositionRequest,
    status: CompositionStatus,
    graph: Option<FiniteGraph>,
    adjacency: Option<Vec<Vec<i64>>>,
    stationary: Option<StationaryArtifact>,
    reasons: Vec<String>,
) -> CompositionResult {
    let mut result = CompositionResult {
        status,
        graph,
        adjacency,
        stationary,
        reasons,
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    let hash = digest(&(
        result.status,
        result.graph.clone(),
        result.adjacency.clone(),
        result.stationary.clone(),
        result.reasons.clone(),
        result.provenance.clone(),
    ));
    result.replay_hash = hash;
    result
}

fn adjacency_request(graph: &GraphRequest) -> GraphRequest {
    GraphRequest {
        operation: GraphOperation::AdjacencyMatrix,
        domain: graph.domain.clone(),
        vertices: graph.vertices.clone(),
        edges: graph.edges.clone(),
        directed: graph.directed,
        matrix: None,
        vertex_order: graph.vertex_order.clone(),
        start: None,
        target: None,
        ambiguity: graph.ambiguity.clone(),
        provenance: graph.provenance.clone(),
    }
}

/// Compose a directed finite graph with a separately typed stationary matrix.
pub fn evaluate(request: &CompositionRequest) -> CompositionResult {
    if let Some(ambiguity) = &request.ambiguity {
        return finish(
            request,
            CompositionStatus::Ambiguous,
            None,
            None,
            None,
            vec![ambiguity.clone()],
        );
    }
    if request.allow_self_transitions != Some(true) {
        return finish(
            request,
            CompositionStatus::Ambiguous,
            None,
            None,
            None,
            vec!["self-transition policy must be explicit".into()],
        );
    }
    let graph_result = evaluate_graph(&GraphRequest {
        operation: GraphOperation::Construction,
        ..request.graph.clone()
    });
    if !graph_result.replay_verified() {
        return finish(
            request,
            CompositionStatus::InvalidGraph,
            None,
            None,
            None,
            vec!["graph replay receipt is invalid".into()],
        );
    }
    let Some(GraphArtifact::Graph(graph)) = graph_result.artifact else {
        return finish(
            request,
            CompositionStatus::InvalidGraph,
            None,
            None,
            None,
            vec!["graph construction did not produce a typed graph".into()],
        );
    };
    if !graph.directed || graph.vertices != request.graph.vertex_order {
        return finish(
            request,
            CompositionStatus::IncompatibleSemantics,
            Some(graph),
            None,
            None,
            vec!["directed graph identity and vertex order are required".into()],
        );
    }
    let adjacency_result = evaluate_graph(&adjacency_request(&request.graph));
    if !adjacency_result.replay_verified() {
        return finish(
            request,
            CompositionStatus::InvalidGraph,
            Some(graph),
            None,
            None,
            vec!["adjacency replay receipt is invalid".into()],
        );
    }
    let Some(GraphArtifact::Matrix(adjacency)) = adjacency_result.artifact else {
        return finish(
            request,
            CompositionStatus::InvalidGraph,
            Some(graph),
            None,
            None,
            vec!["graph did not produce an adjacency matrix".into()],
        );
    };
    if request.transition.transition.len() != graph.vertices.len()
        || request
            .transition
            .transition
            .iter()
            .any(|row| row.len() != graph.vertices.len())
    {
        return finish(
            request,
            CompositionStatus::IncompatibleSemantics,
            Some(graph),
            Some(adjacency),
            None,
            vec!["transition dimensions do not match graph vertex order".into()],
        );
    }
    let edges: BTreeSet<(usize, usize)> = graph.edges.iter().copied().collect();
    for (source, row) in request.transition.transition.iter().enumerate() {
        for (target, probability) in row.iter().enumerate() {
            if source != target && probability.numerator != 0 && !edges.contains(&(source, target))
            {
                return finish(
                    request,
                    CompositionStatus::IncompatibleSemantics,
                    Some(graph),
                    Some(adjacency),
                    None,
                    vec!["positive transition support is not present in the directed graph".into()],
                );
            }
        }
    }
    let stationary = evaluate_stationary(&request.transition);
    let status = match stationary.status {
        StationaryStatus::Complete => CompositionStatus::Complete,
        StationaryStatus::Ambiguous => CompositionStatus::Ambiguous,
        StationaryStatus::NonUnique => CompositionStatus::NonUniqueStationary,
        _ => CompositionStatus::Unsupported,
    };
    let reasons = if status == CompositionStatus::Complete {
        Vec::new()
    } else {
        stationary.reasons
    };
    finish(
        request,
        status,
        Some(graph),
        Some(adjacency),
        stationary.artifact,
        reasons,
    )
}

impl CompositionResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self)) && !self.provenance.is_empty()
    }
}
