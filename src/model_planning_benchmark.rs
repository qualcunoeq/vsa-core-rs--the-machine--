//! Small, deterministic benchmark for the model-to-capability planning loop.
//!
//! This is intentionally a planning benchmark, not an answer benchmark.  It
//! measures whether text yields a uniquely authorized model and a compatible
//! downstream capability plan.  Execution remains owned by the model and
//! capability verifiers.

use crate::capabilities::{CapabilityIoType, CapabilityRegistry};
use crate::capability_planner::{plan_model_to_goal, ModelCapabilityPlan, ModelPlanningFailure};
use crate::constant_rate_model::ModelConstructorRegistry;
#[cfg(test)]
use crate::constant_rate_model::{
    EvidencePolicy, ModelArtifactType, ModelConstructionQualityGate, ModelConstructionSpec,
    ModelConstructorEntry, ModelEvidenceContext, ModelEvidenceState, ModelMatcherResult,
};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedPlanningOutcome {
    PlanReady,
    NoEligibleModel,
    AmbiguousModel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelPlanningBenchmarkCase {
    pub id: String,
    pub prompt: String,
    pub goal: CapabilityIoType,
    pub expected: ExpectedPlanningOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ActualPlanningOutcome {
    PlanReady(ModelCapabilityPlan),
    NoEligibleModel,
    AmbiguousModel(Vec<String>),
    PlanningFailure(ModelPlanningFailure),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelPlanningBenchmarkResult {
    pub case_id: String,
    pub expected: ExpectedPlanningOutcome,
    pub actual: ActualPlanningOutcome,
    pub passed: bool,
}

pub fn cases() -> Vec<ModelPlanningBenchmarkCase> {
    vec![
        ModelPlanningBenchmarkCase {
            id: "constant-rate-plan-ready".into(),
            prompt: "A quantity changes at a constant rate of 3 per interval for 4 intervals. Find the total change.".into(),
            goal: CapabilityIoType::ExactValue,
            expected: ExpectedPlanningOutcome::PlanReady,
        },
        ModelPlanningBenchmarkCase {
            id: "missing-constancy-denied".into(),
            prompt: "A quantity changes at a rate of 3 per interval for 4 intervals. Find the total change.".into(),
            goal: CapabilityIoType::ExactValue,
            expected: ExpectedPlanningOutcome::NoEligibleModel,
        },
        ModelPlanningBenchmarkCase {
            id: "missing-target-denied".into(),
            prompt: "A quantity changes at a constant rate of 3 per interval for 4 intervals.".into(),
            goal: CapabilityIoType::ExactValue,
            expected: ExpectedPlanningOutcome::NoEligibleModel,
        },
        ModelPlanningBenchmarkCase {
            id: "unsupported-projectile-denied".into(),
            prompt: "A ball is thrown upward at 20 meters per second.".into(),
            goal: CapabilityIoType::ExactValue,
            expected: ExpectedPlanningOutcome::NoEligibleModel,
        },
        ModelPlanningBenchmarkCase {
            id: "linear-relationship-plan-ready".into(),
            prompt: "y increases by 3 for every unit increase in x, and y equals 2 when x is 0. Find y when x is 4.".into(),
            goal: CapabilityIoType::ExactValue,
            expected: ExpectedPlanningOutcome::PlanReady,
        },
        ModelPlanningBenchmarkCase {
            id: "linear-relationship-missing-baseline-denied".into(),
            prompt: "y increases by 3 for every unit increase in x. Find y when x is 4.".into(),
            goal: CapabilityIoType::ExactValue,
            expected: ExpectedPlanningOutcome::NoEligibleModel,
        },
        ModelPlanningBenchmarkCase {
            id: "proportional-plan-ready".into(),
            prompt: "y is proportional to x with proportionality constant 3. Find y when x is 4.".into(),
            goal: CapabilityIoType::ExactValue,
            expected: ExpectedPlanningOutcome::PlanReady,
        },
        ModelPlanningBenchmarkCase {
            id: "proportional-missing-constant-denied".into(),
            prompt: "y is proportional to x. Find y when x is 4.".into(),
            goal: CapabilityIoType::ExactValue,
            expected: ExpectedPlanningOutcome::NoEligibleModel,
        },
    ]
}

pub fn evaluate_cases(
    cases: Vec<ModelPlanningBenchmarkCase>,
    model_registry: &ModelConstructorRegistry,
    capability_registry: &CapabilityRegistry,
) -> Vec<ModelPlanningBenchmarkResult> {
    cases
        .into_iter()
        .map(|case| {
            let actual = match plan_model_to_goal(
                &case.prompt,
                case.goal,
                model_registry,
                capability_registry,
            ) {
                Ok(plan) => ActualPlanningOutcome::PlanReady(plan),
                Err(ModelPlanningFailure::NoEligibleModel) => {
                    ActualPlanningOutcome::NoEligibleModel
                }
                Err(ModelPlanningFailure::AmbiguousModels(ids)) => {
                    ActualPlanningOutcome::AmbiguousModel(ids)
                }
                Err(other) => ActualPlanningOutcome::PlanningFailure(other),
            };
            let passed = matches!(
                (&case.expected, &actual),
                (
                    ExpectedPlanningOutcome::PlanReady,
                    ActualPlanningOutcome::PlanReady(_)
                ) | (
                    ExpectedPlanningOutcome::NoEligibleModel,
                    ActualPlanningOutcome::NoEligibleModel
                ) | (
                    ExpectedPlanningOutcome::AmbiguousModel,
                    ActualPlanningOutcome::AmbiguousModel(_)
                )
            );
            ModelPlanningBenchmarkResult {
                case_id: case.id,
                expected: case.expected,
                actual,
                passed,
            }
        })
        .collect()
}

pub fn evaluate(
    model_registry: &ModelConstructorRegistry,
    capability_registry: &CapabilityRegistry,
) -> Vec<ModelPlanningBenchmarkResult> {
    evaluate_cases(cases(), model_registry, capability_registry)
}

pub fn production_results() -> Vec<ModelPlanningBenchmarkResult> {
    evaluate(
        &ModelConstructorRegistry::production(),
        &CapabilityRegistry::production(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn always_matches(_: &ModelEvidenceContext) -> ModelMatcherResult {
        ModelMatcherResult::eligible(vec!["synthetic discriminating evidence".into()])
    }

    fn continuous_match(context: &ModelEvidenceContext) -> ModelMatcherResult {
        let base = context
            .original_text
            .contains("value increases by 5 each step");
        let discrete = context.all_text().contains("discrete");
        if base && !discrete {
            ModelMatcherResult::eligible(vec!["value increases by 5 each step".into()])
        } else {
            ModelMatcherResult::rejected(
                "discrete interpretation selected",
                vec!["continuous interpretation".into()],
            )
        }
    }

    fn discrete_match(context: &ModelEvidenceContext) -> ModelMatcherResult {
        let base = context
            .original_text
            .contains("value increases by 5 each step");
        let continuous = context.all_text().contains("continuous");
        if base && !continuous {
            ModelMatcherResult::eligible(vec!["value increases by 5 each step".into()])
        } else {
            ModelMatcherResult::rejected(
                "continuous interpretation selected",
                vec!["discrete interpretation".into()],
            )
        }
    }

    fn synthetic_spec(id: &str) -> ModelConstructionSpec {
        ModelConstructionSpec {
            id: id.into(),
            version: 1,
            supported_language_pattern: "synthetic benchmark pattern".into(),
            required_evidence: vec!["synthetic discriminating evidence".into()],
            evidence_policy: EvidencePolicy::strict_prompt_confirmed(),
            model_artifacts: vec![ModelArtifactType::Relation],
            produced_artifacts: vec![CapabilityIoType::Expression, CapabilityIoType::BindingSet],
            introduced_assumptions: Vec::new(),
            validation_rules: vec!["synthetic validator".into()],
            quality_gate: ModelConstructionQualityGate {
                positive_cases: 1,
                negative_cases: 1,
                adversarial_cases: 1,
                unauthorized_assumptions: 0,
                replay_failures: 0,
            },
        }
    }

    #[test]
    fn production_benchmark_has_one_reachable_chain_and_safe_denials() {
        let results = production_results();
        assert_eq!(results.len(), 8);
        assert!(results.iter().all(|result| result.passed));
        let ready = results
            .iter()
            .find(|result| result.id() == "constant-rate-plan-ready")
            .expect("positive case present");
        match &ready.actual {
            ActualPlanningOutcome::PlanReady(plan) => {
                assert_eq!(plan.model_step.model_id, "constant_rate_model");
                assert_eq!(
                    plan.capability_plan.selected_capability,
                    "expression_evaluation"
                );
            }
            other => panic!("unexpected positive outcome: {other:?}"),
        }
        let linear = results
            .iter()
            .find(|result| result.id() == "linear-relationship-plan-ready")
            .expect("linear positive case present");
        match &linear.actual {
            ActualPlanningOutcome::PlanReady(plan) => {
                assert_eq!(plan.model_step.model_id, "linear_relationship_model");
                assert_eq!(
                    plan.capability_plan.selected_capability,
                    "expression_evaluation"
                );
            }
            other => panic!("unexpected linear outcome: {other:?}"),
        }
        let proportional = results
            .iter()
            .find(|result| result.id() == "proportional-plan-ready")
            .expect("proportional positive case present");
        match &proportional.actual {
            ActualPlanningOutcome::PlanReady(plan) => {
                assert_eq!(plan.model_step.model_id, "proportional_model");
                assert_eq!(
                    plan.capability_plan.selected_capability,
                    "expression_evaluation"
                );
            }
            other => panic!("unexpected proportional outcome: {other:?}"),
        }
    }

    #[test]
    fn benchmark_rejects_unsupported_projectile_text_without_model() {
        let result = production_results()
            .into_iter()
            .find(|result| result.id() == "unsupported-projectile-denied")
            .unwrap();
        assert_eq!(result.actual, ActualPlanningOutcome::NoEligibleModel);
        assert!(result.passed);
    }

    #[test]
    fn ambiguity_benchmark_reports_abstention_and_receipt() {
        let mut registry = ModelConstructorRegistry::new();
        registry
            .register(ModelConstructorEntry {
                spec: synthetic_spec("synthetic_a"),
                matcher: always_matches,
            })
            .unwrap();
        registry
            .register(ModelConstructorEntry {
                spec: synthetic_spec("synthetic_b"),
                matcher: always_matches,
            })
            .unwrap();
        let case = ModelPlanningBenchmarkCase {
            id: "ambiguous-models".into(),
            prompt: "synthetic evidence".into(),
            goal: CapabilityIoType::ExactValue,
            expected: ExpectedPlanningOutcome::AmbiguousModel,
        };
        let results = evaluate_cases(vec![case], &registry, &CapabilityRegistry::production());
        assert!(results[0].passed);
        match &results[0].actual {
            ActualPlanningOutcome::AmbiguousModel(ids) => {
                assert_eq!(
                    ids,
                    &vec![
                        String::from("synthetic_a@v1"),
                        String::from("synthetic_b@v1"),
                    ]
                );
            }
            other => panic!("expected ambiguity, got {other:?}"),
        }
        let trace = registry.discover("synthetic evidence");
        assert!(trace.ambiguity.is_some());
    }

    #[test]
    fn clarification_answer_re_resolves_model_selection() {
        let mut registry = ModelConstructorRegistry::new();
        registry
            .register(ModelConstructorEntry {
                spec: synthetic_spec("continuous_model"),
                matcher: continuous_match,
            })
            .unwrap();
        registry
            .register(ModelConstructorEntry {
                spec: synthetic_spec("discrete_model"),
                matcher: discrete_match,
            })
            .unwrap();
        let prompt = "The value increases by 5 each step.";
        let initial = registry.discover(prompt);
        let ambiguity = initial.ambiguity.expect("initial ambiguity");
        let mut state = ModelEvidenceState::new(prompt);
        state.add_answer(
            &ambiguity.clarification_request,
            "Each step represents continuous time.",
        );
        let resolved = registry.discover_with_context(&state.context());
        assert_eq!(
            resolved.selection,
            crate::constant_rate_model::ModelSelection::UniqueVersioned {
                id: "continuous_model".into(),
                version: 1,
            }
        );
        assert!(resolved.ambiguity.is_none());
    }

    impl ModelPlanningBenchmarkResult {
        fn id(&self) -> &str {
            &self.case_id
        }
    }
}
