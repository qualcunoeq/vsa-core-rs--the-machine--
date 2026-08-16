//! Explicit bridge from a finite topology's strict specialization relation to
//! a directed graph.
//!
//! The bridge is intentionally opt-in: a topology is not silently treated as
//! a graph, and reflexive preorder edges are omitted because the destination
//! graph pack models loop-free simple graphs. The vertex ordering and source
//! provenance are preserved in the resulting graph request.

use crate::graph_pack::{evaluate_graph, FiniteGraph, GraphArtifact, GraphOperation, GraphRequest, GraphResult, GraphStatus};
use crate::source_topology_pack::{evaluate_topology, TopologyArtifact, TopologyDefinitionRecord, TopologyRequest, TopologyStatus};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TopologyGraphBridgeResult {
    pub status: GraphStatus,
    pub topology_status: TopologyStatus,
    pub graph: Option<FiniteGraph>,
    pub graph_result: Option<GraphResult>,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).expect("topology graph bridge serializes")))
}

fn payload(result: &TopologyGraphBridgeResult) -> impl Serialize + '_ {
    (
        result.status,
        result.topology_status,
        &result.graph,
        &result.graph_result,
        &result.assumptions,
        &result.reasons,
        &result.provenance,
    )
}

fn output(
    status: GraphStatus,
    topology_status: TopologyStatus,
    graph: Option<FiniteGraph>,
    graph_result: Option<GraphResult>,
    assumptions: Vec<String>,
    reasons: Vec<String>,
    provenance: Vec<String>,
) -> TopologyGraphBridgeResult {
    let mut result = TopologyGraphBridgeResult {
        status,
        topology_status,
        graph,
        graph_result,
        assumptions,
        reasons,
        provenance,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

/// Build a graph from the strict specialization relation of a validated
/// finite topology. The caller must explicitly request this bridge by using a
/// graph-construction operation and the `strict_specialization_graph` policy.
pub fn topology_to_graph(
    topology_request: &TopologyRequest,
    records: &[TopologyDefinitionRecord],
    policy: &str,
) -> TopologyGraphBridgeResult {
    let topology_result = evaluate_topology(topology_request, records);
    let mut provenance = topology_request.provenance.clone();
    provenance.push("bridge:finite-topology-strict-specialization-graph".into());
    if policy != "strict_specialization_graph" {
        return output(
            GraphStatus::Ambiguous,
            topology_result.status,
            None,
            None,
            Vec::new(),
            vec!["graph semantics require an explicit strict-specialization policy".into()],
            provenance,
        );
    }
    let TopologyArtifact::ValidatedTopology { points, open_sets } = topology_result.artifact.clone().unwrap_or(TopologyArtifact::ValidatedTopology { points: Vec::new(), open_sets: Vec::new() }) else {
        return output(
            GraphStatus::Unsupported,
            topology_result.status,
            None,
            None,
            topology_result.assumptions.clone(),
            vec!["topology validation did not produce a carrier and open-set family".into()],
            provenance,
        );
    };
    if topology_result.status != TopologyStatus::Complete {
        return output(
            GraphStatus::Unsupported,
            topology_result.status,
            None,
            None,
            topology_result.assumptions.clone(),
            vec!["only a complete finite topology can be bridged".into()],
            provenance,
        );
    }
    let mut edges = Vec::new();
    for left in 0..points.len() {
        for right in 0..points.len() {
            if left == right {
                continue;
            }
            let relation_holds = open_sets.iter().all(|open| {
                !open.binary_search(&points[left]).is_ok() || open.binary_search(&points[right]).is_ok()
            });
            if relation_holds {
                edges.push((left, right));
            }
        }
    }
    let graph_request = GraphRequest {
        operation: GraphOperation::Construction,
        domain: "finite_simple_graph".into(),
        vertices: points.clone(),
        edges,
        directed: true,
        matrix: None,
        vertex_order: points.clone(),
        start: None,
        target: None,
        ambiguity: None,
        provenance: provenance.clone(),
    };
    let graph_result = evaluate_graph(&graph_request);
    let graph = match graph_result.artifact.clone() {
        Some(GraphArtifact::Graph(graph)) if graph_result.status == GraphStatus::Complete => graph,
        _ => return output(GraphStatus::InvalidGraph, topology_result.status, None, Some(graph_result), topology_result.assumptions.clone(), vec!["specialization relation was not accepted by the finite graph boundary".into()], provenance),
    };
    output(
        GraphStatus::Complete,
        topology_result.status,
        Some(graph),
        Some(graph_result),
        vec!["strict specialization relation; reflexive loops omitted".into()],
        Vec::new(),
        provenance,
    )
}

impl TopologyGraphBridgeResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != GraphStatus::Complete || self.graph.is_some())
            && self.graph_result.as_ref().map(|result| result.replay_verified()).unwrap_or(self.status != GraphStatus::Complete)
    }

    pub fn authorized(&self) -> bool {
        self.status == GraphStatus::Complete && self.replay_verified()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn records() -> Vec<TopologyDefinitionRecord> {
        let document = include_str!("../docs/sources/topology_without_tears_finite_definition.txt");
        crate::source_topology_pack::extract_topology_definitions(document).unwrap()
    }

    #[test]
    fn explicit_specialization_policy_produces_replayable_graph() {
        let request = TopologyRequest {
            operation: crate::source_topology_pack::TopologyOperation::ValidateTopology,
            topology: "finite_topology_axioms".into(),
            points: vec!["a".into(), "b".into(), "c".into()],
            open_sets: vec![Vec::new(), vec!["a".into()], vec!["a".into(), "b".into(), "c".into()]],
            target_set: None,
            domain: "source_derived_finite_topology".into(),
            ambiguity: None,
            provenance: vec!["test".into()],
        };
        let result = topology_to_graph(&request, &records(), "strict_specialization_graph");
        assert!(result.authorized());
        assert_eq!(result.graph.as_ref().unwrap().vertices.len(), 3);
    }

    #[test]
    fn missing_policy_preserves_ambiguity() {
        let request = TopologyRequest {
            operation: crate::source_topology_pack::TopologyOperation::ValidateTopology,
            topology: "finite_topology_axioms".into(),
            points: vec!["a".into(), "b".into()],
            open_sets: vec![Vec::new(), vec!["a".into(), "b".into()]],
            target_set: None,
            domain: "source_derived_finite_topology".into(),
            ambiguity: None,
            provenance: vec!["test".into()],
        };
        let result = topology_to_graph(&request, &records(), "infer_graph_semantics");
        assert_eq!(result.status, GraphStatus::Ambiguous);
        assert!(!result.authorized());
    }
}
