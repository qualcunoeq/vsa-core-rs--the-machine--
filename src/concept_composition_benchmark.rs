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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConceptCompositionBudgetMetrics {
    pub budget: usize,
    pub full_proposals: usize,
    pub budgeted_proposals: usize,
    pub nodes_visited: usize,
    pub candidates_pruned: usize,
    pub frontier_subset: bool,
    pub nested_with_previous: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConceptCompositionBudgetSweepReport {
    pub branches_per_stage: usize,
    pub stage_count: usize,
    pub graph_concepts: usize,
    pub max_concepts: usize,
    pub full_proposals: usize,
    pub budgets: Vec<ConceptCompositionBudgetMetrics>,
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

fn layered_fixture(
    branches_per_stage: usize,
    stage_count: usize,
) -> (CapabilityRegistry, CapabilityChainProofConceptIndex) {
    let branches_per_stage = branches_per_stage.max(1);
    let stage_count = stage_count.clamp(1, 5);
    let mut registry = CapabilityRegistry::production();
    let mut index = CapabilityChainProofConceptIndex::default();
    let artifacts = [
        CapabilityIoType::Equation,
        CapabilityIoType::NormalizedEquation,
        CapabilityIoType::EquationClassification,
        CapabilityIoType::ExactValue,
        CapabilityIoType::DerivedFact,
        CapabilityIoType::VerifiedArtifact,
    ];
    let stage_names = ["normalize", "classify", "evaluate", "derive", "verify"];
    for stage in 0..stage_count {
        let stage_name = stage_names[stage];
        let input = artifacts[stage];
        let output = artifacts[stage + 1];
        let spec = if stage == 0 {
            CapabilitySpec::equation_normalization_v1()
        } else {
            CapabilitySpec::equation_classification_v1()
        };
        for branch in 0..branches_per_stage {
            let capability_id = format!("{stage_name}_capability_{branch}");
            let capability = alternate_spec(spec.clone(), &capability_id, input, output);
            let mut capability = capability;
            capability.input_requirements.clear();
            registry.register(capability);
            let concept_id = format!("{stage_name}-{branch}");
            let concept = CapabilityChainProofConceptContract {
                concept_id: concept_id.clone(),
                capabilities: vec![capability_id],
                input_artifacts: vec![input],
                output_artifacts: vec![output],
                source_pattern_ids: vec![
                    format!("{concept_id}-pattern-a"),
                    format!("{concept_id}-pattern-b"),
                ],
                supporting_instances: 12,
                parameterized_signature: format!("{input:?}->{output:?}"),
                diagnostic_only: true,
            };
            let validation = concept.validate(2, 2, 0, 0);
            assert!(validation.passed, "fixture concept must validate: {concept_id}");
            index.insert(concept, &validation).unwrap();
        }
    }
    (registry, index)
}

fn branching_fixture(
    branches_per_stage: usize,
) -> (CapabilityRegistry, CapabilityChainProofConceptIndex) {
    layered_fixture(branches_per_stage, 3)
}

fn fixture() -> (CapabilityRegistry, CapabilityChainProofConceptIndex) {
    branching_fixture(2)
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

fn proposal_ids(
    items: &[crate::capability_planner::CapabilityChainProofConceptPlanningProposal],
) -> std::collections::BTreeSet<String> {
    items.iter().map(|proposal| proposal.concept_id.clone()).collect()
}

fn evaluate_budget_sweep_once(
    branches_per_stage: usize,
    stage_count: usize,
    max_concepts: usize,
    budgets: &[usize],
) -> ConceptCompositionBudgetSweepReport {
    let stage_count = stage_count.clamp(1, 5);
    let (registry, index) = layered_fixture(branches_per_stage, stage_count);
    let goal_artifact = [
        CapabilityIoType::Equation,
        CapabilityIoType::NormalizedEquation,
        CapabilityIoType::EquationClassification,
        CapabilityIoType::ExactValue,
        CapabilityIoType::DerivedFact,
        CapabilityIoType::VerifiedArtifact,
    ][stage_count];
    let full = index.propose_composed_planning_assistance_with_limits(
        &[CapabilityIoType::Equation],
        goal_artifact,
        &registry,
        max_concepts,
        usize::MAX,
    );
    let full_ids = proposal_ids(&full.planning.proposals);
    let mut previous_ids = std::collections::BTreeSet::new();
    let mut metrics = Vec::new();
    for &budget in budgets {
        let bounded = index.propose_composed_planning_assistance_with_limits(
            &[CapabilityIoType::Equation],
            goal_artifact,
            &registry,
            max_concepts,
            budget,
        );
        let ids = proposal_ids(&bounded.planning.proposals);
        metrics.push(ConceptCompositionBudgetMetrics {
            budget,
            full_proposals: full_ids.len(),
            budgeted_proposals: ids.len(),
            nodes_visited: bounded.nodes_visited,
            candidates_pruned: bounded.candidates_pruned,
            frontier_subset: ids.is_subset(&full_ids),
            nested_with_previous: previous_ids.is_subset(&ids),
        });
        previous_ids = ids;
    }
    ConceptCompositionBudgetSweepReport {
        branches_per_stage: branches_per_stage.max(1),
        stage_count,
        graph_concepts: index.len(),
        max_concepts,
        full_proposals: full_ids.len(),
        budgets: metrics,
        deterministic: true,
        diagnostic_only: true,
    }
}

pub fn evaluate_budget_sweep(
    branches_per_stage: usize,
    max_concepts: usize,
    budgets: &[usize],
) -> ConceptCompositionBudgetSweepReport {
    evaluate_budget_sweep_with_stages(branches_per_stage, 3, max_concepts, budgets)
}

pub fn evaluate_budget_sweep_with_stages(
    branches_per_stage: usize,
    stage_count: usize,
    max_concepts: usize,
    budgets: &[usize],
) -> ConceptCompositionBudgetSweepReport {
    let first = evaluate_budget_sweep_once(branches_per_stage, stage_count, max_concepts, budgets);
    let second = evaluate_budget_sweep_once(branches_per_stage, stage_count, max_concepts, budgets);
    ConceptCompositionBudgetSweepReport {
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

    #[test]
    fn larger_branching_budget_sweep_preserves_frontier_membership() {
        let report = evaluate_budget_sweep(3, 3, &[1, 4, 16, 27]);
        assert!(report.diagnostic_only);
        assert!(report.deterministic);
        assert_eq!(report.branches_per_stage, 3);
        assert_eq!(report.graph_concepts, 9);
        assert_eq!(report.full_proposals, 27);
        assert!(report.budgets.iter().all(|metric| metric.frontier_subset));
        assert!(report
            .budgets
            .iter()
            .all(|metric| metric.nested_with_previous));
        assert_eq!(report.budgets.last().unwrap().budgeted_proposals, 27);
        assert!(report
            .budgets
            .iter()
            .filter(|metric| metric.budget < report.full_proposals)
            .all(|metric| metric.candidates_pruned > 0));
    }

    #[test]
    fn deeper_branching_budget_sweep_scales_without_authorization() {
        let report = evaluate_budget_sweep_with_stages(3, 4, 4, &[1, 9, 27, 81]);
        assert!(report.diagnostic_only);
        assert!(report.deterministic);
        assert_eq!(report.branches_per_stage, 3);
        assert_eq!(report.stage_count, 4);
        assert_eq!(report.graph_concepts, 12);
        assert_eq!(report.full_proposals, 81);
        assert!(report.budgets.iter().all(|metric| metric.frontier_subset));
        assert!(report
            .budgets
            .iter()
            .all(|metric| metric.nested_with_previous));
        assert_eq!(report.budgets.last().unwrap().budgeted_proposals, 81);
    }
}
