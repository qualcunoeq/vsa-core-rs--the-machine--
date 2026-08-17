//! Explicit bridge from a validated simplicial complex to its one-skeleton.
//!
//! A simplex is not silently treated as a graph.  The caller must request the
//! `one_skeleton_graph` policy, and the bridge preserves vertex order,
//! provenance, and the distinction between a graph representation and the
//! higher-dimensional homology artifact.

use crate::graph_pack::{
    evaluate_graph, FiniteGraph, GraphArtifact, GraphOperation, GraphRequest, GraphResult,
    GraphStatus,
};
use crate::simplicial_homology_pack::{
    evaluate, HomologyArtifact, HomologyResult, HomologyStatus, SimplicialComplexRequest,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BridgeStatus {
    Complete,
    Ambiguous,
    Unsupported,
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OneSkeletonBridgeResult {
    pub status: BridgeStatus,
    pub homology_status: HomologyStatus,
    pub graph: Option<FiniteGraph>,
    pub graph_result: Option<GraphResult>,
    pub homology_result: HomologyResult,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn payload(result: &OneSkeletonBridgeResult) -> impl Serialize + '_ {
    (
        result.status,
        result.homology_status,
        &result.graph,
        &result.graph_result,
        &result.homology_result,
        &result.assumptions,
        &result.reasons,
        &result.provenance,
    )
}

fn output(
    status: BridgeStatus,
    homology_result: HomologyResult,
    graph: Option<FiniteGraph>,
    graph_result: Option<GraphResult>,
    assumptions: Vec<String>,
    reasons: Vec<String>,
    provenance: Vec<String>,
) -> OneSkeletonBridgeResult {
    let mut result = OneSkeletonBridgeResult {
        status,
        homology_status: homology_result.status,
        graph,
        graph_result,
        homology_result,
        assumptions,
        reasons,
        provenance,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

/// Build the loop-free undirected one-skeleton of a complete finite complex.
pub fn one_skeleton_graph(
    request: &SimplicialComplexRequest,
    policy: &str,
) -> OneSkeletonBridgeResult {
    let homology_result = evaluate(request);
    let mut provenance = request.provenance.clone();
    provenance.push("bridge:finite-simplicial-one-skeleton-graph".into());
    if policy != "one_skeleton_graph" {
        return output(
            BridgeStatus::Ambiguous,
            homology_result,
            None,
            None,
            Vec::new(),
            vec!["graph semantics require an explicit one-skeleton policy".into()],
            provenance,
        );
    }
    if homology_result.status != HomologyStatus::Complete {
        let status = match homology_result.status {
            HomologyStatus::Ambiguous => BridgeStatus::Ambiguous,
            HomologyStatus::Unsupported => BridgeStatus::Unsupported,
            _ => BridgeStatus::Invalid,
        };
        return output(
            status,
            homology_result,
            None,
            None,
            Vec::new(),
            vec!["the source complex did not produce a complete graph carrier".into()],
            provenance,
        );
    }
    let HomologyArtifact::ValidatedComplex {
        vertices,
        simplices_by_dimension,
        coefficient_field,
    } = homology_result
        .artifact
        .clone()
        .unwrap_or(HomologyArtifact::ValidatedComplex {
            vertices: Vec::new(),
            simplices_by_dimension: Vec::new(),
            coefficient_field: 0,
        })
    else {
        return output(
            BridgeStatus::Invalid,
            homology_result,
            None,
            None,
            Vec::new(),
            vec!["complex validation did not produce a typed carrier".into()],
            provenance,
        );
    };
    if coefficient_field != 2 {
        return output(
            BridgeStatus::Unsupported,
            homology_result,
            None,
            None,
            Vec::new(),
            vec!["the one-skeleton bridge requires the validated F_2 complex".into()],
            provenance,
        );
    }
    let edges = simplices_by_dimension
        .get(1)
        .into_iter()
        .flatten()
        .filter_map(|simplex| (simplex.len() == 2).then_some((simplex[0], simplex[1])))
        .collect::<Vec<_>>();
    let graph_request = GraphRequest {
        operation: GraphOperation::Construction,
        domain: "finite_simple_graph".into(),
        vertices: vertices.clone(),
        edges,
        directed: false,
        matrix: None,
        vertex_order: vertices.clone(),
        start: None,
        target: None,
        ambiguity: None,
        provenance: provenance.clone(),
    };
    let graph_result = evaluate_graph(&graph_request);
    let graph = match graph_result.artifact.clone() {
        Some(GraphArtifact::Graph(graph)) if graph_result.status == GraphStatus::Complete => graph,
        _ => {
            return output(
                BridgeStatus::Invalid,
                homology_result,
                None,
                Some(graph_result),
                Vec::new(),
                vec!["the one-skeleton was rejected by the finite graph boundary".into()],
                provenance,
            )
        }
    };
    output(
        BridgeStatus::Complete,
        homology_result,
        Some(graph),
        Some(graph_result),
        vec![
            "higher-dimensional simplices remain in the homology artifact".into(),
            "the graph contains only the explicit one-skeleton".into(),
        ],
        Vec::new(),
        provenance,
    )
}

impl OneSkeletonBridgeResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && self.homology_result.replay_verified()
            && self
                .graph_result
                .as_ref()
                .map(|result| result.replay_verified())
                .unwrap_or(self.status != BridgeStatus::Complete)
            && (self.status != BridgeStatus::Complete || self.graph.is_some())
    }

    pub fn authorized(&self) -> bool {
        self.status == BridgeStatus::Complete && self.replay_verified()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> SimplicialComplexRequest {
        SimplicialComplexRequest {
            operation: crate::simplicial_homology_pack::HomologyOperation::ValidateComplex,
            domain: "finite_simplicial_complex".into(),
            vertices: vec!["a".into(), "b".into(), "c".into()],
            simplices: vec![
                vec![0],
                vec![1],
                vec![2],
                vec![0, 1],
                vec![0, 2],
                vec![1, 2],
                vec![0, 1, 2],
            ],
            coefficient_field: Some(2),
            provenance: vec!["test".into()],
            ambiguity: None,
        }
    }

    #[test]
    fn explicit_policy_preserves_one_skeleton() {
        let result = one_skeleton_graph(&request(), "one_skeleton_graph");
        assert!(result.authorized());
        assert_eq!(result.graph.unwrap().edges.len(), 3);
    }

    #[test]
    fn missing_policy_is_ambiguous() {
        let result = one_skeleton_graph(&request(), "infer_graph");
        assert_eq!(result.status, BridgeStatus::Ambiguous);
        assert!(!result.authorized());
    }

    #[test]
    fn bridge_tampering_is_rejected() {
        let mut result = one_skeleton_graph(&request(), "one_skeleton_graph");
        assert!(result.replay_verified());
        result.replay_hash = "tampered".into();
        assert!(!result.replay_verified());
    }
}
