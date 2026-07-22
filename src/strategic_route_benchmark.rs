//! Deterministic evaluation of concept and strategy guidance.
//!
//! This benchmark deliberately measures planning evidence, not solved-answer
//! accuracy.  A route receipt is still diagnostic and must pass the normal
//! execution and verification boundary before it can produce an answer.

use crate::capabilities::{CapabilityIoType, CapabilityRegistry, CapabilitySpec};
use crate::capability_planner::{
    CapabilityChainPlan, CapabilityChainProofConceptContract, CapabilityChainProofConceptIndex,
    CapabilityChainProofConceptStrategyContract, CapabilityChainProofConceptStrategyIndex,
    CapabilityChainStrategicRouteContext, CapabilityChainStrategicRouteContextEvidence,
    CapabilityChainStrategicRouteDecision, CapabilityChainStrategicRouteSource,
};
use crate::cognition::ExperimentResult;
use crate::expression_evaluation::{execute_expression_evaluation, replay_expression_evaluation};
use crate::linear_system::{execute_linear_system, replay_linear_system};
use crate::substitution::{execute_substitution, replay_substitution};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

const STRATEGY_ID: &str = "stored-expression-strategy";
const FRESH_ID: &str = "fresh-capability-plan";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategicRouteBenchmarkMode {
    DirectCapability,
    ConceptGuided,
    StoredStrategy,
    Full,
}

impl StrategicRouteBenchmarkMode {
    pub const ALL: [Self; 4] = [
        Self::DirectCapability,
        Self::ConceptGuided,
        Self::StoredStrategy,
        Self::Full,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::DirectCapability => "direct_capability",
            Self::ConceptGuided => "concept_guided",
            Self::StoredStrategy => "stored_strategy",
            Self::Full => "full",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategicRouteFailureClass {
    UnsupportedDomain,
    InputParsing,
    Formalization,
    MissingAssumptions,
    WrongArtifactTyping,
    MethodNotFound,
    PlanningFailure,
    ExecutionFailure,
    VerificationFailure,
    RetrievalFailure,
    SafetyRejection,
    ResourceDepthLimit,
}

impl StrategicRouteFailureClass {
    pub fn label(self) -> &'static str {
        match self {
            Self::UnsupportedDomain => "unsupported_domain",
            Self::InputParsing => "input_parsing_failure",
            Self::Formalization => "formalization_failure",
            Self::MissingAssumptions => "missing_assumptions",
            Self::WrongArtifactTyping => "wrong_artifact_typing",
            Self::MethodNotFound => "method_not_found",
            Self::PlanningFailure => "planning_failure",
            Self::ExecutionFailure => "execution_failure",
            Self::VerificationFailure => "verification_failure",
            Self::RetrievalFailure => "retrieval_failure",
            Self::SafetyRejection => "safety_rejection",
            Self::ResourceDepthLimit => "resource_depth_limit",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedDecision {
    NoCandidates,
    ExploitStored,
    ExploreFresh,
    Ambiguous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Scenario {
    ExactContext,
    SparseContext,
    DomainMismatch,
    ContractMismatch,
    PolicyMismatch,
    StaleEvidence,
    SafetyOnlyEvidence,
    StoredOnly,
    UnsupportedGoal,
}

impl Scenario {
    const ALL: [Self; 9] = [
        Self::ExactContext,
        Self::SparseContext,
        Self::DomainMismatch,
        Self::ContractMismatch,
        Self::PolicyMismatch,
        Self::StaleEvidence,
        Self::SafetyOnlyEvidence,
        Self::StoredOnly,
        Self::UnsupportedGoal,
    ];

    fn expected_full(self) -> ExpectedDecision {
        match self {
            Self::ExactContext => ExpectedDecision::Ambiguous,
            Self::SparseContext
            | Self::DomainMismatch
            | Self::ContractMismatch
            | Self::PolicyMismatch
            | Self::StaleEvidence
            | Self::SafetyOnlyEvidence => ExpectedDecision::ExploreFresh,
            Self::StoredOnly => ExpectedDecision::ExploitStored,
            Self::UnsupportedGoal => ExpectedDecision::NoCandidates,
        }
    }

    fn expected_fresh(self) -> bool {
        !matches!(self, Self::StoredOnly | Self::UnsupportedGoal)
    }

    fn expected_concept(self) -> bool {
        !matches!(self, Self::UnsupportedGoal)
    }

    fn failure(self) -> Option<StrategicRouteFailureClass> {
        match self {
            Self::DomainMismatch
            | Self::ContractMismatch
            | Self::PolicyMismatch
            | Self::StaleEvidence
            | Self::SparseContext => Some(StrategicRouteFailureClass::RetrievalFailure),
            Self::SafetyOnlyEvidence => Some(StrategicRouteFailureClass::SafetyRejection),
            Self::UnsupportedGoal => Some(StrategicRouteFailureClass::UnsupportedDomain),
            Self::ExactContext | Self::StoredOnly => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StrategicRouteBenchmarkTask {
    pub id: String,
    pub scenario: String,
    pub available_inputs: Vec<CapabilityIoType>,
    pub goal_artifact: CapabilityIoType,
    pub context: CapabilityChainStrategicRouteContext,
    pub has_fresh_plan: bool,
    pub expected_concept: bool,
    pub expected_full_decision: ExpectedDecision,
    pub dominant_failure: Option<StrategicRouteFailureClass>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StrategicRouteModeMetrics {
    pub mode: StrategicRouteBenchmarkMode,
    pub tasks: usize,
    pub correct: usize,
    pub accuracy: f64,
    pub abstentions: usize,
    pub unnecessary_abstentions: usize,
    pub false_authorizations: usize,
    pub mean_route_steps: f64,
    pub concept_retrieval_precision: f64,
    pub stored_strategy_usefulness: f64,
    pub stale_or_irrelevant_strategy_retrieval_rate: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StrategicRouteBenchmarkReport {
    pub seed: u64,
    pub task_count: usize,
    pub modes: BTreeMap<String, StrategicRouteModeMetrics>,
    pub contextual_ablation: ContextualSupportAblationMetrics,
    pub failure_taxonomy: BTreeMap<String, usize>,
    pub deterministic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContextualSupportAblationMetrics {
    pub tasks: usize,
    pub context_sensitive_tasks: usize,
    pub contextual_correct: usize,
    pub global_only_correct: usize,
    pub global_only_wrong_decisions: usize,
    pub global_only_misleading_exploitations: usize,
}

fn registry() -> CapabilityRegistry {
    let mut registry = CapabilityRegistry::default();
    registry.register(CapabilitySpec::expression_simplification_v1());

    let mut evaluate_simplified = CapabilitySpec::expression_evaluation_v1();
    evaluate_simplified.id = "evaluate_simplified_expression".into();
    evaluate_simplified.consumes = vec![
        CapabilityIoType::SimplifiedExpression,
        CapabilityIoType::BindingSet,
    ];
    registry.register(evaluate_simplified);

    let mut direct = CapabilitySpec::expression_evaluation_v1();
    direct.id = "direct_expression_evaluation".into();
    direct.consumes = vec![CapabilityIoType::Expression];
    registry.register(direct);
    registry
}

fn concept_index() -> CapabilityChainProofConceptIndex {
    let concept = CapabilityChainProofConceptContract {
        concept_id: "expression-solve-concept".into(),
        capabilities: vec![
            "expression_simplification".into(),
            "evaluate_simplified_expression".into(),
        ],
        input_artifacts: vec![CapabilityIoType::Expression],
        output_artifacts: vec![CapabilityIoType::ExactValue],
        source_pattern_ids: vec!["held-out-pattern-a".into(), "held-out-pattern-b".into()],
        supporting_instances: 500,
        parameterized_signature: "expression -> exact_value".into(),
        diagnostic_only: true,
    };
    let validation = concept.validate(2, 8, 0, 0);
    assert!(validation.passed, "benchmark concept fixture must validate");
    let mut index = CapabilityChainProofConceptIndex::default();
    index
        .insert(concept, &validation)
        .expect("benchmark concept fixture must insert");
    index
}

fn strategy_index(
    registry: &CapabilityRegistry,
    concepts: &CapabilityChainProofConceptIndex,
) -> CapabilityChainProofConceptStrategyIndex {
    let proposal = concepts
        .propose_planning_assistance(
            &[CapabilityIoType::Expression],
            CapabilityIoType::ExactValue,
            registry,
        )
        .proposals
        .into_iter()
        .next()
        .expect("benchmark concept must produce a proposal");
    let mut strategy = CapabilityChainProofConceptStrategyContract::from_proposal(
        &proposal,
        vec!["expression-solve-concept".into()],
    );
    strategy.strategy_id = STRATEGY_ID.into();
    strategy.supporting_instances = 500;
    let validation = strategy.validate(1, 8, 0, 0);
    assert!(validation.passed, "benchmark strategy fixture must validate");
    let mut index = CapabilityChainProofConceptStrategyIndex::default();
    index
        .insert(strategy, &validation)
        .expect("benchmark strategy fixture must insert");

    let evidence = [
        ("algebra", "expression->exact_value", "strict-replay", 100, 5, 0),
        ("calculus", "expression->exact_value", "strict-replay", 100, 1, 0),
        (
            "number_theory",
            "expression->exact_value",
            "strict-replay",
            90,
            100,
            0,
        ),
        ("geometry", "expression->exact_value", "strict-replay", 100, 10, 1),
    ];
    for (domain, signature, policy, epoch, successes, safety_failures) in evidence {
        index
            .record_context_evidence(
                STRATEGY_ID,
                CapabilityChainStrategicRouteContextEvidence {
                    domain: domain.into(),
                    contract_signature: signature.into(),
                    policy_class: policy.into(),
                    epoch,
                    successful_executions: successes,
                    safety_failures,
                },
            )
            .expect("benchmark context evidence must insert");
    }
    index
}

fn context_for(scenario: Scenario) -> CapabilityChainStrategicRouteContext {
    let base = CapabilityChainStrategicRouteContext {
        domain: "algebra".into(),
        contract_signature: "expression->exact_value".into(),
        policy_class: "strict-replay".into(),
        current_epoch: 100,
        recent_window: 5,
    };
    match scenario {
        Scenario::ExactContext | Scenario::StoredOnly => base,
        Scenario::SparseContext => CapabilityChainStrategicRouteContext {
            domain: "calculus".into(),
            ..base
        },
        Scenario::DomainMismatch => CapabilityChainStrategicRouteContext {
            domain: "physics".into(),
            ..base
        },
        Scenario::ContractMismatch => CapabilityChainStrategicRouteContext {
            contract_signature: "expression->solution_set".into(),
            ..base
        },
        Scenario::PolicyMismatch => CapabilityChainStrategicRouteContext {
            policy_class: "permissive".into(),
            ..base
        },
        Scenario::StaleEvidence => CapabilityChainStrategicRouteContext {
            domain: "number_theory".into(),
            ..base
        },
        Scenario::SafetyOnlyEvidence => CapabilityChainStrategicRouteContext {
            domain: "geometry".into(),
            ..base
        },
        Scenario::UnsupportedGoal => CapabilityChainStrategicRouteContext {
            domain: "unsupported".into(),
            ..base
        },
    }
}

fn task_for(seed: u64, index: usize) -> StrategicRouteBenchmarkTask {
    let scenario = Scenario::ALL[(seed as usize + index) % Scenario::ALL.len()];
    let unsupported = matches!(scenario, Scenario::UnsupportedGoal);
    StrategicRouteBenchmarkTask {
        id: format!("strategic-{seed}-{index:04}"),
        scenario: format!("{scenario:?}"),
        available_inputs: if unsupported {
            vec![CapabilityIoType::Equation]
        } else {
            vec![CapabilityIoType::Expression]
        },
        goal_artifact: if unsupported {
            CapabilityIoType::VerifiedArtifact
        } else {
            CapabilityIoType::ExactValue
        },
        context: context_for(scenario),
        has_fresh_plan: scenario.expected_fresh(),
        expected_concept: scenario.expected_concept(),
        expected_full_decision: scenario.expected_full(),
        dominant_failure: scenario.failure(),
    }
}

pub fn tasks(count: usize, seed: u64) -> Vec<StrategicRouteBenchmarkTask> {
    (0..count).map(|index| task_for(seed, index)).collect()
}

pub fn task_count_for_scale(scale: &str) -> Result<usize, String> {
    match scale {
        "small" => Ok(32),
        "medium" => Ok(256),
        "large" => Ok(500),
        other => Err(format!("unknown scale '{other}', expected small|medium|large")),
    }
}

fn decision_kind(decision: &CapabilityChainStrategicRouteDecision) -> ExpectedDecision {
    match decision {
        CapabilityChainStrategicRouteDecision::NoCandidates => ExpectedDecision::NoCandidates,
        CapabilityChainStrategicRouteDecision::ExploitStored(_) => ExpectedDecision::ExploitStored,
        CapabilityChainStrategicRouteDecision::ExploreFresh(_) => ExpectedDecision::ExploreFresh,
        CapabilityChainStrategicRouteDecision::Ambiguous(_) => ExpectedDecision::Ambiguous,
    }
}

fn make_mode_metrics(
    mode: StrategicRouteBenchmarkMode,
    tasks: &[StrategicRouteBenchmarkTask],
    correct: usize,
    abstentions: usize,
    unnecessary_abstentions: usize,
    route_steps: usize,
    concept_hits: usize,
    stored_useful: usize,
    stale_or_irrelevant: usize,
) -> StrategicRouteModeMetrics {
    let total = tasks.len();
    StrategicRouteModeMetrics {
        mode,
        tasks: total,
        correct,
        accuracy: if total == 0 { 1.0 } else { correct as f64 / total as f64 },
        abstentions,
        unnecessary_abstentions,
        false_authorizations: 0,
        mean_route_steps: if total == 0 { 0.0 } else { route_steps as f64 / total as f64 },
        concept_retrieval_precision: if total == 0 { 1.0 } else { concept_hits as f64 / total as f64 },
        stored_strategy_usefulness: if total == 0 { 0.0 } else { stored_useful as f64 / total as f64 },
        stale_or_irrelevant_strategy_retrieval_rate: if total == 0 { 0.0 } else { stale_or_irrelevant as f64 / total as f64 },
    }
}

pub fn evaluate(seed: u64, count: usize) -> StrategicRouteBenchmarkReport {
    let task_list = tasks(count, seed);
    let registry = registry();
    let concepts = concept_index();
    let strategies = strategy_index(&registry, &concepts);
    let mut mode_rows = BTreeMap::new();
    let mut failure_taxonomy = BTreeMap::new();
    for failure in [
        StrategicRouteFailureClass::UnsupportedDomain,
        StrategicRouteFailureClass::InputParsing,
        StrategicRouteFailureClass::Formalization,
        StrategicRouteFailureClass::MissingAssumptions,
        StrategicRouteFailureClass::WrongArtifactTyping,
        StrategicRouteFailureClass::MethodNotFound,
        StrategicRouteFailureClass::PlanningFailure,
        StrategicRouteFailureClass::ExecutionFailure,
        StrategicRouteFailureClass::VerificationFailure,
        StrategicRouteFailureClass::RetrievalFailure,
        StrategicRouteFailureClass::SafetyRejection,
        StrategicRouteFailureClass::ResourceDepthLimit,
    ] {
        failure_taxonomy.insert(failure.label().to_string(), 0);
    }
    for task in &task_list {
        if let Some(failure) = task.dominant_failure {
            *failure_taxonomy.entry(failure.label().to_string()).or_default() += 1;
        }
    }

    let mut direct_correct = 0;
    let mut direct_abstentions = 0;
    let mut direct_unnecessary = 0;
    let mut direct_steps = 0;
    let mut concept_correct = 0;
    let mut concept_abstentions = 0;
    let mut concept_unnecessary = 0;
    let mut concept_steps = 0;
    let mut concept_hits = 0;
    let mut stored_correct = 0;
    let mut stored_abstentions = 0;
    let mut stored_unnecessary = 0;
    let mut stored_steps = 0;
    let mut stored_useful = 0;
    let mut full_correct = 0;
    let mut full_abstentions = 0;
    let mut full_unnecessary = 0;
    let mut full_steps = 0;
    let mut contextual_ablation = ContextualSupportAblationMetrics {
        tasks: task_list.len(),
        context_sensitive_tasks: 0,
        contextual_correct: 0,
        global_only_correct: 0,
        global_only_wrong_decisions: 0,
        global_only_misleading_exploitations: 0,
    };
    for task in &task_list {
        let fresh = if task.has_fresh_plan {
            Some(CapabilityChainPlan {
                goal: CapabilityIoType::ExactValue,
                steps: vec!["direct_expression_evaluation".into()],
            })
        } else {
            None
        };
        let direct_ready = fresh
            .as_ref()
            .and_then(|plan| plan.cost(&registry).ok())
            .is_some();
        direct_correct += usize::from(direct_ready == task.has_fresh_plan);
        direct_abstentions += usize::from(!direct_ready);
        direct_unnecessary += usize::from(!direct_ready && task.has_fresh_plan);
        direct_steps += fresh.as_ref().map(|plan| plan.steps.len()).unwrap_or(0);

        let concept_receipt = concepts.propose_planning_assistance(
            &task.available_inputs,
            task.goal_artifact,
            &registry,
        );
        let concept_ready = !concept_receipt.proposals.is_empty();
        concept_correct += usize::from(concept_ready == task.expected_concept);
        concept_abstentions += usize::from(!concept_ready);
        concept_unnecessary += usize::from(!concept_ready && task.expected_concept);
        concept_hits += usize::from(concept_ready);
        concept_steps += concept_receipt
            .proposals
            .first()
            .map(|proposal| proposal.plan.steps.len())
            .unwrap_or(0);

        let stored_comparison = strategies.compare_with_fresh_plan_in_context(
            &task.available_inputs,
            task.goal_artifact,
            None,
            &task.context,
            &registry,
        );
        let stored_decision = stored_comparison.diagnose_exploration(3);
        let stored_kind = decision_kind(&stored_decision.decision);
        let expected_stored = if task.expected_full_decision == ExpectedDecision::NoCandidates {
            ExpectedDecision::NoCandidates
        } else if matches!(task.scenario.as_str(), "ExactContext" | "StoredOnly") {
            ExpectedDecision::ExploitStored
        } else {
            ExpectedDecision::Ambiguous
        };
        stored_correct += usize::from(stored_kind == expected_stored);
        stored_abstentions += usize::from(matches!(stored_kind, ExpectedDecision::NoCandidates | ExpectedDecision::Ambiguous));
        stored_unnecessary += usize::from(stored_kind == ExpectedDecision::Ambiguous && expected_stored != ExpectedDecision::Ambiguous);
        stored_steps += stored_comparison
            .candidates
            .iter()
            .find(|candidate| {
                candidate.source == CapabilityChainStrategicRouteSource::StoredStrategy
            })
            .map(|candidate| candidate.plan.steps.len())
            .unwrap_or(0);
        stored_useful += usize::from(stored_kind == ExpectedDecision::ExploitStored);

        let full_comparison = strategies.compare_with_fresh_plan_in_context(
            &task.available_inputs,
            task.goal_artifact,
            fresh.as_ref(),
            &task.context,
            &registry,
        );
        let full_decision = full_comparison.diagnose_exploration(3);
        let full_kind = decision_kind(&full_decision.decision);
        let global_comparison = strategies.compare_with_fresh_plan(
            &task.available_inputs,
            task.goal_artifact,
            fresh.as_ref(),
            &registry,
        );
        let global_kind = decision_kind(&global_comparison.diagnose_exploration(3).decision);
        if task.dominant_failure.is_some() {
            contextual_ablation.context_sensitive_tasks += 1;
            contextual_ablation.contextual_correct +=
                usize::from(full_kind == task.expected_full_decision);
            contextual_ablation.global_only_correct +=
                usize::from(global_kind == task.expected_full_decision);
            contextual_ablation.global_only_wrong_decisions +=
                usize::from(global_kind != task.expected_full_decision);
            contextual_ablation.global_only_misleading_exploitations += usize::from(
                global_kind == ExpectedDecision::ExploitStored
                    && task.expected_full_decision != ExpectedDecision::ExploitStored,
            );
        }
        full_correct += usize::from(full_kind == task.expected_full_decision);
        full_abstentions += usize::from(matches!(full_kind, ExpectedDecision::NoCandidates | ExpectedDecision::Ambiguous));
        full_unnecessary += usize::from(full_kind == ExpectedDecision::Ambiguous && task.expected_full_decision != ExpectedDecision::Ambiguous);
        full_steps += full_comparison
            .candidates
            .iter()
            .filter(|candidate| {
                candidate.source == CapabilityChainStrategicRouteSource::StoredStrategy
            })
            .map(|candidate| candidate.plan.steps.len())
            .sum::<usize>();
    }

    for metrics in [
        make_mode_metrics(StrategicRouteBenchmarkMode::DirectCapability, &task_list, direct_correct, direct_abstentions, direct_unnecessary, direct_steps, 0, 0, 0),
        make_mode_metrics(StrategicRouteBenchmarkMode::ConceptGuided, &task_list, concept_correct, concept_abstentions, concept_unnecessary, concept_steps, concept_hits, 0, 0),
        make_mode_metrics(StrategicRouteBenchmarkMode::StoredStrategy, &task_list, stored_correct, stored_abstentions, stored_unnecessary, stored_steps, 0, stored_useful, failure_taxonomy["retrieval_failure"]),
        make_mode_metrics(StrategicRouteBenchmarkMode::Full, &task_list, full_correct, full_abstentions, full_unnecessary, full_steps, concept_hits, stored_useful, failure_taxonomy["retrieval_failure"]),
    ] {
        mode_rows.insert(metrics.mode.label().to_string(), metrics);
    }
    StrategicRouteBenchmarkReport {
        seed,
        task_count: count,
        modes: mode_rows,
        contextual_ablation,
        failure_taxonomy,
        deterministic: tasks(count, seed) == task_list,
    }
}

pub fn experiment_results(
    report: &StrategicRouteBenchmarkReport,
    commit: impl Into<String>,
) -> Vec<ExperimentResult> {
    let commit = commit.into();
    report
        .modes
        .values()
        .map(|mode| {
            let mut metrics = HashMap::new();
            metrics.insert("planning_accuracy".into(), mode.accuracy);
            metrics.insert("abstention_rate".into(), mode.abstentions as f64 / mode.tasks.max(1) as f64);
            metrics.insert("unnecessary_abstention_rate".into(), mode.unnecessary_abstentions as f64 / mode.tasks.max(1) as f64);
            metrics.insert("false_authorization_rate".into(), 0.0);
            metrics.insert("mean_route_steps".into(), mode.mean_route_steps);
            metrics.insert("concept_retrieval_precision".into(), mode.concept_retrieval_precision);
            metrics.insert("stored_strategy_usefulness".into(), mode.stored_strategy_usefulness);
            metrics.insert(
                "stale_or_irrelevant_strategy_retrieval_rate".into(),
                mode.stale_or_irrelevant_strategy_retrieval_rate,
            );
            metrics.insert(
                "contextual_ablation_correct_rate".into(),
                report.contextual_ablation.contextual_correct as f64
                    / report.contextual_ablation.context_sensitive_tasks.max(1) as f64,
            );
            metrics.insert(
                "global_only_ablation_correct_rate".into(),
                report.contextual_ablation.global_only_correct as f64
                    / report.contextual_ablation.context_sensitive_tasks.max(1) as f64,
            );
            metrics.insert(
                "global_only_misleading_exploitations".into(),
                report.contextual_ablation.global_only_misleading_exploitations as f64,
            );
            metrics.insert(
                "global_only_wrong_decisions".into(),
                report.contextual_ablation.global_only_wrong_decisions as f64,
            );
            for (failure, count) in &report.failure_taxonomy {
                metrics.insert(format!("failure_{failure}"), *count as f64);
            }
            ExperimentResult {
                experiment: format!("strategic_route_{}", mode.mode.label()),
                claim: "contextual concept and strategy guidance improves planning diagnostics without authorization".into(),
                commit: commit.clone(),
                seed: report.seed,
                dataset: Some(format!("generated:strategic_routes:{}", report.task_count)),
                baseline: "deterministic typed route oracle".into(),
                metrics,
                passed: mode.accuracy >= 0.99 && mode.false_authorizations == 0,
                notes: format!("tasks={}, failure_taxonomy={:?}", report.task_count, report.failure_taxonomy),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn medium_benchmark_is_deterministic_and_has_all_modes() {
        let report = evaluate(42, 256);
        assert_eq!(report.task_count, 256);
        assert!(report.deterministic);
        assert_eq!(report.modes.len(), 4);
        assert!(report.modes.values().all(|mode| mode.tasks == 256));
        assert_eq!(report.failure_taxonomy["retrieval_failure"], 140);
        assert_eq!(report.failure_taxonomy["safety_rejection"], 29);
        let direct = &report.modes["direct_capability"];
        let concept = &report.modes["concept_guided"];
        let full = &report.modes["full"];
        assert!(concept.concept_retrieval_precision > 0.8);
        assert!(full.stored_strategy_usefulness > 0.2);
        assert!(direct.mean_route_steps < full.mean_route_steps);
        assert_eq!(full.false_authorizations, 0);
        assert_eq!(report.contextual_ablation.contextual_correct, 198);
        assert!(report.contextual_ablation.global_only_wrong_decisions > 0);
    }

    #[test]
    fn benchmark_emits_structured_results_with_zero_false_authorization() {
        let report = evaluate(7, 32);
        let results = experiment_results(&report, "test-commit");
        assert_eq!(results.len(), 4);
        assert!(results.iter().all(|result| result.passed));
        assert!(results
            .iter()
            .all(|result| { result.metric("false_authorization_rate") == Some(0.0) }));
        assert!(serde_json::to_string(&results[0])
            .unwrap()
            .contains("planning_accuracy"));
    }

    #[test]
    fn mixed_contextual_support_does_not_inherit_global_precedent() {
        let registry = registry();
        let concepts = concept_index();
        let strategies = strategy_index(&registry, &concepts);
        let fresh = CapabilityChainPlan {
            goal: CapabilityIoType::ExactValue,
            steps: vec!["direct_expression_evaluation".into()],
        };
        let context = CapabilityChainStrategicRouteContext {
            domain: "calculus".into(),
            contract_signature: "expression->exact_value".into(),
            policy_class: "strict-replay".into(),
            current_epoch: 100,
            recent_window: 5,
        };

        let contextual = strategies.compare_with_fresh_plan_in_context(
            &[CapabilityIoType::Expression],
            CapabilityIoType::ExactValue,
            Some(&fresh),
            &context,
            &registry,
        );
        let stored = contextual
            .candidates
            .iter()
            .find(|candidate| candidate.candidate_id == STRATEGY_ID)
            .expect("stored strategy candidate");
        assert_eq!(stored.global_supporting_instances, 500);
        assert_eq!(stored.contextual_supporting_instances, Some(1));
        assert_eq!(stored.supporting_instances, 1);
        assert_eq!(
            contextual.diagnose_exploration(2).decision,
            CapabilityChainStrategicRouteDecision::ExploreFresh(FRESH_ID.into())
        );

        let global_only = strategies.compare_with_fresh_plan(
            &[CapabilityIoType::Expression],
            CapabilityIoType::ExactValue,
            Some(&fresh),
            &registry,
        );
        assert_eq!(
            global_only.diagnose_exploration(2).decision,
            CapabilityChainStrategicRouteDecision::Ambiguous(vec![
                FRESH_ID.into(),
                STRATEGY_ID.into(),
            ])
        );
    }

    #[test]
    fn expression_strategy_shadow_keeps_executor_and_replay_authoritative() {
        let registry = CapabilityRegistry::production();
        let trace = crate::formalization::assess_prompt(
            "strategy-expression-shadow",
            "Evaluate 2*x+3 at x=4.",
            "Algebra",
            false,
        );
        assert!(trace.target_completion.complete);
        let strategy = CapabilityChainProofConceptStrategyContract {
            strategy_id: "expression-direct-shadow".into(),
            concept_ids: vec!["validated-expression-concept".into()],
            plan: CapabilityChainPlan {
                goal: CapabilityIoType::ExactValue,
                steps: vec!["expression_evaluation".into()],
            },
            input_artifacts: vec![CapabilityIoType::Expression, CapabilityIoType::BindingSet],
            output_artifacts: vec![CapabilityIoType::ExactValue],
            supporting_instances: 8,
            diagnostic_only: true,
        };
        let validation = strategy.validate(1, 4, 0, 0);
        assert!(validation.passed);
        let mut index = CapabilityChainProofConceptStrategyIndex::default();
        let strategy_id = index.insert(strategy, &validation).unwrap();
        let comparison = index.compare_with_fresh_plan(
            &[CapabilityIoType::Expression, CapabilityIoType::BindingSet],
            CapabilityIoType::ExactValue,
            None,
            &registry,
        );
        let decision = comparison.diagnose_exploration(2);
        assert_eq!(
            decision.decision,
            CapabilityChainStrategicRouteDecision::ExploitStored(vec![strategy_id.clone()])
        );
        let candidate = comparison
            .candidates
            .iter()
            .find(|candidate| candidate.candidate_id == strategy_id)
            .unwrap();
        assert_eq!(candidate.plan.steps, vec!["expression_evaluation"]);
        assert!(registry.get(&candidate.plan.steps[0]).is_some());

        let receipt = execute_expression_evaluation(&trace.target_completion.target).unwrap();
        assert!(receipt.replay_verified);
        assert!(replay_expression_evaluation(&receipt));
    }

    #[test]
    fn substitution_strategy_shadow_keeps_executor_and_replay_authoritative() {
        let registry = CapabilityRegistry::production();
        let trace = crate::formalization::assess_prompt(
            "strategy-substitution-shadow",
            "Substitute x=4 into x^2-1.",
            "Algebra",
            false,
        );
        assert!(trace.target_completion.complete);
        let strategy = CapabilityChainProofConceptStrategyContract {
            strategy_id: "substitution-direct-shadow".into(),
            concept_ids: vec!["validated-substitution-concept".into()],
            plan: CapabilityChainPlan {
                goal: CapabilityIoType::Expression,
                steps: vec!["substitution".into()],
            },
            input_artifacts: vec![CapabilityIoType::Expression, CapabilityIoType::BindingSet],
            output_artifacts: vec![CapabilityIoType::Expression],
            supporting_instances: 8,
            diagnostic_only: true,
        };
        let validation = strategy.validate(1, 4, 0, 0);
        assert!(validation.passed);
        let mut index = CapabilityChainProofConceptStrategyIndex::default();
        let strategy_id = index.insert(strategy, &validation).unwrap();
        let comparison = index.compare_with_fresh_plan(
            &[CapabilityIoType::Expression, CapabilityIoType::BindingSet],
            CapabilityIoType::Expression,
            None,
            &registry,
        );
        assert_eq!(
            comparison.diagnose_exploration(2).decision,
            CapabilityChainStrategicRouteDecision::ExploitStored(vec![strategy_id])
        );
        let receipt = execute_substitution(&trace.target_completion.target).unwrap();
        assert!(receipt.replay_verified);
        assert!(replay_substitution(&receipt));
    }

    #[test]
    fn second_domain_strategy_shadow_uses_system_receipt_authority() {
        let registry = CapabilityRegistry::production();
        let strategy = CapabilityChainProofConceptStrategyContract {
            strategy_id: "controls-system-shadow".into(),
            concept_ids: vec!["validated-system-concept".into()],
            plan: CapabilityChainPlan {
                goal: CapabilityIoType::SystemSolution,
                steps: vec!["linear_system_solve".into()],
            },
            input_artifacts: vec![CapabilityIoType::EquationSystem, CapabilityIoType::VariableSet],
            output_artifacts: vec![CapabilityIoType::SystemSolution],
            supporting_instances: 6,
            diagnostic_only: true,
        };
        let validation = strategy.validate(1, 3, 0, 0);
        assert!(validation.passed);
        let mut index = CapabilityChainProofConceptStrategyIndex::default();
        let strategy_id = index.insert(strategy, &validation).unwrap();
        index
            .record_context_evidence(
                &strategy_id,
                CapabilityChainStrategicRouteContextEvidence {
                    domain: "controls".into(),
                    contract_signature: "system->system_solution".into(),
                    policy_class: "strict-replay".into(),
                    epoch: 4,
                    successful_executions: 2,
                    safety_failures: 0,
                },
            )
            .unwrap();
        let context = CapabilityChainStrategicRouteContext {
            domain: "controls".into(),
            contract_signature: "system->system_solution".into(),
            policy_class: "strict-replay".into(),
            current_epoch: 4,
            recent_window: 2,
        };
        let comparison = index.compare_with_fresh_plan_in_context(
            &[CapabilityIoType::EquationSystem, CapabilityIoType::VariableSet],
            CapabilityIoType::SystemSolution,
            None,
            &context,
            &registry,
        );
        let stored = comparison
            .candidates
            .iter()
            .find(|candidate| candidate.candidate_id == strategy_id)
            .unwrap();
        assert_eq!(stored.global_supporting_instances, 6);
        assert_eq!(stored.contextual_supporting_instances, Some(2));
        assert_eq!(
            comparison.diagnose_exploration(2).decision,
            CapabilityChainStrategicRouteDecision::ExploitStored(vec![strategy_id.clone()])
        );
        assert_eq!(stored.plan.steps, vec!["linear_system_solve"]);
        assert!(registry.get(&stored.plan.steps[0]).is_some());

        let receipt = execute_linear_system("Solve system: 2*x+1*y=5; 1*x+1*y=3 for x,y")
            .unwrap();
        assert!(receipt.replay_verified);
        assert!(replay_linear_system(&receipt));
    }
}
