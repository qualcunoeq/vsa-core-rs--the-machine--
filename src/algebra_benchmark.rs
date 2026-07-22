//! Deterministic end-to-end benchmark for the bounded algebra capabilities.
//!
//! Unlike the formalization benchmark, this harness crosses the execution
//! boundary.  It still keeps authorization, execution, and replay metrics
//! separate so a correct refusal cannot be confused with a failed solve.

use crate::capabilities::{CapabilityRegistry, CapabilitySelection};
use crate::cognition::ExperimentResult;
use crate::formalization::{assess_prompt, FormalizedTarget};
use crate::linear_equation::{execute_linear_equation, replay_linear_equation};
use crate::linear_system::{execute_linear_system, replay_linear_system};
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
    pub deterministic: bool,
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
    }
    AlgebraBenchmarkReport {
        corpus_cases: corpus.cases.len(),
        groups: groups
            .into_iter()
            .map(|(k, v)| (k.clone(), v.finish(k)))
            .collect(),
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
    report
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
        .collect()
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
        assert_eq!(report.corpus_cases, 30);
        assert!(report.deterministic);
        assert_eq!(report.groups["total"].false_authorizations, 0);
        assert_eq!(report.groups["total"].solution_accuracy, 1.0);
        assert_eq!(report.groups["total"].method_selection_success_rate, 1.0);
        assert_eq!(report.groups["total"].execution_success_rate, 1.0);
        assert_eq!(report.groups["total"].replay_success_rate, 1.0);
    }
}
