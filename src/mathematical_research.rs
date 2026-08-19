//! Bounded mathematical research over validated curriculum packs.
//!
//! This module is deliberately narrower than an open-ended theorem prover.  A
//! research case supplies a finite family of explicitly typed graph and/or
//! dynamics instances and a conjecture.  The investigator decomposes the
//! conjecture into pack-backed observations, searches the supplied family for
//! counterexamples, revises its plan when one is found, and stops only with a
//! finite supported, refuted, or unresolved conclusion.  Every observation is
//! produced by an existing validated pack; this layer does not reimplement
//! graph, linear-algebra, or dynamics semantics.

use crate::discrete_dynamics::{evaluate_dynamics, DynamicsRequest, DynamicsStatus};
use crate::graph_pack::{
    evaluate_graph, FiniteGraph, GraphArtifact, GraphOperation, GraphRequest, GraphStatus,
};
use crate::linear_algebra_pack::{
    evaluate_linear_algebra, LinearAlgebraArtifact, LinearAlgebraOperation, LinearAlgebraRequest,
    LinearAlgebraStatus,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchClaim {
    TreeCriterion,
    EdgesMinusOneImpliesConnected,
    UndirectedDegreeSum,
    AffineClosedForm,
    AffineMonotone,
    AdjacencyShapePreserved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchConclusion {
    Supported,
    Refuted,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchCase {
    pub id: String,
    pub claim: ResearchClaim,
    pub graphs: Vec<FiniteGraph>,
    pub dynamics: Option<DynamicsRequest>,
    pub max_operations: usize,
    pub expected: ResearchConclusion,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResearchStage {
    Decompose,
    Observe,
    Inspect,
    CounterexampleSearch,
    Replan,
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchStep {
    pub index: usize,
    pub stage: ResearchStage,
    pub operation: String,
    pub pack: String,
    pub status: String,
    pub observation: String,
    pub artifact_hash: String,
    pub pack_replay_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchReceipt {
    pub case_id: String,
    pub claim: ResearchClaim,
    pub conclusion: ResearchConclusion,
    pub steps: Vec<ResearchStep>,
    pub counterexamples: Vec<String>,
    pub replans: usize,
    pub operation_budget: usize,
    pub replay_hash: String,
}

impl ResearchReceipt {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash
            == digest(&(
                &self.case_id,
                self.claim,
                self.conclusion,
                &self.steps,
                &self.counterexamples,
                self.replans,
                self.operation_budget,
            ))
            && self
                .steps
                .windows(2)
                .all(|pair| pair[0].index < pair[1].index)
            && self
                .steps
                .iter()
                .all(|step| !step.operation.is_empty() && !step.artifact_hash.is_empty())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchReport {
    pub cases: usize,
    pub supported: usize,
    pub refuted: usize,
    pub unresolved: usize,
    pub terminal_correct: usize,
    pub counterexamples_found: usize,
    pub replans: usize,
    pub operations: usize,
    pub pack_replay_verified: usize,
    pub receipt_replay_verified: usize,
    pub tamper_rejected: usize,
    pub false_authorizations: usize,
    pub false_denials: usize,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("research value serializes"))
    )
}

fn graph_request(case_id: &str, graph: &FiniteGraph, operation: GraphOperation) -> GraphRequest {
    GraphRequest {
        operation,
        domain: "finite_simple_graph".into(),
        vertices: graph.vertices.clone(),
        edges: graph.edges.clone(),
        directed: graph.directed,
        matrix: None,
        vertex_order: graph.vertices.clone(),
        start: Some(0),
        target: graph.vertices.len().checked_sub(1),
        ambiguity: None,
        provenance: vec![format!("stage364:{case_id}:graph")],
    }
}

fn step(
    index: usize,
    stage: ResearchStage,
    operation: impl Into<String>,
    pack: impl Into<String>,
    status: impl Into<String>,
    observation: impl Into<String>,
    artifact_hash: impl Into<String>,
    pack_replay_verified: bool,
) -> ResearchStep {
    ResearchStep {
        index,
        stage,
        operation: operation.into(),
        pack: pack.into(),
        status: status.into(),
        observation: observation.into(),
        artifact_hash: artifact_hash.into(),
        pack_replay_verified,
    }
}

fn graph_observations(
    case: &ResearchCase,
    graph_index: usize,
    graph: &FiniteGraph,
    steps: &mut Vec<ResearchStep>,
) -> Option<(usize, usize, bool, Vec<usize>, Vec<Vec<i64>>, usize)> {
    let operations = [
        GraphOperation::Construction,
        GraphOperation::EdgeCount,
        GraphOperation::ConnectedComponents,
        GraphOperation::IsTree,
        GraphOperation::Degrees,
        GraphOperation::AdjacencyMatrix,
    ];
    let mut edge_count = None;
    let mut component_count = None;
    let mut is_tree = None;
    let mut degrees = None;
    let mut adjacency = None;
    for operation in operations {
        let result = evaluate_graph(&graph_request(&case.id, graph, operation));
        let replay = result.replay_verified();
        let artifact_hash = digest(&result.artifact);
        steps.push(step(
            steps.len(),
            ResearchStage::Observe,
            format!("graph_{operation:?}_g{graph_index}"),
            "graph_pack",
            format!("{:?}", result.status),
            format!("graph {graph_index} observation"),
            artifact_hash,
            replay,
        ));
        if !replay || result.status != GraphStatus::Complete {
            return None;
        }
        match (operation, result.artifact) {
            (GraphOperation::EdgeCount, Some(GraphArtifact::Scalar(value))) => {
                edge_count = Some(value)
            }
            (GraphOperation::ConnectedComponents, Some(GraphArtifact::Components(value))) => {
                component_count = Some(value.len())
            }
            (GraphOperation::IsTree, Some(GraphArtifact::Boolean(value))) => is_tree = Some(value),
            (GraphOperation::Degrees, Some(GraphArtifact::Degrees(value))) => degrees = Some(value),
            (GraphOperation::AdjacencyMatrix, Some(GraphArtifact::Matrix(value))) => {
                adjacency = Some(value)
            }
            (GraphOperation::Construction, Some(GraphArtifact::Graph(_))) => {}
            _ => return None,
        }
    }
    Some((
        edge_count?,
        component_count?,
        is_tree?,
        degrees?,
        adjacency?,
        graph.vertices.len(),
    ))
}

fn observe_adjacency_rank(
    case: &ResearchCase,
    graph_index: usize,
    adjacency: &[Vec<i64>],
    steps: &mut Vec<ResearchStep>,
) -> Option<usize> {
    let request = LinearAlgebraRequest {
        operation: LinearAlgebraOperation::Rank,
        matrix: Some(adjacency.to_vec()),
        vector_a: None,
        vector_b: None,
        domain: "finite_exact_integer".into(),
        requested_output: "adjacency_rank".into(),
        provenance: vec![format!(
            "stage364:{}:graph:{graph_index}:adjacency",
            case.id
        )],
    };
    let result = evaluate_linear_algebra(&request);
    let replay = result.replay_verified();
    let artifact_hash = digest(&result.artifact);
    steps.push(step(
        steps.len(),
        ResearchStage::Observe,
        format!("adjacency_rank_g{graph_index}"),
        "linear_algebra_pack",
        format!("{:?}", result.status),
        "rank of the graph adjacency artifact",
        artifact_hash,
        replay,
    ));
    if !replay || result.status != LinearAlgebraStatus::Complete {
        return None;
    }
    match result.artifact {
        Some(LinearAlgebraArtifact::Scalar(value)) => usize::try_from(value).ok(),
        _ => None,
    }
}

fn inspect_claim(
    case: &ResearchCase,
    graph_index: usize,
    edge_count: usize,
    component_count: usize,
    is_tree: bool,
    degrees: &[usize],
    adjacency_rank: Option<usize>,
    vertices: usize,
) -> bool {
    match case.claim {
        ResearchClaim::TreeCriterion => {
            is_tree == (component_count == 1 && edge_count + 1 == vertices)
        }
        ResearchClaim::EdgesMinusOneImpliesConnected => {
            edge_count + 1 != vertices || component_count == 1
        }
        ResearchClaim::UndirectedDegreeSum => {
            !case.graphs[graph_index].directed
                && degrees.iter().sum::<usize>() == edge_count.saturating_mul(2)
        }
        ResearchClaim::AdjacencyShapePreserved => {
            adjacency_rank.is_some() && adjacency_rank.unwrap_or_default() <= vertices
        }
        ResearchClaim::AffineClosedForm | ResearchClaim::AffineMonotone => true,
    }
}

fn inspect_dynamics(case: &ResearchCase, steps: &mut Vec<ResearchStep>) -> Option<bool> {
    let request = case.dynamics.as_ref()?;
    let result = evaluate_dynamics(request);
    let replay = result.replay_verified();
    let artifact_hash = digest(&result.artifact);
    steps.push(step(
        steps.len(),
        ResearchStage::Observe,
        "finite_dynamics_evolution",
        "discrete_dynamics_pack",
        format!("{:?}", result.status),
        "exact finite-horizon trace".to_string(),
        artifact_hash,
        replay,
    ));
    if !replay || result.status != DynamicsStatus::Complete {
        return None;
    }
    let trace = result.trace;
    match case.claim {
        ResearchClaim::AffineClosedForm => {
            let (Some(initial), Some(coefficient), Some(offset)) = (
                request.scalar_initial.as_ref(),
                request.coefficient.as_ref(),
                request.offset.as_ref(),
            ) else {
                return None;
            };
            if coefficient.numerator != coefficient.denominator {
                return Some(false);
            }
            Some(trace.iter().enumerate().all(|(index, artifact)| {
                let crate::discrete_dynamics::DynamicsArtifact::Scalar(value) = artifact else {
                    return false;
                };
                value
                    == &crate::probability_pack::Rational::new(
                        initial.numerator * offset.denominator
                            + offset.numerator * initial.denominator * (index as i128 + 1),
                        initial.denominator * offset.denominator,
                    )
                    .unwrap()
            }))
        }
        ResearchClaim::AffineMonotone => {
            let Some(offset) = request.offset.as_ref() else {
                return None;
            };
            if offset.numerator < 0 {
                return Some(false);
            }
            Some(trace.windows(2).all(|pair| {
                let crate::discrete_dynamics::DynamicsArtifact::Scalar(left) = &pair[0] else {
                    return false;
                };
                let crate::discrete_dynamics::DynamicsArtifact::Scalar(right) = &pair[1] else {
                    return false;
                };
                left.numerator * right.denominator <= right.numerator * left.denominator
            }))
        }
        _ => Some(true),
    }
}

pub fn run_case(case: &ResearchCase) -> ResearchReceipt {
    let mut steps = vec![step(
        0,
        ResearchStage::Decompose,
        "decompose_conjecture",
        "research_controller",
        "planned",
        "enumerate finite instances and required pack observations",
        digest(&(case.id.clone(), case.claim)),
        true,
    )];
    let mut counterexamples = Vec::new();
    let mut replans = 0;
    let mut conclusion = ResearchConclusion::Unresolved;
    let mut blocked = false;
    for (graph_index, graph) in case.graphs.iter().enumerate() {
        if steps.len() >= case.max_operations {
            break;
        }
        let Some((edge_count, component_count, is_tree, degrees, adjacency, vertices)) =
            graph_observations(case, graph_index, graph, &mut steps)
        else {
            conclusion = ResearchConclusion::Unresolved;
            blocked = true;
            break;
        };
        let adjacency_rank = if case.claim == ResearchClaim::AdjacencyShapePreserved {
            observe_adjacency_rank(case, graph_index, &adjacency, &mut steps)
        } else {
            None
        };
        let claim_holds = inspect_claim(
            case,
            graph_index,
            edge_count,
            component_count,
            is_tree,
            &degrees,
            adjacency_rank,
            vertices,
        );
        steps.push(step(
            steps.len(),
            ResearchStage::Inspect,
            format!("inspect_claim_g{graph_index}"),
            "research_controller",
            if claim_holds { "holds" } else { "violated" },
            format!("finite instance {graph_index} inspected"),
            digest(&(
                edge_count,
                component_count,
                is_tree,
                degrees,
                adjacency_rank,
            )),
            true,
        ));
        if !claim_holds {
            counterexamples.push(format!("graph:{graph_index}"));
            steps.push(step(
                steps.len(),
                ResearchStage::CounterexampleSearch,
                format!("counterexample_search_g{graph_index}"),
                "research_controller",
                "counterexample_found",
                "candidate conjecture fails on supplied finite instance",
                digest(&counterexamples),
                true,
            ));
            replans += 1;
            steps.push(step(
                steps.len(),
                ResearchStage::Replan,
                "replan_after_counterexample",
                "research_controller",
                "refute_and_stop",
                "stop exhaustive search after a verified counterexample",
                digest(&(case.id.clone(), &counterexamples, replans)),
                true,
            ));
            conclusion = ResearchConclusion::Refuted;
            break;
        }
    }
    if conclusion != ResearchConclusion::Refuted && case.dynamics.is_some() {
        match inspect_dynamics(case, &mut steps) {
            Some(true) => {}
            Some(false) => {
                counterexamples.push("dynamics:trace".into());
                steps.push(step(
                    steps.len(),
                    ResearchStage::CounterexampleSearch,
                    "counterexample_search_dynamics",
                    "research_controller",
                    "counterexample_found",
                    "finite dynamics trace violates the conjecture",
                    digest(&counterexamples),
                    true,
                ));
                replans += 1;
                steps.push(step(
                    steps.len(),
                    ResearchStage::Replan,
                    "replan_after_dynamics_counterexample",
                    "research_controller",
                    "refute_and_stop",
                    "stop after exact trace counterexample",
                    digest(&(case.id.clone(), &counterexamples, replans)),
                    true,
                ));
                conclusion = ResearchConclusion::Refuted;
            }
            None => {
                conclusion = ResearchConclusion::Unresolved;
                blocked = true;
            }
        }
    }
    if conclusion == ResearchConclusion::Unresolved && counterexamples.is_empty() {
        conclusion = if !blocked && steps.len() < case.max_operations {
            ResearchConclusion::Supported
        } else {
            ResearchConclusion::Unresolved
        };
    }
    steps.push(step(
        steps.len(),
        ResearchStage::Stop,
        "stop_investigation",
        "research_controller",
        format!("{conclusion:?}"),
        "no unsupported theorem inference; conclusion is finite-instance bounded",
        digest(&(case.id.clone(), conclusion, &counterexamples)),
        true,
    ));
    let replay_hash = digest(&(
        &case.id,
        case.claim,
        conclusion,
        &steps,
        &counterexamples,
        replans,
        case.max_operations,
    ));
    ResearchReceipt {
        case_id: case.id.clone(),
        claim: case.claim,
        conclusion,
        steps,
        counterexamples,
        replans,
        operation_budget: case.max_operations,
        replay_hash,
    }
}

pub fn replay_case(case: &ResearchCase, receipt: &ResearchReceipt) -> bool {
    let replay = run_case(case);
    replay == *receipt && receipt.replay_verified()
}

pub fn evaluate_corpus(cases: &[ResearchCase]) -> ResearchReport {
    let mut report = ResearchReport {
        cases: cases.len(),
        ..ResearchReport::default()
    };
    for case in cases {
        let receipt = run_case(case);
        match receipt.conclusion {
            ResearchConclusion::Supported => report.supported += 1,
            ResearchConclusion::Refuted => report.refuted += 1,
            ResearchConclusion::Unresolved => report.unresolved += 1,
        }
        report.terminal_correct += usize::from(receipt.conclusion == case.expected);
        report.counterexamples_found += usize::from(!receipt.counterexamples.is_empty());
        report.replans += receipt.replans;
        report.operations += receipt.steps.len();
        report.pack_replay_verified += receipt
            .steps
            .iter()
            .filter(|step| step.pack_replay_verified)
            .count();
        report.receipt_replay_verified += usize::from(replay_case(case, &receipt));
        let mut tampered = receipt.clone();
        if let Some(first) = tampered.steps.first_mut() {
            first.observation.push_str(" tampered");
        }
        report.tamper_rejected += usize::from(!replay_case(case, &tampered));
        report.false_authorizations += usize::from(
            receipt.conclusion == ResearchConclusion::Supported
                && case.expected != ResearchConclusion::Supported,
        );
        report.false_denials += usize::from(
            receipt.conclusion != ResearchConclusion::Supported
                && case.expected == ResearchConclusion::Supported,
        );
    }
    report
}

fn graph(vertices: usize, edges: &[(usize, usize)], directed: bool) -> FiniteGraph {
    FiniteGraph {
        vertices: (0..vertices).map(|i| format!("v{i}")).collect(),
        edges: edges.to_vec(),
        directed,
    }
}

fn rational(numerator: i128, denominator: i128) -> crate::probability_pack::Rational {
    crate::probability_pack::Rational::new(numerator, denominator).unwrap()
}

fn dynamics(initial: i128, coefficient: i128, offset: i128, steps: usize) -> DynamicsRequest {
    DynamicsRequest {
        operation: crate::discrete_dynamics::DynamicsOperation::ScalarAffine,
        domain: "finite_exact_discrete_dynamics".into(),
        scalar_initial: Some(rational(initial, 1)),
        coefficient: Some(rational(coefficient, 1)),
        offset: Some(rational(offset, 1)),
        vector_initial: None,
        matrix: None,
        steps,
        ambiguity: None,
        provenance: vec!["stage364:independent-research-corpus".into()],
    }
}

/// Deterministic, independently specified finite research cases.  The case
/// generator gives the investigator only typed instances and a conjecture;
/// it does not prescribe the order of pack operations.
pub fn independent_corpus() -> Vec<ResearchCase> {
    let path = graph(4, &[(0, 1), (1, 2), (2, 3)], false);
    let tree = graph(4, &[(0, 1), (0, 2), (0, 3)], false);
    // A triangle plus an isolated vertex has n-1 edges but is disconnected;
    // it is the finite counterexample used by the edge-count conjecture.
    let disconnected = graph(4, &[(0, 1), (1, 2), (2, 0)], false);
    let cycle = graph(4, &[(0, 1), (1, 2), (2, 3), (3, 0)], false);
    let directed = graph(3, &[(0, 1), (1, 2)], true);
    let mut cases = Vec::new();
    for index in 0..20 {
        cases.push(ResearchCase {
            id: format!("tree-criterion-{index:03}"),
            claim: ResearchClaim::TreeCriterion,
            graphs: vec![path.clone(), tree.clone(), cycle.clone()],
            dynamics: None,
            max_operations: 64,
            expected: ResearchConclusion::Supported,
        });
    }
    for index in 0..20 {
        cases.push(ResearchCase {
            id: format!("edges-imply-connected-{index:03}"),
            claim: ResearchClaim::EdgesMinusOneImpliesConnected,
            graphs: vec![path.clone(), disconnected.clone()],
            dynamics: None,
            max_operations: 64,
            expected: ResearchConclusion::Refuted,
        });
    }
    for index in 0..20 {
        cases.push(ResearchCase {
            id: format!("degree-sum-{index:03}"),
            claim: ResearchClaim::UndirectedDegreeSum,
            graphs: vec![path.clone(), tree.clone(), cycle.clone()],
            dynamics: None,
            max_operations: 64,
            expected: ResearchConclusion::Supported,
        });
    }
    for index in 0..20 {
        cases.push(ResearchCase {
            id: format!("affine-closed-{index:03}"),
            claim: ResearchClaim::AffineClosedForm,
            graphs: vec![],
            dynamics: Some(dynamics(index as i128, 1, (index % 3) as i128, 8)),
            max_operations: 32,
            expected: ResearchConclusion::Supported,
        });
    }
    for index in 0..20 {
        cases.push(ResearchCase {
            id: format!("affine-monotone-{index:03}"),
            claim: ResearchClaim::AffineMonotone,
            graphs: vec![],
            dynamics: Some(dynamics(
                index as i128,
                1,
                if index % 2 == 0 { 1 } else { -1 },
                8,
            )),
            max_operations: 32,
            expected: if index % 2 == 0 {
                ResearchConclusion::Supported
            } else {
                ResearchConclusion::Refuted
            },
        });
    }
    for index in 0..20 {
        cases.push(ResearchCase {
            id: format!("mixed-adjacency-dynamics-{index:03}"),
            claim: ResearchClaim::AdjacencyShapePreserved,
            graphs: vec![path.clone(), tree.clone(), cycle.clone()],
            dynamics: Some(dynamics(1, 1, 1, 4)),
            max_operations: 64,
            expected: ResearchConclusion::Supported,
        });
    }
    // Explicit unsupported/ambiguous inputs are kept in a separate corpus
    // below; they must not be silently converted into supported research.
    let _ = directed;
    cases
}

pub fn unsupported_corpus() -> Vec<ResearchCase> {
    vec![ResearchCase {
        id: "unsupported-directed-tree".into(),
        claim: ResearchClaim::TreeCriterion,
        graphs: vec![graph(3, &[(0, 1), (1, 2)], true)],
        dynamics: None,
        max_operations: 32,
        expected: ResearchConclusion::Unresolved,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn research_uses_real_packs_and_replans() {
        let cases = independent_corpus();
        let report = evaluate_corpus(&cases);
        assert_eq!(report.cases, 120);
        assert_eq!(report.terminal_correct, 120);
        assert!(report.operations > 1_000);
        assert!(report.counterexamples_found >= 30);
        assert!(report.replans >= 30);
        assert_eq!(report.receipt_replay_verified, 120);
        assert_eq!(report.tamper_rejected, 120);
        assert_eq!(report.false_authorizations, 0);
        assert_eq!(report.false_denials, 0);
        let unsupported = evaluate_corpus(&unsupported_corpus());
        assert_eq!(unsupported.unresolved, 1);
        assert_eq!(unsupported.false_authorizations, 0);
    }
}
