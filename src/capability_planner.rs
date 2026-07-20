//! Deterministic planning over the verified capability dependency graph.
//!
//! This planner is intentionally not a theorem planner and does not infer
//! missing modeling steps.  It only expands the unique capability selected
//! for an already-grounded target and returns its dependency-first closure.

use crate::capabilities::{
    CapabilityIoType, CapabilityRegistry, CapabilitySelection,
};
use crate::formalization::{AnswerForm, FormalizedTarget, OperationKind, SubjectObjectType};
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilityPlanningFailure {
    NoEligibleCapability,
    AmbiguousCapabilities(Vec<String>),
    DependencyUnavailable(String),
    DependencyCycle(String),
    NoProducer(CapabilityIoType),
    MissingInputs {
        capability: String,
        missing: Vec<CapabilityIoType>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityPlanStep {
    pub capability_id: String,
    pub version: u32,
    pub executor: String,
    pub verifier: String,
    pub consumes: Vec<CapabilityIoType>,
    pub produces: Vec<CapabilityIoType>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityPlan {
    pub operation: OperationKind,
    pub subject_type: SubjectObjectType,
    pub answer_form: Option<AnswerForm>,
    pub selected_capability: String,
    pub steps: Vec<CapabilityPlanStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GoalCapabilityPlan {
    pub goal: CapabilityIoType,
    pub available_inputs: Vec<CapabilityIoType>,
    pub selected_capability: String,
    pub steps: Vec<CapabilityPlanStep>,
}

fn dependency_steps(
    id: &str,
    registry: &CapabilityRegistry,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    steps: &mut Vec<CapabilityPlanStep>,
) -> Result<(), CapabilityPlanningFailure> {
    if visited.contains(id) {
        return Ok(());
    }
    if !visiting.insert(id.to_string()) {
        return Err(CapabilityPlanningFailure::DependencyCycle(id.to_string()));
    }
    let capability = registry
        .get(id)
        .ok_or_else(|| CapabilityPlanningFailure::DependencyUnavailable(id.to_string()))?;
    if !capability.quality_gate.enabled() {
        return Err(CapabilityPlanningFailure::DependencyUnavailable(id.to_string()));
    }
    for dependency in &capability.dependencies {
        dependency_steps(dependency, registry, visiting, visited, steps)?;
    }
    visiting.remove(id);
    visited.insert(id.to_string());
    steps.push(CapabilityPlanStep {
        capability_id: capability.id.clone(),
        version: capability.version,
        executor: capability.executor.clone(),
        verifier: capability.verifier.clone(),
        consumes: capability.consumes.clone(),
        produces: capability.produces.clone(),
    });
    Ok(())
}

pub fn plan_target(
    target: &FormalizedTarget,
    registry: &CapabilityRegistry,
) -> Result<CapabilityPlan, CapabilityPlanningFailure> {
    let discovery = registry.discover(target);
    let selected = match discovery.selection {
        CapabilitySelection::Unique(id) => id,
        CapabilitySelection::Ambiguous(ids) => {
            return Err(CapabilityPlanningFailure::AmbiguousCapabilities(ids))
        }
        CapabilitySelection::None => return Err(CapabilityPlanningFailure::NoEligibleCapability),
    };
    let subject_type = target
        .subject_resolution
        .selected
        .as_ref()
        .map(|subject| subject.object_type)
        .ok_or(CapabilityPlanningFailure::NoEligibleCapability)?;
    let mut steps = Vec::new();
    dependency_steps(
        &selected,
        registry,
        &mut BTreeSet::new(),
        &mut BTreeSet::new(),
        &mut steps,
    )?;
    Ok(CapabilityPlan {
        operation: target.operation,
        subject_type,
        answer_form: target.answer_form,
        selected_capability: selected,
        steps,
    })
}

/// Select one capability that can produce `goal` from the explicitly
/// available artifacts.  This is deliberately one-step dataflow planning;
/// dependencies are expanded, but missing data inputs are not invented.
pub fn plan_for_goal(
    goal: CapabilityIoType,
    available_inputs: &BTreeSet<CapabilityIoType>,
    registry: &CapabilityRegistry,
) -> Result<GoalCapabilityPlan, CapabilityPlanningFailure> {
    let mut candidates = registry
        .capabilities
        .values()
        .filter(|capability| capability.quality_gate.enabled())
        .filter(|capability| capability.produces.contains(&goal))
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Err(CapabilityPlanningFailure::NoProducer(goal));
    }
    candidates.retain(|capability| {
        capability
            .consumes
            .iter()
            .all(|input| available_inputs.contains(input))
    });
    if candidates.is_empty() {
        let mut possible = registry
            .capabilities
            .values()
            .filter(|capability| capability.quality_gate.enabled())
            .filter(|capability| capability.produces.contains(&goal));
        let capability = possible.next().expect("producer checked above");
        let missing = capability
            .consumes
            .iter()
            .filter(|input| !available_inputs.contains(input))
            .copied()
            .collect();
        return Err(CapabilityPlanningFailure::MissingInputs {
            capability: capability.id.clone(),
            missing,
        });
    }
    if candidates.len() > 1 {
        return Err(CapabilityPlanningFailure::AmbiguousCapabilities(
            candidates.into_iter().map(|capability| capability.id.clone()).collect(),
        ));
    }
    let selected = candidates[0];
    let mut steps = Vec::new();
    dependency_steps(
        &selected.id,
        registry,
        &mut BTreeSet::new(),
        &mut BTreeSet::new(),
        &mut steps,
    )?;
    Ok(GoalCapabilityPlan {
        goal,
        available_inputs: available_inputs.iter().copied().collect(),
        selected_capability: selected.id.clone(),
        steps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::{CapabilityIoType, CapabilityRegistry};
    use crate::formalization::assess_prompt;

    #[test]
    fn function_plan_expands_dependencies_first() {
        let target = assess_prompt(
            "plan-function",
            "Let f(x)=2*x+1. Evaluate f(3).",
            "Math",
            false,
        )
        .target_completion
        .target;
        let plan = plan_target(&target, &CapabilityRegistry::production()).unwrap();
        assert_eq!(plan.selected_capability, "function_application");
        assert_eq!(
            plan.steps
                .iter()
                .map(|step| step.capability_id.as_str())
                .collect::<Vec<_>>(),
            vec!["expression_evaluation", "function_application"]
        );
        assert_eq!(plan.steps[0].version, 1);
        assert!(!plan.steps[0].verifier.is_empty());
        assert_eq!(
            plan.steps[0].produces,
            vec![CapabilityIoType::ExactValue]
        );
        assert_eq!(
            plan.steps[1].consumes,
            vec![CapabilityIoType::FunctionDefinition, CapabilityIoType::BindingSet]
        );
    }

    #[test]
    fn expression_plan_is_single_step() {
        let target = assess_prompt("plan-expression", "Evaluate 2+3.", "Math", false)
            .target_completion
            .target;
        let plan = plan_target(&target, &CapabilityRegistry::production()).unwrap();
        assert_eq!(plan.selected_capability, "expression_evaluation");
        assert_eq!(plan.steps.len(), 1);
    }

    #[test]
    fn unsupported_target_has_no_plan() {
        let target = assess_prompt("plan-unsupported", "Prove x=x.", "Math", false)
            .target_completion
            .target;
        assert_eq!(
            plan_target(&target, &CapabilityRegistry::production()),
            Err(CapabilityPlanningFailure::NoEligibleCapability)
        );
    }

    #[test]
    fn goal_planner_selects_substitution_from_typed_inputs() {
        let available = BTreeSet::from([
            CapabilityIoType::Expression,
            CapabilityIoType::BindingSet,
        ]);
        let plan = plan_for_goal(
            CapabilityIoType::Expression,
            &available,
            &CapabilityRegistry::production(),
        )
        .unwrap();
        assert_eq!(plan.selected_capability, "substitution");
        assert_eq!(plan.steps.len(), 1);
    }

    #[test]
    fn goal_planner_rejects_missing_inputs() {
        let available = BTreeSet::from([CapabilityIoType::Equation]);
        assert!(matches!(
            plan_for_goal(
                CapabilityIoType::SolutionSet,
                &available,
                &CapabilityRegistry::production()
            ),
            Err(CapabilityPlanningFailure::MissingInputs { .. })
        ));
    }

    #[test]
    fn goal_planner_abstains_on_multiple_exact_value_producers() {
        let available = BTreeSet::from([
            CapabilityIoType::Expression,
            CapabilityIoType::FunctionDefinition,
            CapabilityIoType::BindingSet,
        ]);
        assert!(matches!(
            plan_for_goal(
                CapabilityIoType::ExactValue,
                &available,
                &CapabilityRegistry::production()
            ),
            Err(CapabilityPlanningFailure::AmbiguousCapabilities(_))
        ));
    }
}
