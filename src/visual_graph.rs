//! Conservative coordinate-preserving visual graph frontend.
//!
//! The frontend consumes candidate observations produced by an upstream visual
//! extractor.  It does not infer graph semantics from geometry alone: a graph
//! label, vertex identities, edge endpoints, direction policy, confidence, and
//! provenance must all be explicit before a typed graph artifact is emitted.
//! The resulting artifact can then be handed to the bounded finite-graph pack.

use crate::graph_pack::{FiniteGraph, GraphOperation, GraphRequest};
use crate::probability_pack::{ProbabilityResult, Rational};
use crate::random_walk_composition::{execute_one_step, RandomWalkResult, TransitionConvention};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const DOMAIN: &str = "visual_finite_graph";
const MAX_VERTICES: usize = 16;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VisualGraphStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    Invalid,
}

/// A coordinate-bearing candidate vertex from a visual extractor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualNodeObservation {
    pub label: String,
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
    /// Confidence is an observation quality value, not semantic certainty.
    pub confidence: u8,
}

/// A candidate edge whose endpoints are explicitly identified by the
/// extractor.  A line segment without endpoint identity is not an edge claim.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualEdgeObservation {
    pub from: String,
    pub to: String,
    pub directed: Option<bool>,
    pub confidence: u8,
}

/// Input to the visual graph formalizer.  These are observations, not yet a
/// graph assertion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualGraphObservation {
    pub semantic_label: Option<String>,
    pub nodes: Vec<VisualNodeObservation>,
    pub edges: Vec<VisualEdgeObservation>,
    pub directed: Option<bool>,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualGraphArtifact {
    pub vertex_order: Vec<String>,
    pub edges: Vec<(usize, usize)>,
    pub directed: bool,
    pub node_spans: Vec<String>,
    pub edge_spans: Vec<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualGraphResult {
    pub status: VisualGraphStatus,
    pub artifact: Option<VisualGraphArtifact>,
    pub alternatives: Vec<String>,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("visual graph serializes"))
    )
}

fn payload(result: &VisualGraphResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.artifact,
        &result.alternatives,
        &result.reasons,
    )
}

fn result(
    status: VisualGraphStatus,
    artifact: Option<VisualGraphArtifact>,
    alternatives: Vec<String>,
    reasons: Vec<String>,
) -> VisualGraphResult {
    let mut output = VisualGraphResult {
        status,
        artifact,
        alternatives,
        reasons,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&output));
    output.replay_hash = replay_hash;
    output
}

fn unsupported(reason: impl Into<String>) -> VisualGraphResult {
    result(
        VisualGraphStatus::Unsupported,
        None,
        Vec::new(),
        vec![reason.into()],
    )
}

/// Formalize only an explicitly identified simple finite graph.
pub fn formalize_visual_graph(input: &VisualGraphObservation) -> VisualGraphResult {
    if let Some(ambiguity) = &input.ambiguity {
        return result(
            VisualGraphStatus::Ambiguous,
            None,
            vec![ambiguity.clone()],
            vec!["visual extractor reported unresolved alternatives".into()],
        );
    }
    if input.provenance.is_empty() {
        return result(
            VisualGraphStatus::Missing,
            None,
            Vec::new(),
            vec!["coordinate observations need provenance".into()],
        );
    }
    if input.semantic_label.as_deref() != Some("finite_simple_graph") {
        return unsupported("visual geometry does not establish finite_simple_graph semantics");
    }
    if input.nodes.is_empty() {
        return result(
            VisualGraphStatus::Missing,
            None,
            Vec::new(),
            vec!["at least one explicit vertex is required".into()],
        );
    }
    if input.nodes.len() > MAX_VERTICES {
        return unsupported("visual graph exceeds the bounded vertex budget");
    }
    let Some(directed) = input.directed else {
        return result(
            VisualGraphStatus::Ambiguous,
            None,
            vec!["directed".into(), "undirected".into()],
            vec!["edge direction policy is not explicit".into()],
        );
    };
    let mut labels = BTreeSet::new();
    for node in &input.nodes {
        if node.label.trim().is_empty() {
            return result(
                VisualGraphStatus::Missing,
                None,
                Vec::new(),
                vec!["vertex labels must be explicit".into()],
            );
        }
        if node.width == 0 || node.height == 0 {
            return result(
                VisualGraphStatus::Missing,
                None,
                Vec::new(),
                vec!["vertex coordinates need positive bounding boxes".into()],
            );
        }
        if node.confidence < 80 {
            return result(
                VisualGraphStatus::Ambiguous,
                None,
                vec![node.label.clone()],
                vec!["a vertex observation is below the confidence boundary".into()],
            );
        }
        if !labels.insert(node.label.clone()) {
            return result(
                VisualGraphStatus::Invalid,
                None,
                Vec::new(),
                vec!["duplicate visual vertex labels are not identity-safe".into()],
            );
        }
    }
    let vertex_order: Vec<String> = input.nodes.iter().map(|node| node.label.clone()).collect();
    let mut edges = Vec::new();
    let mut seen = BTreeSet::new();
    for edge in &input.edges {
        let Some(from) = vertex_order.iter().position(|label| label == &edge.from) else {
            return result(
                VisualGraphStatus::Invalid,
                None,
                Vec::new(),
                vec![format!(
                    "edge references unknown source vertex {}",
                    edge.from
                )],
            );
        };
        let Some(to) = vertex_order.iter().position(|label| label == &edge.to) else {
            return result(
                VisualGraphStatus::Invalid,
                None,
                Vec::new(),
                vec![format!("edge references unknown target vertex {}", edge.to)],
            );
        };
        if from == to {
            return unsupported("self-loops are outside the finite simple-graph boundary");
        }
        if edge.directed.is_some_and(|value| value != directed) {
            return result(
                VisualGraphStatus::Ambiguous,
                None,
                vec!["directed".into(), "undirected".into()],
                vec!["edge-level direction conflicts with graph direction".into()],
            );
        }
        if edge.confidence < 80 {
            return result(
                VisualGraphStatus::Ambiguous,
                None,
                vec![format!("{} -> {}", edge.from, edge.to)],
                vec!["an edge observation is below the confidence boundary".into()],
            );
        }
        let key = if directed {
            (from, to)
        } else {
            (from.min(to), from.max(to))
        };
        if !seen.insert(key) {
            return result(
                VisualGraphStatus::Invalid,
                None,
                Vec::new(),
                vec!["duplicate visual edges are not identity-safe".into()],
            );
        }
        edges.push(key);
    }
    edges.sort_unstable();
    let node_spans = input
        .nodes
        .iter()
        .map(|node| {
            format!(
                "node:{}@{},{},{},{}",
                node.label, node.left, node.top, node.width, node.height
            )
        })
        .collect::<Vec<_>>();
    let edge_spans = input
        .edges
        .iter()
        .map(|edge| format!("edge:{}->{}", edge.from, edge.to))
        .collect::<Vec<_>>();
    result(
        VisualGraphStatus::Complete,
        Some(VisualGraphArtifact {
            vertex_order,
            edges,
            directed,
            node_spans,
            edge_spans,
            provenance: input.provenance.clone(),
        }),
        Vec::new(),
        Vec::new(),
    )
}

/// Lower only a complete visual artifact into the finite graph pack.
pub fn to_graph_request(result: &VisualGraphResult) -> Option<GraphRequest> {
    let graph = to_finite_graph(result)?;
    let artifact = result.artifact.as_ref()?;
    Some(GraphRequest {
        operation: GraphOperation::Construction,
        domain: "finite_simple_graph".into(),
        vertices: graph.vertices,
        edges: graph.edges,
        directed: graph.directed,
        matrix: None,
        vertex_order: artifact.vertex_order.clone(),
        start: None,
        target: None,
        ambiguity: None,
        provenance: artifact.provenance.clone(),
    })
}

/// Convert only a complete visual artifact into graph identity.  This helper
/// is intentionally separate from stochastic semantics.
pub fn to_finite_graph(result: &VisualGraphResult) -> Option<FiniteGraph> {
    let artifact = result.artifact.as_ref()?;
    (result.status == VisualGraphStatus::Complete).then(|| FiniteGraph {
        vertices: artifact.vertex_order.clone(),
        edges: artifact.edges.clone(),
        directed: artifact.directed,
    })
}

/// Compose a complete visual graph with an independently validated finite
/// probability distribution and an explicitly declared transition matrix.
/// Shape alone never establishes a random walk; the caller must supply exact
/// row/column semantics and `explicit_semantics = true` at the trusted bridge.
pub fn execute_one_step_random_walk(
    visual: &VisualGraphResult,
    transition: Option<&[Vec<Rational>]>,
    initial: &ProbabilityResult,
    convention: Option<TransitionConvention>,
    provenance: Vec<String>,
) -> Option<RandomWalkResult> {
    let graph = to_finite_graph(visual)?;
    let artifact = visual.artifact.as_ref()?;
    Some(execute_one_step(
        &graph,
        transition,
        initial,
        &artifact.vertex_order,
        convention,
        true,
        1,
        provenance,
    ))
}

impl VisualGraphResult {
    pub fn authorized(&self) -> bool {
        self.status == VisualGraphStatus::Complete && self.artifact.is_some()
    }

    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && (!self.authorized()
                || self
                    .artifact
                    .as_ref()
                    .is_some_and(|artifact| !artifact.provenance.is_empty()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn supported() -> VisualGraphObservation {
        VisualGraphObservation {
            semantic_label: Some("finite_simple_graph".into()),
            nodes: vec![
                VisualNodeObservation {
                    label: "a".into(),
                    left: 10,
                    top: 10,
                    width: 20,
                    height: 20,
                    confidence: 99,
                },
                VisualNodeObservation {
                    label: "b".into(),
                    left: 50,
                    top: 10,
                    width: 20,
                    height: 20,
                    confidence: 99,
                },
            ],
            edges: vec![VisualEdgeObservation {
                from: "a".into(),
                to: "b".into(),
                directed: Some(false),
                confidence: 99,
            }],
            directed: Some(false),
            ambiguity: None,
            provenance: vec!["diagram:test".into()],
        }
    }

    #[test]
    fn complete_visual_graph_lowers_and_replays() {
        let visual = formalize_visual_graph(&supported());
        assert!(visual.authorized());
        assert!(visual.replay_verified());
        assert!(to_graph_request(&visual).is_some());
    }

    #[test]
    fn missing_direction_stays_ambiguous() {
        let mut input = supported();
        input.directed = None;
        let result = formalize_visual_graph(&input);
        assert_eq!(result.status, VisualGraphStatus::Ambiguous);
        assert!(!result.authorized());
    }

    #[test]
    fn geometry_without_graph_semantics_is_refused() {
        let mut input = supported();
        input.semantic_label = Some("diagram".into());
        let result = formalize_visual_graph(&input);
        assert_eq!(result.status, VisualGraphStatus::Unsupported);
        assert!(!result.authorized());
    }

    #[test]
    fn tampering_rejects_visual_receipt() {
        let mut result = formalize_visual_graph(&supported());
        result.replay_hash.push('x');
        assert!(!result.replay_verified());
    }
}
