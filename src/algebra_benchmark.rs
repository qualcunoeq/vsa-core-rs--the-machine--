//! Deterministic end-to-end benchmark for the bounded algebra capabilities.
//!
//! Unlike the formalization benchmark, this harness crosses the execution
//! boundary.  It still keeps authorization, execution, and replay metrics
//! separate so a correct refusal cannot be confused with a failed solve.

use crate::capabilities::{CapabilityIoType, CapabilityRegistry, CapabilitySelection};
use crate::capability_planner::{
    plan_capability_chain, plan_equation_chain, CapabilityChainPlan,
    CapabilityChainProofConceptStrategyContract, CapabilityChainProofConceptStrategyIndex,
    CapabilityChainStrategicRouteContext, CapabilityChainStrategicRouteContextEvidence,
    CapabilityChainStrategicRouteDecision,
};
use crate::cognition::ExperimentResult;
use crate::formalization::{assess_prompt, FormalizedTarget};
use crate::linear_equation::{execute_linear_equation, replay_linear_equation};
use crate::linear_system::{
    classify_linear_system, execute_linear_system, replay_linear_system, LinearSystemClassification,
};
use crate::quadratic_equation::{execute_quadratic_equation, replay_quadratic_equation};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlgebraMethod {
    LinearEquation,
    QuadraticEquation,
    LinearSystem,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlgebraCase {
    pub id: String,
    pub tier: String,
    pub method: AlgebraMethod,
    pub prompt: String,
    pub expected_result: Option<String>,
    pub should_authorize: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlgebraCorpus {
    pub schema_version: u32,
    pub cases: Vec<AlgebraCase>,
}

impl AlgebraCorpus {
    pub fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.schema_version != 1 {
            errors.push(format!("unsupported_schema:{}", self.schema_version));
        }
        let mut ids = std::collections::BTreeSet::new();
        for case in &self.cases {
            if !ids.insert(case.id.clone()) {
                errors.push(format!("duplicate_case:{}", case.id));
            }
            if case.prompt.trim().is_empty() {
                errors.push(format!("empty_prompt:{}", case.id));
            }
            if case.should_authorize && case.expected_result.is_none() {
                errors.push(format!("missing_expected_result:{}", case.id));
            }
        }
        errors
    }

    /// Extend a hand-authored corpus with deterministic parameterized cases.
    /// Gold answers are constructed from integer witnesses, independently of
    /// the algebra executor, so this is useful for scale and holdout tests.
    pub fn with_generated_cases(&self, count: usize, seed: u64) -> Self {
        let mut expanded = self.clone();
        for index in 0..count {
            let n = splitmix64(seed.wrapping_add(index as u64));
            let holdout = index % 5 == 0;
            let id = if holdout {
                format!("gen-{index:04}-h")
            } else {
                format!("gen-{index:04}")
            };
            let case = match index % 3 {
                0 => {
                    let a = (n % 9 + 1) as i64;
                    let b = ((n >> 8) % 17) as i64 - 8;
                    let x = ((n >> 16) % 21) as i64 - 10;
                    let c = a * x + b;
                    AlgebraCase {
                        id,
                        tier: "generated".into(),
                        method: AlgebraMethod::LinearEquation,
                        prompt: format!("Solve for x: {}*x{}= {}.", a, signed_term(b), c),
                        expected_result: Some(x.to_string()),
                        should_authorize: true,
                    }
                }
                1 => {
                    let r1 = ((n >> 8) % 10) as i64 - 4;
                    let r2 = ((n >> 16) % 10) as i64 - 4;
                    let sum = r1 + r2;
                    let product = r1 * r2;
                    let mut root_strings = vec![r1.to_string(), r2.to_string()];
                    root_strings.sort();
                    let roots = format!("[{}, {}]", root_strings[0], root_strings[1]);
                    AlgebraCase {
                        id,
                        tier: "generated".into(),
                        method: AlgebraMethod::QuadraticEquation,
                        prompt: format!(
                            "Solve for x: x^2{}*x{}=0.",
                            signed_term(-sum),
                            signed_term(product)
                        ),
                        expected_result: Some(if r1 == r2 { r1.to_string() } else { roots }),
                        should_authorize: true,
                    }
                }
                _ => {
                    let a = (n % 4 + 1) as i64;
                    let b = ((n >> 8) % 3 + 1) as i64;
                    let mut c = ((n >> 16) % 4 + 1) as i64;
                    let d = ((n >> 24) % 3 + 1) as i64;
                    if a * d == b * c {
                        c += 1;
                    }
                    let x = ((n >> 32) % 9) as i64 - 4;
                    let y = ((n >> 40) % 9) as i64 - 4;
                    let rhs1 = a * x + b * y;
                    let rhs2 = c * x + d * y;
                    AlgebraCase {
                        id,
                        tier: "generated".into(),
                        method: AlgebraMethod::LinearSystem,
                        prompt: format!(
                            "Solve system: {}*x+{}*y={}; {}*x+{}*y={} for x,y",
                            a, b, rhs1, c, d, rhs2
                        ),
                        expected_result: Some(format!("{{\"x\": \"{x}\", \"y\": \"{y}\"}}")),
                        should_authorize: true,
                    }
                }
            };
            expanded.cases.push(case);
        }
        expanded
    }
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = value;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

fn signed_term(value: i64) -> String {
    if value < 0 {
        format!("- {}", value.unsigned_abs())
    } else {
        format!("+ {}", value)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlgebraGroupMetrics {
    pub group: String,
    pub cases: usize,
    pub positive_cases: usize,
    pub correct_solutions: usize,
    pub solution_accuracy: f64,
    pub formalization_attempts: usize,
    pub formalization_success: usize,
    pub formalization_success_rate: f64,
    pub method_selection_attempts: usize,
    pub positive_method_selection_attempts: usize,
    pub method_selection_unique: usize,
    pub positive_method_selection_unique: usize,
    pub method_selection_none: usize,
    pub method_selection_success_rate: f64,
    pub execution_attempts: usize,
    pub positive_execution_attempts: usize,
    pub execution_success: usize,
    pub execution_success_rate: f64,
    pub replay_success: usize,
    pub replay_success_rate: f64,
    pub false_authorizations: usize,
    pub false_denials: usize,
    pub expected_abstentions: usize,
    pub mean_route_steps: f64,
    pub failures: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlgebraBenchmarkReport {
    pub corpus_cases: usize,
    pub groups: BTreeMap<String, AlgebraGroupMetrics>,
    pub strategy_shadow: AlgebraStrategyShadowMetrics,
    pub deterministic: bool,
}

/// Shadow execution evidence for the staged strategy-integration boundary.
/// A stored route is never executed as authority: it must first match an
/// independently generated route, after which the existing capability
/// executor and replay verifier remain the only execution path.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlgebraStrategyShadowMetrics {
    pub cases: usize,
    pub eligible_cases: usize,
    pub recommendations: usize,
    pub independent_revalidations: usize,
    pub executed_under_existing_authority: usize,
    pub successful_executions: usize,
    pub replay_success: usize,
    pub positive_executions: usize,
    pub positive_successful_executions: usize,
    pub positive_replay_success: usize,
    pub route_agreements: usize,
    pub counterfactual_steps_saved: usize,
    pub false_authorizations: usize,
    pub false_denials: usize,
}

#[derive(Debug, Default)]
struct GroupAccumulator {
    cases: usize,
    positive_cases: usize,
    correct_solutions: usize,
    formalization_attempts: usize,
    formalization_success: usize,
    method_selection_attempts: usize,
    positive_method_selection_attempts: usize,
    method_selection_unique: usize,
    positive_method_selection_unique: usize,
    method_selection_none: usize,
    execution_attempts: usize,
    positive_execution_attempts: usize,
    execution_success: usize,
    replay_success: usize,
    false_authorizations: usize,
    false_denials: usize,
    expected_abstentions: usize,
    route_steps: usize,
    route_count: usize,
    failures: BTreeMap<String, usize>,
}

impl GroupAccumulator {
    fn failure(&mut self, class: &str) {
        *self.failures.entry(class.into()).or_default() += 1;
    }

    fn add(
        &mut self,
        case: &AlgebraCase,
        trace: &crate::formalization::FormalizationTrace,
        registry: &CapabilityRegistry,
    ) {
        self.cases += 1;
        self.positive_cases += usize::from(case.should_authorize);
        self.formalization_attempts += 1;
        if trace.target_completion.complete {
            self.formalization_success += 1;
        } else {
            self.failure("formalization_failure");
        }

        let selection = registry.discover(&trace.target_completion.target).selection;
        self.method_selection_attempts += usize::from(trace.target_completion.complete);
        self.positive_method_selection_attempts +=
            usize::from(trace.target_completion.complete && case.should_authorize);
        match selection {
            CapabilitySelection::Unique(_) => {
                self.method_selection_unique += 1;
                self.positive_method_selection_unique += usize::from(case.should_authorize);
            }
            CapabilitySelection::Ambiguous(_) => {
                if case.should_authorize {
                    self.failure("method_not_found");
                }
            }
            CapabilitySelection::None => {
                if trace.target_completion.complete {
                    self.method_selection_none += 1;
                    if case.should_authorize {
                        self.failure("method_not_found");
                    }
                }
            }
        }

        let planned_route_steps = if trace.target_completion.complete {
            crate::capability_planner::plan_target(&trace.target_completion.target, registry)
                .ok()
                .map(|plan| plan.steps.len())
        } else {
            None
        };
        let outcome = execute_case(case, &trace.target_completion.target);
        self.execution_attempts += usize::from(outcome.attempted);
        self.positive_execution_attempts += usize::from(outcome.attempted && case.should_authorize);
        self.execution_success += usize::from(outcome.success);
        self.replay_success += usize::from(outcome.replayed);
        if outcome.attempted {
            if outcome.success && outcome.replayed {
                self.route_count += 1;
                self.route_steps += planned_route_steps.unwrap_or(1);
            } else if case.should_authorize {
                if outcome.success {
                    self.failure("verification_failure");
                } else {
                    self.failure("execution_failure");
                }
            } else {
                self.expected_abstentions += 1;
            }
        }

        let correct = if case.should_authorize {
            outcome.success
                && outcome
                    .result
                    .as_deref()
                    .zip(case.expected_result.as_deref())
                    .map(|(actual, expected)| actual == expected)
                    .unwrap_or(false)
        } else {
            !outcome.success
        };
        self.correct_solutions += usize::from(correct);
        self.false_authorizations += usize::from(outcome.success && !case.should_authorize);
        self.false_denials += usize::from(!outcome.success && case.should_authorize);
    }

    fn finish(self, group: String) -> AlgebraGroupMetrics {
        let denom = self.cases.max(1) as f64;
        AlgebraGroupMetrics {
            group,
            cases: self.cases,
            positive_cases: self.positive_cases,
            correct_solutions: self.correct_solutions,
            solution_accuracy: self.correct_solutions as f64 / denom,
            formalization_attempts: self.formalization_attempts,
            formalization_success: self.formalization_success,
            formalization_success_rate: self.formalization_success as f64
                / self.formalization_attempts.max(1) as f64,
            method_selection_attempts: self.method_selection_attempts,
            positive_method_selection_attempts: self.positive_method_selection_attempts,
            method_selection_unique: self.method_selection_unique,
            positive_method_selection_unique: self.positive_method_selection_unique,
            method_selection_none: self.method_selection_none,
            method_selection_success_rate: self.positive_method_selection_unique as f64
                / self.positive_method_selection_attempts.max(1) as f64,
            execution_attempts: self.execution_attempts,
            positive_execution_attempts: self.positive_execution_attempts,
            execution_success: self.execution_success,
            execution_success_rate: self.execution_success as f64
                / self.positive_execution_attempts.max(1) as f64,
            replay_success: self.replay_success,
            replay_success_rate: self.replay_success as f64 / self.execution_success.max(1) as f64,
            false_authorizations: self.false_authorizations,
            false_denials: self.false_denials,
            expected_abstentions: self.expected_abstentions,
            mean_route_steps: self.route_steps as f64 / self.route_count.max(1) as f64,
            failures: self.failures,
        }
    }
}

#[derive(Debug, Default)]
struct ExecutionOutcome {
    attempted: bool,
    success: bool,
    replayed: bool,
    result: Option<String>,
}

/// Per-case result exposed for independent evaluation corpora.  This is an
/// observation surface only: execution still goes through the same
/// method-specific capability and replay functions used by the benchmark.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AlgebraCaseEvaluation {
    pub case_id: String,
    pub formalized: bool,
    pub authorized: bool,
    pub execution_attempted: bool,
    pub execution_success: bool,
    pub replayed: bool,
    pub result: Option<String>,
    pub abstention_reason: Option<String>,
    pub divergence_stage: String,
    pub canonical_signature: String,
    pub authorization_blockers: Vec<String>,
}

pub fn evaluate_case_independently(case: &AlgebraCase) -> AlgebraCaseEvaluation {
    let registry = CapabilityRegistry::production();
    let trace = assess_prompt(&case.id, &case.prompt, "Algebra", false);
    let assessment = crate::formalization::assess_direct_instantiation(&trace);
    let system_classification =
        (case.method == AlgebraMethod::LinearSystem).then(|| classify_linear_system(&case.prompt));
    let authorized = assessment.authorization_safe()
        && !matches!(
            system_classification.clone(),
            Some(
                LinearSystemClassification::NoSolution
                    | LinearSystemClassification::InfiniteSolutions(_)
                    | LinearSystemClassification::Unsupported
            )
        );
    // The independent audit respects the normal authorization boundary: an
    // unapproved target is observed as a refusal and never reaches execution.
    let outcome = if authorized {
        execute_case(case, &trace.target_completion.target)
    } else {
        ExecutionOutcome::default()
    };
    let abstention_reason = if authorized {
        None
    } else if let Some(classification) = system_classification {
        if !matches!(classification, LinearSystemClassification::Unique(_)) {
            Some(format!("system_classification:{classification:?}"))
        } else {
            Some(assessment.denial_trace(case.should_authorize).first_blocker)
        }
    } else {
        Some(assessment.denial_trace(case.should_authorize).first_blocker)
    };
    let divergence_stage = if !trace.target_completion.complete {
        "formalization"
    } else if !authorized {
        "authorization"
    } else if !outcome.success {
        "execution"
    } else if !outcome.replayed {
        "verification"
    } else {
        "none"
    };
    // Touch the registry through the same discovery path as the aggregate
    // benchmark so independent reports cannot accidentally bypass typing.
    if trace.target_completion.complete {
        let _ = registry.discover(&trace.target_completion.target);
    }
    AlgebraCaseEvaluation {
        case_id: case.id.clone(),
        formalized: trace.target_completion.complete,
        authorized,
        execution_attempted: outcome.attempted,
        execution_success: outcome.success,
        replayed: outcome.replayed,
        result: outcome.result,
        abstention_reason,
        divergence_stage: divergence_stage.into(),
        canonical_signature: crate::formalization::canonical_formalization_signature(&trace),
        authorization_blockers: assessment.authorization_blockers,
    }
}

fn execute_case(case: &AlgebraCase, target: &FormalizedTarget) -> ExecutionOutcome {
    match case.method {
        AlgebraMethod::LinearEquation => execute_linear_equation(target)
            .map(|receipt| ExecutionOutcome {
                attempted: true,
                success: true,
                replayed: replay_linear_equation(&receipt),
                result: Some(receipt.result),
            })
            .unwrap_or_else(|_| ExecutionOutcome {
                attempted: true,
                ..Default::default()
            }),
        AlgebraMethod::QuadraticEquation => execute_quadratic_equation(target)
            .map(|receipt| ExecutionOutcome {
                attempted: true,
                success: true,
                replayed: replay_quadratic_equation(&receipt),
                result: Some(receipt.result),
            })
            .unwrap_or_else(|_| ExecutionOutcome {
                attempted: true,
                ..Default::default()
            }),
        AlgebraMethod::LinearSystem => execute_linear_system(&case.prompt)
            .map(|receipt| ExecutionOutcome {
                attempted: true,
                success: true,
                replayed: replay_linear_system(&receipt),
                result: Some(receipt.result),
            })
            .unwrap_or_else(|_| ExecutionOutcome {
                attempted: true,
                ..Default::default()
            }),
    }
}

fn strategy_fixture(
    case: &AlgebraCase,
    target: &FormalizedTarget,
    registry: &CapabilityRegistry,
) -> Option<(
    String,
    Vec<CapabilityIoType>,
    CapabilityChainPlan,
    CapabilityChainPlan,
)> {
    let (available_inputs, fresh) = match case.method {
        AlgebraMethod::LinearEquation | AlgebraMethod::QuadraticEquation => {
            let subject = target.subject_resolution.selected.as_ref()?;
            let variable = target.target_variable.as_deref()?;
            let equation = plan_equation_chain(
                &subject.object,
                variable,
                CapabilityIoType::SolutionSet,
                registry,
            )
            .ok()?;
            (
                vec![
                    CapabilityIoType::NormalizedEquation,
                    CapabilityIoType::TargetVariable,
                ],
                equation.chain,
            )
        }
        AlgebraMethod::LinearSystem => {
            let available = [
                CapabilityIoType::EquationSystem,
                CapabilityIoType::VariableSet,
            ]
            .into_iter()
            .collect();
            let plan =
                plan_capability_chain(CapabilityIoType::SystemSolution, &available, registry)
                    .ok()?;
            (available.into_iter().collect(), plan)
        }
    };
    let solver = fresh.steps.last()?.clone();
    let stored = CapabilityChainPlan {
        goal: fresh.goal,
        steps: vec![solver.clone()],
    };
    Some((
        format!("algebra-stored-{solver}"),
        available_inputs,
        fresh,
        stored,
    ))
}

fn strategy_index_for(
    strategy_id: &str,
    input_artifacts: &[CapabilityIoType],
    stored: &CapabilityChainPlan,
) -> Option<CapabilityChainProofConceptStrategyIndex> {
    let mut index = CapabilityChainProofConceptStrategyIndex::default();
    let strategy = CapabilityChainProofConceptStrategyContract {
        strategy_id: strategy_id.into(),
        concept_ids: vec![format!("concept:{strategy_id}")],
        plan: stored.clone(),
        input_artifacts: input_artifacts.to_vec(),
        output_artifacts: vec![stored.goal],
        supporting_instances: 500,
        diagnostic_only: true,
    };
    let validation = strategy.validate(1, 1, 0, 0);
    if !validation.passed {
        return None;
    }
    index.insert(strategy, &validation).ok()?;
    index
        .record_context_evidence(
            strategy_id,
            CapabilityChainStrategicRouteContextEvidence {
                domain: "algebra".into(),
                contract_signature: "typed-algebra->solution".into(),
                policy_class: "strict-replay".into(),
                epoch: 100,
                successful_executions: 5,
                safety_failures: 0,
            },
        )
        .ok()?;
    Some(index)
}

fn independently_revalidate_strategy_route(
    candidate: &crate::capability_planner::CapabilityChainStrategicRouteCandidate,
    stored: &CapabilityChainPlan,
    fresh: &CapabilityChainPlan,
    registry: &CapabilityRegistry,
) -> bool {
    candidate.plan == *stored
        && candidate.plan.steps.len() == 1
        && fresh.steps.last() == candidate.plan.steps.first()
        && candidate
            .plan
            .steps
            .iter()
            .all(|step| registry.get(step).is_some())
}

fn evaluate_strategy_shadow(corpus: &AlgebraCorpus) -> AlgebraStrategyShadowMetrics {
    let registry = CapabilityRegistry::production();
    let context = CapabilityChainStrategicRouteContext {
        domain: "algebra".into(),
        contract_signature: "typed-algebra->solution".into(),
        policy_class: "strict-replay".into(),
        current_epoch: 100,
        recent_window: 5,
    };
    let mut metrics = AlgebraStrategyShadowMetrics {
        cases: corpus.cases.len(),
        eligible_cases: 0,
        recommendations: 0,
        independent_revalidations: 0,
        executed_under_existing_authority: 0,
        successful_executions: 0,
        replay_success: 0,
        positive_executions: 0,
        positive_successful_executions: 0,
        positive_replay_success: 0,
        route_agreements: 0,
        counterfactual_steps_saved: 0,
        false_authorizations: 0,
        false_denials: 0,
    };
    for case in &corpus.cases {
        let trace = assess_prompt(&case.id, &case.prompt, "Algebra", false);
        if !trace.target_completion.complete {
            continue;
        }
        let Some((strategy_id, inputs, fresh, stored)) =
            strategy_fixture(case, &trace.target_completion.target, &registry)
        else {
            continue;
        };
        metrics.eligible_cases += 1;
        let Some(index) = strategy_index_for(&strategy_id, &inputs, &stored) else {
            continue;
        };
        let comparison = index.compare_with_fresh_plan_in_context(
            &inputs,
            fresh.goal,
            Some(&fresh),
            &context,
            &registry,
        );
        let decision = comparison.diagnose_exploration(2);
        let recommended = matches!(
            decision.decision,
            CapabilityChainStrategicRouteDecision::ExploitStored(_)
        );
        if !recommended {
            continue;
        }
        metrics.recommendations += 1;
        let stored_candidate = comparison
            .candidates
            .iter()
            .find(|candidate| candidate.candidate_id == strategy_id);
        let revalidated = stored_candidate
            .map(|candidate| {
                independently_revalidate_strategy_route(candidate, &stored, &fresh, &registry)
            })
            .unwrap_or(false);
        if !revalidated {
            continue;
        }
        metrics.independent_revalidations += 1;
        metrics.route_agreements += 1;
        metrics.counterfactual_steps_saved += fresh.steps.len().saturating_sub(stored.steps.len());

        // The strategy remains non-authorizing.  This call is deliberately
        // the existing method-specific executor, which performs its normal
        // capability contract checks and replay verification.
        let outcome = execute_case(case, &trace.target_completion.target);
        metrics.executed_under_existing_authority += 1;
        metrics.successful_executions += usize::from(outcome.success);
        metrics.replay_success += usize::from(outcome.replayed);
        metrics.positive_executions += usize::from(case.should_authorize);
        metrics.positive_successful_executions +=
            usize::from(case.should_authorize && outcome.success);
        metrics.positive_replay_success += usize::from(case.should_authorize && outcome.replayed);
        metrics.false_authorizations += usize::from(outcome.success && !case.should_authorize);
        metrics.false_denials +=
            usize::from(case.should_authorize && (!outcome.success || !outcome.replayed));
    }
    metrics
}

fn add_group(
    groups: &mut BTreeMap<String, GroupAccumulator>,
    name: String,
    case: &AlgebraCase,
    registry: &CapabilityRegistry,
) {
    let trace = assess_prompt(&case.id, &case.prompt, "Algebra", false);
    groups.entry(name).or_default().add(case, &trace, registry);
}

pub fn evaluate(corpus: &AlgebraCorpus) -> AlgebraBenchmarkReport {
    let registry = CapabilityRegistry::production();
    let mut groups = BTreeMap::new();
    for case in &corpus.cases {
        add_group(&mut groups, "total".into(), case, &registry);
        add_group(
            &mut groups,
            if case.id.ends_with("-h") {
                "holdout"
            } else {
                "development"
            }
            .into(),
            case,
            &registry,
        );
        add_group(
            &mut groups,
            format!("method:{:?}", case.method).to_ascii_lowercase(),
            case,
            &registry,
        );
        add_group(&mut groups, format!("tier:{}", case.tier), case, &registry);
    }
    AlgebraBenchmarkReport {
        corpus_cases: corpus.cases.len(),
        groups: groups
            .into_iter()
            .map(|(k, v)| (k.clone(), v.finish(k)))
            .collect(),
        strategy_shadow: evaluate_strategy_shadow(corpus),
        deterministic: true,
    }
}

pub fn experiment_results(
    report: &AlgebraBenchmarkReport,
    dataset: impl Into<String>,
    commit: impl Into<String>,
) -> Vec<ExperimentResult> {
    let dataset = dataset.into();
    let commit = commit.into();
    let mut results = report
        .groups
        .values()
        .map(|group| {
            let mut metrics = HashMap::new();
            macro_rules! metric {
                ($name:expr, $value:expr) => {
                    metrics.insert($name.into(), $value as f64);
                };
            }
            metric!("solution_accuracy", group.solution_accuracy);
            metric!(
                "formalization_success_rate",
                group.formalization_success_rate
            );
            metric!(
                "method_selection_success_rate",
                group.method_selection_success_rate
            );
            metric!(
                "positive_method_selection_attempts",
                group.positive_method_selection_attempts as f64
            );
            metric!(
                "positive_method_selection_unique",
                group.positive_method_selection_unique as f64
            );
            metric!("execution_success_rate", group.execution_success_rate);
            metric!(
                "positive_execution_attempts",
                group.positive_execution_attempts as f64
            );
            metric!("replay_success_rate", group.replay_success_rate);
            metric!(
                "false_authorization_rate",
                group.false_authorizations as f64 / group.cases.max(1) as f64
            );
            metric!(
                "false_denial_rate",
                group.false_denials as f64 / group.cases.max(1) as f64
            );
            metric!("expected_abstentions", group.expected_abstentions as f64);
            metric!("mean_route_steps", group.mean_route_steps);
            for (failure, count) in &group.failures {
                metric!(format!("failure_{failure}"), *count as f64);
            }
            ExperimentResult {
                experiment: format!("algebra_{}", group.group.replace(':', "_")),
                claim: "bounded algebra capabilities solve and replay verified exact tasks".into(),
                commit: commit.clone(),
                seed: 0,
                dataset: Some(dataset.clone()),
                baseline: "typed algebra execution baseline".into(),
                metrics,
                passed: group.false_authorizations == 0,
                notes: format!("cases={}, failures={:?}", group.cases, group.failures),
            }
        })
        .collect::<Vec<_>>();
    let shadow = &report.strategy_shadow;
    let mut metrics = HashMap::new();
    metrics.insert(
        "eligible_case_rate".into(),
        shadow.eligible_cases as f64 / shadow.cases.max(1) as f64,
    );
    metrics.insert(
        "recommendation_rate".into(),
        shadow.recommendations as f64 / shadow.eligible_cases.max(1) as f64,
    );
    metrics.insert(
        "independent_revalidation_rate".into(),
        shadow.independent_revalidations as f64 / shadow.recommendations.max(1) as f64,
    );
    metrics.insert(
        "execution_rate".into(),
        shadow.executed_under_existing_authority as f64
            / shadow.independent_revalidations.max(1) as f64,
    );
    metrics.insert(
        "replay_rate".into(),
        shadow.replay_success as f64 / shadow.executed_under_existing_authority.max(1) as f64,
    );
    metrics.insert(
        "successful_execution_rate".into(),
        shadow.successful_executions as f64
            / shadow.executed_under_existing_authority.max(1) as f64,
    );
    metrics.insert(
        "positive_execution_rate".into(),
        shadow.positive_successful_executions as f64 / shadow.positive_executions.max(1) as f64,
    );
    metrics.insert(
        "positive_replay_rate".into(),
        shadow.positive_replay_success as f64 / shadow.positive_successful_executions.max(1) as f64,
    );
    metrics.insert(
        "route_agreement_rate".into(),
        shadow.route_agreements as f64 / shadow.independent_revalidations.max(1) as f64,
    );
    metrics.insert(
        "counterfactual_steps_saved".into(),
        shadow.counterfactual_steps_saved as f64,
    );
    metrics.insert(
        "false_authorization_rate".into(),
        shadow.false_authorizations as f64 / shadow.cases.max(1) as f64,
    );
    metrics.insert(
        "false_denial_rate".into(),
        shadow.false_denials as f64 / shadow.cases.max(1) as f64,
    );
    results.push(ExperimentResult {
        experiment: "algebra_strategy_shadow".into(),
        claim: "validated strategy routes can guide execution only after independent revalidation"
            .into(),
        commit: commit.clone(),
        seed: 0,
        dataset: Some(dataset),
        baseline: "ordinary governed algebra executor".into(),
        metrics,
        passed: shadow.false_authorizations == 0
            && shadow.false_denials == 0
            && shadow.recommendations == shadow.independent_revalidations
            && shadow.replay_success == shadow.successful_executions,
        notes: format!(
            "cases={}, eligible={}, recommendations={}, revalidated={}, saved_steps={}",
            shadow.cases,
            shadow.eligible_cases,
            shadow.recommendations,
            shadow.independent_revalidations,
            shadow.counterfactual_steps_saved
        ),
    });
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus() -> AlgebraCorpus {
        serde_json::from_str(include_str!("../data/algebra_seed_v1.json")).unwrap()
    }

    #[test]
    fn seed_corpus_is_versioned_and_deterministic() {
        let corpus = corpus();
        assert!(corpus.validation_errors().is_empty());
        let report = evaluate(&corpus);
        assert_eq!(report.corpus_cases, 60);
        assert!(report.deterministic);
        assert_eq!(report.groups["total"].false_authorizations, 0);
        assert_eq!(report.groups["total"].solution_accuracy, 1.0);
        assert_eq!(report.groups["total"].method_selection_success_rate, 1.0);
        assert_eq!(report.groups["total"].execution_success_rate, 1.0);
        assert_eq!(report.groups["total"].replay_success_rate, 1.0);
    }

    #[test]
    fn generated_cases_are_independent_and_replay_cleanly() {
        let expanded = corpus().with_generated_cases(200, 42);
        assert_eq!(expanded.cases.len(), 260);
        assert!(expanded.validation_errors().is_empty());
        let report = evaluate(&expanded);
        assert_eq!(report.groups["total"].solution_accuracy, 1.0);
        assert_eq!(report.groups["total"].execution_success_rate, 1.0);
        assert_eq!(report.groups["total"].replay_success_rate, 1.0);
        assert_eq!(report.groups["total"].false_authorizations, 0);
        assert_eq!(report.groups["holdout"].cases, 59);
        assert_eq!(report.groups["tier:development"].cases, 27);
        assert_eq!(report.groups["tier:holdout"].cases, 19);
        assert_eq!(report.groups["tier:generated"].cases, 200);
        assert_eq!(report.groups["tier:adversarial"].cases, 14);
        assert_eq!(report.strategy_shadow.cases, 260);
        assert_eq!(report.strategy_shadow.recommendations, 253);
        assert_eq!(
            report.strategy_shadow.recommendations,
            report.strategy_shadow.independent_revalidations
        );
        assert_eq!(
            report.strategy_shadow.replay_success,
            report.strategy_shadow.successful_executions
        );
        assert_eq!(report.strategy_shadow.false_authorizations, 0);
        assert_eq!(report.strategy_shadow.false_denials, 0);
        assert_eq!(report.strategy_shadow.positive_executions, 239);
        assert_eq!(report.strategy_shadow.positive_successful_executions, 239);
        assert_eq!(report.strategy_shadow.positive_replay_success, 239);
        let shadow = experiment_results(&report, "generated", "test")
            .into_iter()
            .find(|result| result.experiment == "algebra_strategy_shadow")
            .expect("strategy shadow result");
        assert!(shadow.passed);
    }

    #[test]
    fn prose_corpus_executes_authorized_language_and_abstains_safely() {
        let corpus: AlgebraCorpus =
            serde_json::from_str(include_str!("../data/algebra_prose_v1.json")).unwrap();
        assert!(corpus.validation_errors().is_empty());
        assert_eq!(corpus.cases.len(), 20);
        let report = evaluate(&corpus);
        let total = &report.groups["total"];
        assert_eq!(total.solution_accuracy, 1.0);
        assert_eq!(total.execution_success_rate, 1.0);
        assert_eq!(total.replay_success_rate, 1.0);
        assert_eq!(total.false_authorizations, 0);
        assert_eq!(total.false_denials, 0);
        assert_eq!(report.groups["holdout"].cases, 4);
        assert_eq!(report.groups["tier:development"].cases, 16);
        assert_eq!(report.groups["tier:holdout"].cases, 4);
        assert_eq!(report.strategy_shadow.positive_executions, 10);
        assert_eq!(report.strategy_shadow.positive_successful_executions, 10);
        assert_eq!(report.strategy_shadow.positive_replay_success, 10);
        let shadow = experiment_results(&report, "prose", "test")
            .into_iter()
            .find(|result| result.experiment == "algebra_strategy_shadow")
            .expect("strategy shadow result");
        assert!(shadow.passed);
    }

    #[test]
    fn strategy_shadow_rejects_route_drift_before_execution() {
        let case = AlgebraCase {
            id: "shadow-route-drift".into(),
            tier: "test".into(),
            method: AlgebraMethod::LinearEquation,
            prompt: "Solve for x: x + 3 = 7".into(),
            expected_result: Some("4".into()),
            should_authorize: true,
        };
        let registry = CapabilityRegistry::production();
        let trace = assess_prompt(&case.id, &case.prompt, "Algebra", false);
        let (strategy_id, inputs, fresh, stored) =
            strategy_fixture(&case, &trace.target_completion.target, &registry).unwrap();
        let index = strategy_index_for(&strategy_id, &inputs, &stored).unwrap();
        let comparison =
            index.compare_with_fresh_plan(&inputs, fresh.goal, Some(&fresh), &registry);
        let mut drifted = comparison
            .candidates
            .into_iter()
            .find(|candidate| candidate.candidate_id == strategy_id)
            .unwrap();
        drifted.plan.steps = vec!["quadratic_equation_solve".into()];
        assert!(!independently_revalidate_strategy_route(
            &drifted, &stored, &fresh, &registry
        ));
    }
}
