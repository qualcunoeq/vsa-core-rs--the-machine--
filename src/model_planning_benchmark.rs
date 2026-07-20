//! Small, deterministic benchmark for the model-to-capability planning loop.
//!
//! This is intentionally a planning benchmark, not an answer benchmark.  It
//! measures whether text yields a uniquely authorized model and a compatible
//! downstream capability plan.  Execution remains owned by the model and
//! capability verifiers.

use crate::capabilities::{CapabilityIoType, CapabilityRegistry};
use crate::capability_planner::{
    plan_model_to_goal, ModelCapabilityPlan, ModelPlanningFailure,
};
use crate::constant_rate_model::{ModelConstructorRegistry, ModelSelection};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedPlanningOutcome {
    PlanReady,
    NoEligibleModel,
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
    ]
}

pub fn evaluate(
    model_registry: &ModelConstructorRegistry,
    capability_registry: &CapabilityRegistry,
) -> Vec<ModelPlanningBenchmarkResult> {
    cases()
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
                (ExpectedPlanningOutcome::PlanReady, ActualPlanningOutcome::PlanReady(_))
                    | (ExpectedPlanningOutcome::NoEligibleModel, ActualPlanningOutcome::NoEligibleModel)
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

pub fn production_results() -> Vec<ModelPlanningBenchmarkResult> {
    evaluate(
        &ModelConstructorRegistry::production(),
        &CapabilityRegistry::production(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_benchmark_has_one_reachable_chain_and_safe_denials() {
        let results = production_results();
        assert_eq!(results.len(), 4);
        assert!(results.iter().all(|result| result.passed));
        let ready = results
            .iter()
            .find(|result| result.id() == "constant-rate-plan-ready")
            .expect("positive case present");
        match &ready.actual {
            ActualPlanningOutcome::PlanReady(plan) => {
                assert_eq!(plan.model_step.model_id, "constant_rate_model");
                assert_eq!(plan.capability_plan.selected_capability, "expression_evaluation");
            }
            other => panic!("unexpected positive outcome: {other:?}"),
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

    impl ModelPlanningBenchmarkResult {
        fn id(&self) -> &str {
            &self.case_id
        }
    }
}
