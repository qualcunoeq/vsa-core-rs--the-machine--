//! Shadow one-step finite random-walk composition.
//!
//! This layer composes the graph, probability, and linear-algebra packs only
//! when transition semantics are explicit. It intentionally stops before
//! multi-step walks, stationary distributions, mixing, or spectral claims.

use crate::graph_pack::FiniteGraph;
use crate::probability_pack::{
    evaluate_probability, FiniteDistribution, ProbabilityArtifact, ProbabilityOperation,
    ProbabilityRequest, ProbabilityResult, Rational,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransitionConvention {
    RowStochastic,
    ColumnStochastic,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RandomWalkStatus {
    Complete,
    Missing,
    Ambiguous,
    InvalidTransition,
    DimensionMismatch,
    ZeroDegree,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RandomWalkArtifact {
    NextDistribution(FiniteDistribution),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RandomWalkResult {
    pub status: RandomWalkStatus,
    pub artifact: Option<RandomWalkArtifact>,
    pub convention: Option<TransitionConvention>,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FiniteStepResult {
    pub status: RandomWalkStatus,
    pub final_artifact: Option<RandomWalkArtifact>,
    pub trace: Vec<RandomWalkResult>,
    pub steps: usize,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("random walk serializes"))
    )
}

fn replay_payload(result: &RandomWalkResult) -> impl Serialize + '_ {
    (
        result.status,
        result.artifact.as_ref(),
        result.convention,
        &result.assumptions,
        &result.reasons,
        &result.provenance,
    )
}

fn result(
    status: RandomWalkStatus,
    artifact: Option<RandomWalkArtifact>,
    convention: Option<TransitionConvention>,
    assumptions: Vec<String>,
    reasons: Vec<String>,
    provenance: Vec<String>,
) -> RandomWalkResult {
    let mut result = RandomWalkResult {
        status,
        artifact,
        convention,
        assumptions,
        reasons,
        provenance,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&replay_payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn edge_exists(graph: &FiniteGraph, from: usize, to: usize) -> bool {
    graph.edges.iter().any(|&(left, right)| {
        (left == from && right == to) || (!graph.directed && left == to && right == from)
    })
}

/// Construct the explicit uniform-neighbor transition matrix for a graph.
/// Zero-degree vertices are rejected because no self-loop or absorbing policy
/// is inferred.
pub fn uniform_neighbor_transition(
    graph: &FiniteGraph,
) -> Result<Vec<Vec<Rational>>, RandomWalkStatus> {
    let mut matrix = vec![vec![Rational::zero(); graph.vertices.len()]; graph.vertices.len()];
    for from in 0..graph.vertices.len() {
        let neighbors = (0..graph.vertices.len())
            .filter(|to| edge_exists(graph, from, *to))
            .collect::<Vec<_>>();
        if neighbors.is_empty() {
            return Err(RandomWalkStatus::ZeroDegree);
        }
        let probability =
            Rational::new(1, neighbors.len() as i128).ok_or(RandomWalkStatus::InvalidTransition)?;
        for to in neighbors {
            matrix[from][to] = probability.clone();
        }
    }
    Ok(matrix)
}

fn validate_transition(
    graph: &FiniteGraph,
    matrix: &[Vec<Rational>],
    vertex_order: &[String],
    convention: TransitionConvention,
) -> Result<(), RandomWalkStatus> {
    if vertex_order != graph.vertices.as_slice() {
        return Err(RandomWalkStatus::DimensionMismatch);
    }
    if matrix.len() != graph.vertices.len()
        || matrix.iter().any(|row| row.len() != graph.vertices.len())
    {
        return Err(RandomWalkStatus::DimensionMismatch);
    }
    if matrix.iter().flatten().any(|value| !value.nonnegative()) {
        return Err(RandomWalkStatus::InvalidTransition);
    }
    for from in 0..graph.vertices.len() {
        for to in 0..graph.vertices.len() {
            let weight = match convention {
                TransitionConvention::RowStochastic => &matrix[from][to],
                TransitionConvention::ColumnStochastic => &matrix[to][from],
            };
            if weight.positive() && !edge_exists(graph, from, to) {
                return Err(RandomWalkStatus::InvalidTransition);
            }
        }
    }
    for index in 0..graph.vertices.len() {
        let total = match convention {
            TransitionConvention::RowStochastic => matrix[index]
                .iter()
                .try_fold(Rational::zero(), |sum, value| sum.add(value)),
            TransitionConvention::ColumnStochastic => matrix
                .iter()
                .map(|row| row[index].clone())
                .try_fold(Rational::zero(), |sum, value| sum.add(&value)),
        }
        .ok_or(RandomWalkStatus::InvalidTransition)?;
        if total != Rational::one() {
            return Err(RandomWalkStatus::InvalidTransition);
        }
    }
    Ok(())
}

/// Execute exactly one transition. `explicit_semantics` must be true: a
/// numeric matrix or adjacency artifact never becomes a transition matrix by
/// shape alone.
pub fn execute_one_step(
    graph: &FiniteGraph,
    transition: Option<&[Vec<Rational>]>,
    initial: &ProbabilityResult,
    vertex_order: &[String],
    convention: Option<TransitionConvention>,
    explicit_semantics: bool,
    steps: usize,
    provenance: Vec<String>,
) -> RandomWalkResult {
    let Some(convention) = convention else {
        return result(
            RandomWalkStatus::Ambiguous,
            None,
            None,
            vec![],
            vec!["row or column stochastic convention is required".into()],
            provenance,
        );
    };
    if !explicit_semantics {
        return result(
            RandomWalkStatus::Ambiguous,
            None,
            Some(convention),
            vec![],
            vec!["adjacency shape does not establish transition semantics".into()],
            provenance,
        );
    }
    if steps != 1 {
        return result(
            RandomWalkStatus::Unsupported,
            None,
            Some(convention),
            vec!["one-step transition only".into()],
            vec!["multi-step and limiting-walk semantics are later curriculum items".into()],
            provenance,
        );
    }
    let Some(transition) = transition else {
        return result(
            RandomWalkStatus::Missing,
            None,
            Some(convention),
            vec![],
            vec!["transition matrix is required".into()],
            provenance,
        );
    };
    if let Err(status) = validate_transition(graph, transition, vertex_order, convention) {
        return result(
            status,
            None,
            Some(convention),
            vec!["explicit finite transition semantics".into()],
            vec![
                "transition matrix violates graph, dimension, or normalization constraints".into(),
            ],
            provenance,
        );
    }
    let Some(ProbabilityArtifact::Distribution(initial_distribution)) = initial.artifact.as_ref()
    else {
        return result(
            RandomWalkStatus::Missing,
            None,
            Some(convention),
            vec![],
            vec!["initial state must be a verified finite probability distribution".into()],
            provenance,
        );
    };
    if initial.status != crate::probability_pack::ProbabilityStatus::Complete
        || !initial.replay_verified()
        || initial_distribution.outcomes != graph.vertices
        || initial_distribution.probabilities.len() != graph.vertices.len()
    {
        return result(
            RandomWalkStatus::DimensionMismatch,
            None,
            Some(convention),
            vec!["initial distribution uses the explicit graph vertex order".into()],
            vec!["initial distribution is incomplete or does not match graph identity".into()],
            provenance,
        );
    }
    let mut next = vec![Rational::zero(); graph.vertices.len()];
    for (from, probability) in initial_distribution.probabilities.iter().enumerate() {
        for to in 0..graph.vertices.len() {
            let weight = match convention {
                TransitionConvention::RowStochastic => transition[from][to].clone(),
                TransitionConvention::ColumnStochastic => transition[to][from].clone(),
            };
            let Some(term) = probability.mul(&weight) else {
                return result(
                    RandomWalkStatus::InvalidTransition,
                    None,
                    Some(convention),
                    vec![],
                    vec!["exact rational transition failed".into()],
                    provenance,
                );
            };
            next[to] = next[to].add(&term).expect("bounded rational walk sum");
        }
    }
    result(
        RandomWalkStatus::Complete,
        Some(RandomWalkArtifact::NextDistribution(FiniteDistribution {
            outcomes: graph.vertices.clone(),
            probabilities: next,
        })),
        Some(convention),
        vec![
            "finite graph topology".into(),
            "exact finite probability".into(),
        ],
        Vec::new(),
        provenance,
    )
}

impl RandomWalkResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&replay_payload(self))
            && !self.provenance.is_empty()
            && (self.status != RandomWalkStatus::Complete || self.artifact.is_some())
    }
}

fn finite_step_payload(result: &FiniteStepResult) -> impl Serialize + '_ {
    (
        result.status,
        result.final_artifact.as_ref(),
        &result.trace,
        result.steps,
        &result.assumptions,
        &result.reasons,
        &result.provenance,
    )
}

fn finite_step_result(
    status: RandomWalkStatus,
    final_artifact: Option<RandomWalkArtifact>,
    trace: Vec<RandomWalkResult>,
    steps: usize,
    assumptions: Vec<String>,
    reasons: Vec<String>,
    provenance: Vec<String>,
) -> FiniteStepResult {
    let mut result = FiniteStepResult {
        status,
        final_artifact,
        trace,
        steps,
        assumptions,
        reasons,
        provenance,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&finite_step_payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn distribution_as_probability_result(
    distribution: &FiniteDistribution,
    provenance: Vec<String>,
) -> ProbabilityResult {
    evaluate_probability(&ProbabilityRequest {
        operation: ProbabilityOperation::DistributionConstruction,
        domain: "finite_exact_probability".into(),
        outcomes: distribution.outcomes.clone(),
        probabilities: distribution.probabilities.clone(),
        values: Vec::new(),
        event_a: None,
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: None,
        provenance,
    })
}

/// Execute a fixed number of steps with a complete intermediate trace. The
/// step budget is deliberately small and exact; no stationary or limiting
/// inference is performed.
pub fn execute_bounded_steps(
    graph: &FiniteGraph,
    transition: Option<&[Vec<Rational>]>,
    initial: &ProbabilityResult,
    vertex_order: &[String],
    convention: Option<TransitionConvention>,
    explicit_semantics: bool,
    steps: usize,
    provenance: Vec<String>,
) -> FiniteStepResult {
    if !(1..=8).contains(&steps) {
        return finite_step_result(
            RandomWalkStatus::Unsupported,
            None,
            Vec::new(),
            steps,
            vec!["bounded one-to-eight step budget".into()],
            vec![
                "zero-step, multi-step beyond budget, and limiting-walk semantics are unsupported"
                    .into(),
            ],
            provenance,
        );
    }
    let mut current = initial.clone();
    let mut trace = Vec::with_capacity(steps);
    let mut final_artifact = None;
    for step in 0..steps {
        let mut step_provenance = provenance.clone();
        step_provenance.push(format!("step:{step}"));
        let one = execute_one_step(
            graph,
            transition,
            &current,
            vertex_order,
            convention,
            explicit_semantics,
            1,
            step_provenance,
        );
        let status = one.status;
        let artifact = one.artifact.clone();
        trace.push(one);
        if status != RandomWalkStatus::Complete {
            return finite_step_result(
                status,
                None,
                trace,
                steps,
                vec!["every intermediate step must be verified".into()],
                vec!["bounded walk stopped at the first invalid transition".into()],
                provenance,
            );
        }
        let Some(RandomWalkArtifact::NextDistribution(distribution)) = artifact.as_ref() else {
            return finite_step_result(
                RandomWalkStatus::DimensionMismatch,
                None,
                trace,
                steps,
                vec![],
                vec!["verified step did not produce a distribution".into()],
                provenance,
            );
        };
        let distribution = distribution.clone();
        final_artifact = artifact;
        current = distribution_as_probability_result(&distribution, provenance.clone());
        if current.status != crate::probability_pack::ProbabilityStatus::Complete
            || !current.replay_verified()
        {
            return finite_step_result(
                RandomWalkStatus::InvalidTransition,
                None,
                trace,
                steps,
                vec![],
                vec!["intermediate distribution failed finite-probability replay".into()],
                provenance,
            );
        }
    }
    finite_step_result(
        RandomWalkStatus::Complete,
        final_artifact,
        trace,
        steps,
        vec![
            "exact rational arithmetic".into(),
            "fixed graph and vertex order".into(),
        ],
        Vec::new(),
        provenance,
    )
}

impl FiniteStepResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&finite_step_payload(self))
            && !self.provenance.is_empty()
            && self.trace.iter().all(RandomWalkResult::replay_verified)
            && (self.status != RandomWalkStatus::Complete || self.final_artifact.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probability_pack::{evaluate_probability, ProbabilityOperation, ProbabilityRequest};

    fn graph() -> FiniteGraph {
        FiniteGraph {
            vertices: vec!["a".into(), "b".into()],
            edges: vec![(0, 1)],
            directed: false,
        }
    }

    fn initial() -> ProbabilityResult {
        evaluate_probability(&ProbabilityRequest {
            operation: ProbabilityOperation::DistributionConstruction,
            domain: "finite_exact_probability".into(),
            outcomes: vec!["a".into(), "b".into()],
            probabilities: vec![Rational::one(), Rational::zero()],
            values: Vec::new(),
            event_a: None,
            event_b: None,
            partition: Vec::new(),
            conditional_values: Vec::new(),
            prior_probability: None,
            likelihood: None,
            evidence: None,
            ambiguity: None,
            provenance: vec!["test".into()],
        })
    }

    #[test]
    fn one_step_walk_replays() {
        let matrix = uniform_neighbor_transition(&graph()).unwrap();
        let result = execute_one_step(
            &graph(),
            Some(&matrix),
            &initial(),
            &["a".into(), "b".into()],
            Some(TransitionConvention::RowStochastic),
            true,
            1,
            vec!["test".into()],
        );
        assert_eq!(result.status, RandomWalkStatus::Complete);
        assert!(result.replay_verified());
    }

    #[test]
    fn semantics_and_zero_degree_fail_closed() {
        let disconnected = FiniteGraph {
            vertices: vec!["a".into(), "b".into()],
            edges: Vec::new(),
            directed: false,
        };
        assert_eq!(
            uniform_neighbor_transition(&disconnected),
            Err(RandomWalkStatus::ZeroDegree)
        );
        let graph = graph();
        let result = execute_one_step(
            &graph,
            Some(&uniform_neighbor_transition(&graph).unwrap()),
            &initial(),
            &["a".into(), "b".into()],
            Some(TransitionConvention::RowStochastic),
            false,
            1,
            vec!["test".into()],
        );
        assert_eq!(result.status, RandomWalkStatus::Ambiguous);
    }

    #[test]
    fn bounded_trace_replays_and_budget_is_enforced() {
        let matrix = uniform_neighbor_transition(&graph()).unwrap();
        let result = execute_bounded_steps(
            &graph(),
            Some(&matrix),
            &initial(),
            &["a".into(), "b".into()],
            Some(TransitionConvention::RowStochastic),
            true,
            3,
            vec!["test".into()],
        );
        assert_eq!(result.status, RandomWalkStatus::Complete);
        assert_eq!(result.trace.len(), 3);
        assert!(result.replay_verified());
        let too_deep = execute_bounded_steps(
            &graph(),
            Some(&matrix),
            &initial(),
            &["a".into(), "b".into()],
            Some(TransitionConvention::RowStochastic),
            true,
            9,
            vec!["test".into()],
        );
        assert_eq!(too_deep.status, RandomWalkStatus::Unsupported);
    }
}
