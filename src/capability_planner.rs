//! Deterministic planning over the verified capability dependency graph.
//!
//! This planner is intentionally not a theorem planner and does not infer
//! missing modeling steps.  It only expands the unique capability selected
//! for an already-grounded target and returns its dependency-first closure.

use crate::capabilities::{
    CapabilityIoType, CapabilityRegistry, CapabilitySelection,
};
use crate::constant_rate_model::{ModelArtifactType, ModelConstructorRegistry, ModelSelection};
use crate::evidence::{DerivedFact, FactPolicyRejection};
use crate::formalization::{AnswerForm, FormalizedTarget, OperationKind, SubjectObjectType};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

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
    MissingFactPolicy(String),
    InvalidDerivedFacts {
        capability: String,
        rejections: Vec<DerivedFactRejection>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DerivedFactRejection {
    pub fact_id: String,
    pub reason: FactPolicyRejection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ModelPlanningFailure {
    NoEligibleModel,
    AmbiguousModels(Vec<String>),
    MissingModelEntry(String),
    CapabilityPlanning(CapabilityPlanningFailure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct PlanCost {
    pub steps: usize,
    pub dependency_edges: usize,
    pub verification_steps: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PlanSelectionReason {
    UniqueTargetCapability,
    UniqueGoalProducer,
    UniqueModelThenGoalProducer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DependencyProof {
    pub capability: String,
    pub dependency: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InputProof {
    pub capability: String,
    pub input: CapabilityIoType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DerivedFactProof {
    pub capability: String,
    pub fact_id: String,
    pub parent_lineage: Vec<String>,
}

/// Inputs available to goal-directed planning.  Derived facts are kept
/// separate from model evidence and are admitted only through a capability's
/// declared lineage policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct ReasoningContext {
    pub available_inputs: BTreeSet<CapabilityIoType>,
    pub derived_facts: Vec<DerivedFact>,
}

impl ReasoningContext {
    pub fn new(available_inputs: BTreeSet<CapabilityIoType>) -> Self {
        Self {
            available_inputs,
            derived_facts: Vec::new(),
        }
    }

    pub fn with_derived_facts(
        available_inputs: BTreeSet<CapabilityIoType>,
        derived_facts: Vec<DerivedFact>,
    ) -> Self {
        Self {
            available_inputs,
            derived_facts,
        }
    }
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
    pub cost: PlanCost,
    pub selection_reason: PlanSelectionReason,
    pub dependency_proofs: Vec<DependencyProof>,
    pub input_proofs: Vec<InputProof>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GoalCapabilityPlan {
    pub goal: CapabilityIoType,
    pub available_inputs: Vec<CapabilityIoType>,
    pub selected_capability: String,
    pub steps: Vec<CapabilityPlanStep>,
    pub cost: PlanCost,
    pub selection_reason: PlanSelectionReason,
    pub dependency_proofs: Vec<DependencyProof>,
    pub input_proofs: Vec<InputProof>,
    pub derived_fact_proofs: Vec<DerivedFactProof>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelPlanStep {
    pub model_id: String,
    pub version: u32,
    pub model_artifacts: Vec<ModelArtifactType>,
    pub downstream_artifacts: Vec<CapabilityIoType>,
}

/// A shadow-planning receipt for the first model-to-transformation bridge.
/// Model construction remains a separate authorization boundary; this type
/// only proves that a uniquely selected model declares enough typed outputs
/// for a uniquely selected transformation plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelCapabilityPlan {
    pub goal: CapabilityIoType,
    pub model_step: ModelPlanStep,
    pub capability_plan: GoalCapabilityPlan,
    pub cost: PlanCost,
    pub selection_reason: PlanSelectionReason,
}

fn plan_metadata(
    selected: &str,
    steps: &[CapabilityPlanStep],
    registry: &CapabilityRegistry,
    available_inputs: Option<&BTreeSet<CapabilityIoType>>,
) -> (PlanCost, Vec<DependencyProof>, Vec<InputProof>) {
    let mut dependency_proofs = Vec::new();
    let mut input_proofs = Vec::new();
    for step in steps {
        if let Some(capability) = registry.get(&step.capability_id) {
            for dependency in &capability.dependencies {
                dependency_proofs.push(DependencyProof {
                    capability: capability.id.clone(),
                    dependency: dependency.clone(),
                });
            }
            if step.capability_id == selected {
                if let Some(available) = available_inputs {
                    for input in &capability.consumes {
                        if available.contains(input) {
                            input_proofs.push(InputProof {
                                capability: capability.id.clone(),
                                input: *input,
                            });
                        }
                    }
                }
            }
        }
    }
    let cost = PlanCost {
        steps: steps.len(),
        dependency_edges: dependency_proofs.len(),
        verification_steps: steps.iter().filter(|step| !step.verifier.is_empty()).count(),
    };
    (cost, dependency_proofs, input_proofs)
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
    let (cost, dependency_proofs, input_proofs) =
        plan_metadata(&selected, &steps, registry, None);
    Ok(CapabilityPlan {
        operation: target.operation,
        subject_type,
        answer_form: target.answer_form,
        selected_capability: selected,
        steps,
        cost,
        selection_reason: PlanSelectionReason::UniqueTargetCapability,
        dependency_proofs,
        input_proofs,
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
    plan_for_goal_with_context(
        goal,
        &ReasoningContext::new(available_inputs.clone()),
        registry,
    )
}

/// Goal-directed planning with a unified artifact context.  Raw model
/// evidence is intentionally absent here; derived facts enter only through a
/// capability that explicitly consumes `DerivedFact` and declares a
/// lineage-validating `FactPolicy`.
pub fn plan_for_goal_with_context(
    goal: CapabilityIoType,
    context: &ReasoningContext,
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
    let mut candidate_fact_proofs = BTreeMap::new();
    let mut candidate_failures = BTreeMap::new();
    candidates.retain(|capability| {
        let mut eligible = true;
        let mut missing = Vec::new();
        for input in &capability.consumes {
            if *input == CapabilityIoType::DerivedFact {
                let Some(policy) = capability.fact_policy.as_ref() else {
                    eligible = false;
                    candidate_failures.insert(
                        capability.id.clone(),
                        CapabilityPlanningFailure::MissingFactPolicy(capability.id.clone()),
                    );
                    continue;
                };
                let mut proofs = Vec::new();
                let mut rejections = Vec::new();
                for fact in &context.derived_facts {
                    match policy.evaluate(fact, crate::evidence::EvidenceStatus::Inferred) {
                        Ok(()) => proofs.push(DerivedFactProof {
                            capability: capability.id.clone(),
                            fact_id: fact.id.clone(),
                            parent_lineage: fact.parent_lineage.clone(),
                        }),
                        Err(reason) => rejections.push(DerivedFactRejection {
                            fact_id: fact.id.clone(),
                            reason,
                        }),
                    }
                }
                if proofs.is_empty() {
                    eligible = false;
                    candidate_failures.insert(
                        capability.id.clone(),
                        if context.derived_facts.is_empty() {
                            CapabilityPlanningFailure::MissingInputs {
                                capability: capability.id.clone(),
                                missing: vec![CapabilityIoType::DerivedFact],
                            }
                        } else {
                            CapabilityPlanningFailure::InvalidDerivedFacts {
                                capability: capability.id.clone(),
                                rejections,
                            }
                        },
                    );
                } else {
                    candidate_fact_proofs.insert(capability.id.clone(), proofs);
                }
            } else if !context.available_inputs.contains(input) {
                eligible = false;
                missing.push(*input);
            }
        }
        if !missing.is_empty() {
            candidate_failures.insert(
                capability.id.clone(),
                CapabilityPlanningFailure::MissingInputs {
                    capability: capability.id.clone(),
                    missing,
                },
            );
        }
        eligible
    });
    if candidates.is_empty() {
        let mut possible = registry
            .capabilities
            .values()
            .filter(|capability| capability.quality_gate.enabled())
            .filter(|capability| capability.produces.contains(&goal));
        let capability = possible.next().expect("producer checked above");
        if let Some(failure) = candidate_failures.remove(&capability.id) {
            return Err(failure);
        }
        let missing = capability
            .consumes
            .iter()
            .filter(|input| !context.available_inputs.contains(input))
            .copied()
            .collect();
        return Err(CapabilityPlanningFailure::MissingInputs { capability: capability.id.clone(), missing });
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
    let (cost, dependency_proofs, input_proofs) =
        plan_metadata(&selected.id, &steps, registry, Some(&context.available_inputs));
    Ok(GoalCapabilityPlan {
        goal,
        available_inputs: context.available_inputs.iter().copied().collect(),
        selected_capability: selected.id.clone(),
        steps,
        cost,
        selection_reason: PlanSelectionReason::UniqueGoalProducer,
        dependency_proofs,
        input_proofs,
        derived_fact_proofs: candidate_fact_proofs.remove(&selected.id).unwrap_or_default(),
    })
}

/// Plan a uniquely selected text model into one uniquely selected capability
/// producer.  No model is inferred when discovery is empty or ambiguous, and
/// no missing downstream artifact is invented.
pub fn plan_model_to_goal(
    text: &str,
    goal: CapabilityIoType,
    model_registry: &ModelConstructorRegistry,
    capability_registry: &CapabilityRegistry,
) -> Result<ModelCapabilityPlan, ModelPlanningFailure> {
    let discovery = model_registry.discover(text);
    let (model_id, model_version) = match discovery.selection {
        ModelSelection::UniqueVersioned { id, version } => (id, version),
        ModelSelection::Ambiguous(ids) => return Err(ModelPlanningFailure::AmbiguousModels(ids)),
        ModelSelection::None => return Err(ModelPlanningFailure::NoEligibleModel),
    };
    let entry = model_registry
        .get_versioned(&model_id, model_version)
        .ok_or_else(|| ModelPlanningFailure::MissingModelEntry(model_id.clone()))?;
    let available_inputs = entry
        .spec
        .produced_artifacts
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let capability_plan = plan_for_goal(goal, &available_inputs, capability_registry)
        .map_err(ModelPlanningFailure::CapabilityPlanning)?;
    let model_step = ModelPlanStep {
        model_id,
        version: entry.spec.version,
        model_artifacts: entry.spec.model_artifacts.clone(),
        downstream_artifacts: entry.spec.produced_artifacts.clone(),
    };
    let cost = PlanCost {
        steps: capability_plan.cost.steps + 1,
        dependency_edges: capability_plan.cost.dependency_edges,
        verification_steps: capability_plan.cost.verification_steps + 1,
    };
    Ok(ModelCapabilityPlan {
        goal,
        model_step,
        capability_plan,
        cost,
        selection_reason: PlanSelectionReason::UniqueModelThenGoalProducer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::{
        CapabilityIoType, CapabilityRegistry, CapabilitySpec, InputRequirement,
    };
    use crate::evidence::{DerivedFact, FactPolicy};
    use crate::formalization::assess_prompt;
    use crate::constant_rate_model::ModelConstructorRegistry;

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
        assert_eq!(plan.cost.steps, 2);
        assert_eq!(plan.cost.dependency_edges, 1);
        assert_eq!(plan.selection_reason, PlanSelectionReason::UniqueTargetCapability);
        assert_eq!(
            plan.dependency_proofs,
            vec![DependencyProof {
                capability: "function_application".into(),
                dependency: "expression_evaluation".into(),
            }]
        );
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
        assert_eq!(plan.cost.steps, 1);
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
        assert_eq!(plan.cost.steps, 1);
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
    fn model_plan_composes_unique_constructor_with_expression_evaluation() {
        let plan = plan_model_to_goal(
            "A quantity changes at a constant rate of 3 per interval for 4 intervals. Find the total change.",
            CapabilityIoType::ExactValue,
            &ModelConstructorRegistry::production(),
            &CapabilityRegistry::production(),
        )
        .unwrap();
        assert_eq!(plan.model_step.model_id, "constant_rate_model");
        assert_eq!(plan.model_step.version, 1);
        assert!(plan
            .model_step
            .model_artifacts
            .contains(&ModelArtifactType::Relation));
        assert_eq!(
            plan.capability_plan.selected_capability,
            "expression_evaluation"
        );
        assert_eq!(plan.cost.steps, 2);
        assert_eq!(plan.cost.verification_steps, 2);
    }

    #[test]
    fn model_plan_rejects_text_without_a_unique_model() {
        assert_eq!(
            plan_model_to_goal(
                "A quantity changes at a rate of 3 per interval for 4 intervals. Find the total change.",
                CapabilityIoType::ExactValue,
                &ModelConstructorRegistry::production(),
                &CapabilityRegistry::production(),
            ),
            Err(ModelPlanningFailure::NoEligibleModel)
        );
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

    #[test]
    fn composition_benchmark_substitution_target_is_one_step() {
        let target = assess_prompt(
            "composition-substitution",
            "Substitute x=4 into x^2-1.",
            "Math",
            false,
        )
        .target_completion
        .target;
        let plan = plan_target(&target, &CapabilityRegistry::production()).unwrap();
        assert_eq!(plan.selected_capability, "substitution");
        assert_eq!(plan.cost.steps, 1);
    }

    #[test]
    fn composition_benchmark_linear_target_is_one_step() {
        let target = assess_prompt(
            "composition-linear",
            "Solve for x: 3*x+2=11.",
            "Math",
            false,
        )
        .target_completion
        .target;
        let plan = plan_target(&target, &CapabilityRegistry::production()).unwrap();
        assert_eq!(plan.selected_capability, "linear_equation_solve");
        assert_eq!(plan.cost.steps, 1);
    }

    #[test]
    fn composition_benchmark_rejects_unmodeled_function_equation_chain() {
        let target = assess_prompt(
            "composition-function-equation",
            "Given f(x)=x+5. Find x when f(x)=12.",
            "Math",
            false,
        )
        .target_completion
        .target;
        assert!(plan_target(&target, &CapabilityRegistry::production()).is_err());
    }

    fn derived_fact_registry() -> CapabilityRegistry {
        let mut registry = CapabilityRegistry::default();
        let mut capability = CapabilitySpec::expression_evaluation_v1();
        capability.id = "derived_fact_consumer".into();
        capability.consumes = vec![CapabilityIoType::DerivedFact];
        capability.produces = vec![CapabilityIoType::ExactValue];
        capability.input_requirements = vec![
            InputRequirement::VerifiedDerivedFact,
            InputRequirement::ReplayVerifier,
        ];
        capability.fact_policy = Some(FactPolicy::verified_transformation());
        registry.register(capability);
        registry
    }

    #[test]
    fn context_planner_admits_only_lineage_bearing_derived_facts() {
        let context = ReasoningContext::with_derived_facts(
            BTreeSet::new(),
            vec![DerivedFact {
                id: "derived-1".into(),
                content: "distance = 12".into(),
                parent_lineage: vec!["constant-rate-model".into()],
                provenance: "verified expression evaluation".into(),
                proof_kind: crate::evidence::DerivedProofKind::ExactTransformation,
                precision: crate::evidence::FactPrecision::Exact,
                assumptions: Vec::new(),
                domain: None,
            }],
        );
        let plan = plan_for_goal_with_context(
            CapabilityIoType::ExactValue,
            &context,
            &derived_fact_registry(),
        )
        .unwrap();
        assert_eq!(plan.selected_capability, "derived_fact_consumer");
        assert_eq!(plan.derived_fact_proofs.len(), 1);
        assert_eq!(plan.derived_fact_proofs[0].fact_id, "derived-1");
        assert_eq!(
            plan.derived_fact_proofs[0].parent_lineage,
            vec!["constant-rate-model"]
        );
    }

    #[test]
    fn context_planner_rejects_unlineaged_derived_facts() {
        let context = ReasoningContext::with_derived_facts(
            BTreeSet::new(),
            vec![DerivedFact {
                id: "untrusted".into(),
                content: "answer = 42".into(),
                parent_lineage: Vec::new(),
                provenance: "unverified guess".into(),
                proof_kind: crate::evidence::DerivedProofKind::ExactTransformation,
                precision: crate::evidence::FactPrecision::Exact,
                assumptions: Vec::new(),
                domain: None,
            }],
        );
        assert!(matches!(
            plan_for_goal_with_context(
                CapabilityIoType::ExactValue,
                &context,
                &derived_fact_registry(),
            ),
            Err(CapabilityPlanningFailure::InvalidDerivedFacts { capability, .. })
                if capability == "derived_fact_consumer"
        ));
    }
}
