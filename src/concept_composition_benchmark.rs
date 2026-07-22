//! Bounded resource measurements for validated concept composition.
//!
//! This benchmark exercises the advisory concept planner over a small
//! branching graph.  It measures candidate growth by depth and never executes
//! a composed route or mutates the concept index.

use crate::capabilities::{CapabilityIoType, CapabilityRegistry, CapabilitySpec};
use crate::capability_planner::{
    CapabilityChainProofConceptContract, CapabilityChainProofConceptIndex,
};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConceptCompositionDepthMetrics {
    pub max_concepts: usize,
    pub proposals: usize,
    pub rejections: usize,
    pub route_lengths: BTreeMap<usize, usize>,
    pub theoretical_path_bound: usize,
    pub candidate_budget: usize,
    pub budgeted_proposals: usize,
    pub budgeted_nodes_visited: usize,
    pub budgeted_candidates_pruned: usize,
    pub full_budget_frontier_preserved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConceptCompositionBenchmarkReport {
    pub graph_concepts: usize,
    pub requested_max_depth: usize,
    pub depths: Vec<ConceptCompositionDepthMetrics>,
    pub deterministic: bool,
    pub diagnostic_only: bool,
}

fn alternate_spec(
    mut spec: CapabilitySpec,
    id: &str,
    consumes: CapabilityIoType,
    produces: CapabilityIoType,
) -> CapabilitySpec {
    spec.id = id.into();
    spec.consumes = vec![consumes];
    spec.produces = vec![produces];
    spec
}

fn fixture() -> (CapabilityRegistry, CapabilityChainProofConceptIndex) {
    let mut registry = CapabilityRegistry::production();
    let normalization_alt = alternate_spec(
        CapabilitySpec::equation_normalization_v1(),
        "equation_normalization_alt",
        CapabilityIoType::Equation,
        CapabilityIoType::NormalizedEquation,
    );
    let classification_alt = alternate_spec(
        CapabilitySpec::equation_classification_v1(),
        "equation_classification_alt",
        CapabilityIoType::NormalizedEquation,
        CapabilityIoType::EquationClassification,
    );
    let mut value = alternate_spec(
        CapabilitySpec::equation_classification_v1(),
        "classification_to_value",
        CapabilityIoType::EquationClassification,
        CapabilityIoType::ExactValue,
    );
    value.input_requirements.clear();
    let mut value_alt = value.clone();
    value_alt.id = "classification_to_value_alt".into();
    registry.register(normalization_alt);
    registry.register(classification_alt);
    registry.register(value);
    registry.register(value_alt);

    let concepts = [
        (
            "normalize-base",
            "equation_normalization",
            CapabilityIoType::Equation,
            CapabilityIoType::NormalizedEquation,
        ),
        (
            "normalize-alt",
            "equation_normalization_alt",
            CapabilityIoType::Equation,
            CapabilityIoType::NormalizedEquation,
        ),
        (
            "classify-base",
            "equation_classification",
            CapabilityIoType::NormalizedEquation,
            CapabilityIoType::EquationClassification,
        ),
        (
            "classify-alt",
            "equation_classification_alt",
            CapabilityIoType::NormalizedEquation,
            CapabilityIoType::EquationClassification,
        ),
        (
            "value-base",
            "classification_to_value",
            CapabilityIoType::EquationClassification,
            CapabilityIoType::ExactValue,
        ),
        (
            "value-alt",
            "classification_to_value_alt",
            CapabilityIoType::EquationClassification,
            CapabilityIoType::ExactValue,
        ),
    ];
    let mut index = CapabilityChainProofConceptIndex::default();
    for (concept_id, capability, input, output) in concepts {
        let concept = CapabilityChainProofConceptContract {
            concept_id: concept_id.into(),
            capabilities: vec![capability.into()],
            input_artifacts: vec![input],
            output_artifacts: vec![output],
            source_pattern_ids: vec![format!("{concept_id}-pattern-a"), format!("{concept_id}-pattern-b")],
            supporting_instances: 12,
            parameterized_signature: format!("{input:?}->{output:?}"),
            diagnostic_only: true,
        };
        let validation = concept.validate(2, 2, 0, 0);
        assert!(validation.passed, "fixture concept must validate: {concept_id}");
        index.insert(concept, &validation).unwrap();
    }
    (registry, index)
}

fn path_bound(concepts: usize, depth: usize) -> usize {
    (1..=depth).fold(0usize, |sum, level| {
        sum.saturating_add(concepts.saturating_pow(level as u32))
    })
}

fn evaluate_once(max_depth: usize) -> ConceptCompositionBenchmarkReport {
    let (registry, index) = fixture();
    let requested_max_depth = max_depth.max(2).min(8);
    let mut depths = Vec::new();
    for depth in 2..=requested_max_depth {
        let receipt = index.propose_composed_planning_assistance_with_depth(
            &[CapabilityIoType::Equation],
            CapabilityIoType::ExactValue,
            &registry,
            depth,
        );
        let mut route_lengths = BTreeMap::new();
        for proposal in &receipt.proposals {
            *route_lengths.entry(proposal.plan.steps.len()).or_default() += 1;
        }
        let candidate_budget = 4;
        let budgeted = index.propose_composed_planning_assistance_with_limits(
            &[CapabilityIoType::Equation],
            CapabilityIoType::ExactValue,
            &registry,
            depth,
            candidate_budget,
        );
        let full_budget = index.propose_composed_planning_assistance_with_limits(
            &[CapabilityIoType::Equation],
            CapabilityIoType::ExactValue,
            &registry,
            depth,
            receipt.proposals.len(),
        );
        let proposal_ids = |items: &[crate::capability_planner::CapabilityChainProofConceptPlanningProposal]| {
            items
                .iter()
                .map(|proposal| proposal.concept_id.clone())
                .collect::<Vec<_>>()
        };
        depths.push(ConceptCompositionDepthMetrics {
            max_concepts: depth,
            proposals: receipt.proposals.len(),
            rejections: receipt.rejections.len(),
            route_lengths,
            theoretical_path_bound: path_bound(index.len(), depth),
            candidate_budget,
            budgeted_proposals: budgeted.planning.proposals.len(),
            budgeted_nodes_visited: budgeted.nodes_visited,
            budgeted_candidates_pruned: budgeted.candidates_pruned,
            full_budget_frontier_preserved: proposal_ids(&receipt.proposals)
                == proposal_ids(&full_budget.planning.proposals),
        });
    }
    ConceptCompositionBenchmarkReport {
        graph_concepts: index.len(),
        requested_max_depth,
        depths,
        deterministic: true,
        diagnostic_only: true,
    }
}

pub fn evaluate(max_depth: usize) -> ConceptCompositionBenchmarkReport {
    let first = evaluate_once(max_depth);
    let second = evaluate_once(max_depth);
    ConceptCompositionBenchmarkReport {
        deterministic: first == second,
        ..first
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_composition_reports_branching_without_execution() {
        let report = evaluate(5);
        assert!(report.diagnostic_only);
        assert!(report.deterministic);
        assert_eq!(report.graph_concepts, 6);
        assert_eq!(report.depths.len(), 4);
        assert_eq!(report.depths[0].proposals, 0);
        assert_eq!(report.depths[1].proposals, 8);
        assert_eq!(report.depths[1].route_lengths.get(&3), Some(&8));
        assert_eq!(report.depths[1].candidate_budget, 4);
        assert_eq!(report.depths[1].budgeted_proposals, 4);
        assert!(report.depths[1].budgeted_candidates_pruned > 0);
        assert!(report.depths[1].budgeted_nodes_visited < report.depths[1].theoretical_path_bound);
        assert!(report
            .depths
            .iter()
            .all(|depth| depth.full_budget_frontier_preserved));
        assert_eq!(report.depths[2].proposals, 8);
        assert!(report
            .depths
            .iter()
            .all(|depth| depth.rejections == 0));
    }

    #[test]
    fn depth_is_clamped_to_a_finite_resource_bound() {
        let report = evaluate(1000);
        assert_eq!(report.requested_max_depth, 8);
        assert_eq!(report.depths.len(), 7);
        assert!(report
            .depths
            .iter()
            .all(|depth| depth.theoretical_path_bound > 0));
    }
}
