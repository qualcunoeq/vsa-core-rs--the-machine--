//! Explicit graph/probability composition for target-before-avoid events.

use crate::finite_markov_hitting_pack::{
    evaluate as evaluate_hitting, HittingArtifact, HittingRequest, HittingStatus,
};
use crate::graph_pack::{evaluate_graph, FiniteGraph, GraphArtifact, GraphOperation, GraphRequest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HittingCompositionStatus {
    Complete,
    Ambiguous,
    Unsupported,
    InvalidGraph,
    IncompatibleSemantics,
    NonUniqueHitting,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HittingCompositionRequest {
    pub graph: GraphRequest,
    pub hitting: HittingRequest,
    pub allow_self_transitions: Option<bool>,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HittingCompositionResult {
    pub status: HittingCompositionStatus,
    pub graph: Option<FiniteGraph>,
    pub adjacency: Option<Vec<Vec<i64>>>,
    pub hitting: Option<HittingArtifact>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("hitting composition serializes"))
    )
}

fn payload(result: &HittingCompositionResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.graph,
        &result.adjacency,
        &result.hitting,
        &result.reasons,
        &result.provenance,
    )
}

fn finish(
    request: &HittingCompositionRequest,
    status: HittingCompositionStatus,
    graph: Option<FiniteGraph>,
    adjacency: Option<Vec<Vec<i64>>>,
    hitting: Option<HittingArtifact>,
    reasons: Vec<String>,
) -> HittingCompositionResult {
    let mut result = HittingCompositionResult {
        status,
        graph,
        adjacency,
        hitting,
        reasons,
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    result.replay_hash = digest(&(
        result.status,
        result.graph.clone(),
        result.adjacency.clone(),
        result.hitting.clone(),
        result.reasons.clone(),
        result.provenance.clone(),
    ));
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

pub fn evaluate(request: &HittingCompositionRequest) -> HittingCompositionResult {
    if request.ambiguity.is_some() || request.allow_self_transitions != Some(true) {
        return finish(
            request,
            HittingCompositionStatus::Ambiguous,
            None,
            None,
            None,
            vec!["graph/event ambiguity or self-transition policy is unresolved".into()],
        );
    }
    let graph_result = evaluate_graph(&GraphRequest {
        operation: GraphOperation::Construction,
        ..request.graph.clone()
    });
    if !graph_result.replay_verified() {
        return finish(
            request,
            HittingCompositionStatus::InvalidGraph,
            None,
            None,
            None,
            vec!["graph replay receipt is invalid".into()],
        );
    }
    let Some(GraphArtifact::Graph(graph)) = graph_result.artifact else {
        return finish(
            request,
            HittingCompositionStatus::InvalidGraph,
            None,
            None,
            None,
            vec!["graph construction did not produce a typed graph".into()],
        );
    };
    if !graph.directed || graph.vertices != request.graph.vertex_order {
        return finish(
            request,
            HittingCompositionStatus::IncompatibleSemantics,
            Some(graph),
            None,
            None,
            vec!["directed graph identity and stable vertex order are required".into()],
        );
    }
    let adjacency_result = evaluate_graph(&adjacency_request(&request.graph));
    if !adjacency_result.replay_verified() {
        return finish(
            request,
            HittingCompositionStatus::InvalidGraph,
            Some(graph),
            None,
            None,
            vec!["adjacency replay receipt is invalid".into()],
        );
    }
    let Some(GraphArtifact::Matrix(adjacency)) = adjacency_result.artifact else {
        return finish(
            request,
            HittingCompositionStatus::InvalidGraph,
            Some(graph),
            None,
            None,
            vec!["adjacency artifact is unavailable".into()],
        );
    };
    if request.hitting.transition.len() != graph.vertices.len()
        || request
            .hitting
            .transition
            .iter()
            .any(|row| row.len() != graph.vertices.len())
    {
        return finish(
            request,
            HittingCompositionStatus::IncompatibleSemantics,
            Some(graph),
            Some(adjacency),
            None,
            vec!["transition dimensions do not match graph vertex order".into()],
        );
    }
    let edges: BTreeSet<(usize, usize)> = graph.edges.iter().copied().collect();
    for (source, row) in request.hitting.transition.iter().enumerate() {
        for (target, probability) in row.iter().enumerate() {
            if source != target && probability.numerator != 0 && !edges.contains(&(source, target))
            {
                return finish(
                    request,
                    HittingCompositionStatus::IncompatibleSemantics,
                    Some(graph),
                    Some(adjacency),
                    None,
                    vec!["positive transition support is not present in the directed graph".into()],
                );
            }
        }
    }
    let hitting = evaluate_hitting(&request.hitting);
    let status = match hitting.status {
        HittingStatus::Complete => HittingCompositionStatus::Complete,
        HittingStatus::Ambiguous => HittingCompositionStatus::Ambiguous,
        HittingStatus::NonUnique => HittingCompositionStatus::NonUniqueHitting,
        _ => HittingCompositionStatus::Unsupported,
    };
    let reasons = if status == HittingCompositionStatus::Complete {
        Vec::new()
    } else {
        hitting.reasons
    };
    finish(
        request,
        status,
        Some(graph),
        Some(adjacency),
        hitting.artifact,
        reasons,
    )
}

impl HittingCompositionResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self)) && !self.provenance.is_empty()
    }
}
