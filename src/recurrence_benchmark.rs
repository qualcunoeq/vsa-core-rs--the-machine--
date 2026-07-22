//! Deterministic bounded benchmark for the typed recurrence execution island.
//!
//! The benchmark deliberately exercises the existing contract boundary rather
//! than adding a recurrence method.  Positive cases are independently
//! generated first-order affine recurrences; negative cases probe missing or
//! conflicting provenance, domain/budget violations, and checked arithmetic.
//! Authorization, execution, replay, and refusal reasons remain separate.

use crate::algebra_island::ExactNumber;
use crate::cognition::ExperimentResult;
use crate::recurrence::{
    DefinitionProvenance, IndexDomain, InitialCondition, RecurrenceContract, RecurrenceDefinition,
    RecurrenceFailure, RecurrenceRelation, RecurrenceTarget,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecurrenceCaseKind {
    Valid,
    MissingInitialCondition,
    ConflictingInitialCondition,
    UnrollLimit,
    TargetOutsideDomain,
    TargetBeforeBase,
    ArithmeticOverflow,
}

#[derive(Debug, Clone)]
struct RecurrenceCase {
    id: String,
    definition: RecurrenceDefinition,
    target: RecurrenceTarget,
    contract: RecurrenceContract,
    should_authorize: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct RecurrenceMetrics {
    pub cases: usize,
    pub expected_authorized: usize,
    pub authorized: usize,
    pub executions: usize,
    pub replay_verified: usize,
    pub false_authorizations: usize,
    pub false_denials: usize,
    pub failure_taxonomy: BTreeMap<String, usize>,
}

impl RecurrenceMetrics {
    fn record_failure(&mut self, failure: &RecurrenceFailure) {
        *self
            .failure_taxonomy
            .entry(failure_label(failure).into())
            .or_default() += 1;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecurrenceBenchmarkReport {
    pub seed: u64,
    pub generated_cases: usize,
    pub total: RecurrenceMetrics,
    pub development: RecurrenceMetrics,
    pub holdout: RecurrenceMetrics,
    pub deterministic: bool,
}

fn n(value: i128) -> ExactNumber {
    ExactNumber::Integer(value)
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = value;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

fn base_definition(coefficient: i128, offset: i128, initial: i128) -> RecurrenceDefinition {
    RecurrenceDefinition {
        sequence: "a".into(),
        index_variable: "n".into(),
        index_domain: IndexDomain::NonNegative,
        relation: RecurrenceRelation::ExplicitAffine {
            coefficient: n(coefficient),
            offset: n(offset),
        },
        initial_conditions: vec![InitialCondition {
            index: 0,
            value: n(initial),
            source_fragment: "a_0 = generated".into(),
        }],
        quantification: "for all n >= 0".into(),
        provenance: DefinitionProvenance::PromptSupplied {
            fragments: vec!["generated affine recurrence".into()],
            normalized_hash: "benchmark-generated".into(),
        },
    }
}

fn case_at(index: usize, seed: u64) -> RecurrenceCase {
    let entropy = splitmix64(seed.wrapping_add(index as u64));
    let coefficient = (entropy % 6) as i128 - 2; // -2..3
    let offset = ((entropy >> 8) % 11) as i128 - 5;
    let initial = ((entropy >> 16) % 11) as i128 - 5;
    let target_index = ((entropy >> 24) % 13) as i64;
    // The stride makes the every-fifth holdout slice cover all twelve
    // scenarios, rather than accidentally containing only positive cases.
    let scenario = (index + 2 * (index / 5)) % 12;
    let kind = match scenario {
        0..=5 => RecurrenceCaseKind::Valid,
        6 => RecurrenceCaseKind::MissingInitialCondition,
        7 => RecurrenceCaseKind::ConflictingInitialCondition,
        8 => RecurrenceCaseKind::UnrollLimit,
        9 => RecurrenceCaseKind::TargetOutsideDomain,
        10 => RecurrenceCaseKind::TargetBeforeBase,
        _ => RecurrenceCaseKind::ArithmeticOverflow,
    };
    let mut definition = base_definition(coefficient, offset, initial);
    let (target, contract, should_authorize) = match kind {
        RecurrenceCaseKind::Valid => (
            RecurrenceTarget::ValueAt {
                index: target_index,
            },
            RecurrenceContract::default(),
            true,
        ),
        RecurrenceCaseKind::MissingInitialCondition => {
            definition.initial_conditions.clear();
            (
                RecurrenceTarget::ValueAt { index: 2 },
                RecurrenceContract::default(),
                false,
            )
        }
        RecurrenceCaseKind::ConflictingInitialCondition => {
            definition.initial_conditions.push(InitialCondition {
                index: 0,
                value: n(initial + 1),
                source_fragment: "a_0 = conflicting".into(),
            });
            (
                RecurrenceTarget::ValueAt { index: 2 },
                RecurrenceContract::default(),
                false,
            )
        }
        RecurrenceCaseKind::UnrollLimit => (
            RecurrenceTarget::ValueAt { index: 65 },
            RecurrenceContract {
                max_unroll_steps: 8,
                ..Default::default()
            },
            false,
        ),
        RecurrenceCaseKind::TargetOutsideDomain => (
            RecurrenceTarget::ValueAt { index: -1 },
            RecurrenceContract::default(),
            false,
        ),
        RecurrenceCaseKind::TargetBeforeBase => {
            definition.initial_conditions[0].index = 3;
            definition.index_domain = IndexDomain::Range { start: 0, end: 100 };
            (
                RecurrenceTarget::ValueAt { index: 1 },
                RecurrenceContract::default(),
                false,
            )
        }
        RecurrenceCaseKind::ArithmeticOverflow => {
            definition.relation = RecurrenceRelation::ExplicitAffine {
                coefficient: n(i128::MAX),
                offset: n(0),
            };
            definition.initial_conditions[0].value = n(2);
            (
                RecurrenceTarget::ValueAt { index: 1 },
                RecurrenceContract::default(),
                false,
            )
        }
    };
    RecurrenceCase {
        id: format!("rec-{index:05}{}", if index % 5 == 0 { "-h" } else { "" }),
        definition,
        target,
        contract,
        should_authorize,
    }
}

fn generated_cases(count: usize, seed: u64) -> Vec<RecurrenceCase> {
    (0..count).map(|index| case_at(index, seed)).collect()
}

fn failure_label(failure: &RecurrenceFailure) -> &'static str {
    match failure {
        RecurrenceFailure::InitialConditionMissing => "initial_condition_missing",
        RecurrenceFailure::ConflictingDefinitions => "conflicting_definitions",
        RecurrenceFailure::UnrollLimitExceeded => "unroll_limit_exceeded",
        RecurrenceFailure::TargetOutsideDomain => "target_outside_domain",
        RecurrenceFailure::TargetBeforeBase => "target_before_base",
        RecurrenceFailure::ArithmeticFailure(_) => "arithmetic_failure",
        RecurrenceFailure::InitialConditionsInsufficient => "initial_conditions_insufficient",
        RecurrenceFailure::DefinitionNotIdentified => "definition_not_identified",
        RecurrenceFailure::EmptySequence => "empty_sequence",
        RecurrenceFailure::IndexVariableUnbound => "index_variable_unbound",
        RecurrenceFailure::QuantifierMissing => "quantifier_missing",
        RecurrenceFailure::UnsupportedOrder => "unsupported_order",
        RecurrenceFailure::UnsupportedImplicitRecurrence => "unsupported_implicit_recurrence",
        RecurrenceFailure::UnsupportedTarget => "unsupported_target",
        RecurrenceFailure::ReplayVerificationFailed => "replay_verification_failed",
    }
}

fn evaluate_slice(cases: &[RecurrenceCase]) -> RecurrenceMetrics {
    let mut metrics = RecurrenceMetrics::default();
    for case in cases {
        metrics.cases += 1;
        metrics.expected_authorized += usize::from(case.should_authorize);
        match case.definition.execute(case.target.clone(), case.contract) {
            Ok(answer) => {
                metrics.authorized += 1;
                metrics.executions += 1;
                let replayed = answer.receipt.steps.iter().all(|step| step.replay_verified);
                metrics.replay_verified += usize::from(replayed);
                metrics.false_authorizations += usize::from(!case.should_authorize);
                metrics.false_denials += usize::from(case.should_authorize && !replayed);
            }
            Err(failure) => {
                metrics.record_failure(&failure);
                metrics.false_denials += usize::from(case.should_authorize);
            }
        }
    }
    metrics
}

pub fn evaluate(count: usize, seed: u64) -> RecurrenceBenchmarkReport {
    let cases = generated_cases(count, seed);
    let development_cases: Vec<_> = cases
        .iter()
        .filter(|case| !case.id.ends_with("-h"))
        .cloned()
        .collect();
    let holdout_cases: Vec<_> = cases
        .iter()
        .filter(|case| case.id.ends_with("-h"))
        .cloned()
        .collect();
    RecurrenceBenchmarkReport {
        seed,
        generated_cases: count,
        total: evaluate_slice(&cases),
        development: evaluate_slice(&development_cases),
        holdout: evaluate_slice(&holdout_cases),
        deterministic: true,
    }
}

fn result_for(
    name: &str,
    metrics: &RecurrenceMetrics,
    seed: u64,
    commit: &str,
) -> ExperimentResult {
    let mut values = BTreeMap::new();
    values.insert("cases".into(), metrics.cases as f64);
    values.insert(
        "expected_authorized".into(),
        metrics.expected_authorized as f64,
    );
    values.insert("authorized".into(), metrics.authorized as f64);
    values.insert(
        "execution_rate".into(),
        metrics.authorized as f64 / metrics.cases.max(1) as f64,
    );
    values.insert(
        "replay_rate".into(),
        metrics.replay_verified as f64 / metrics.executions.max(1) as f64,
    );
    values.insert(
        "false_authorization_rate".into(),
        metrics.false_authorizations as f64 / metrics.cases.max(1) as f64,
    );
    values.insert(
        "false_denial_rate".into(),
        metrics.false_denials as f64 / metrics.cases.max(1) as f64,
    );
    for (label, count) in &metrics.failure_taxonomy {
        values.insert(format!("failure_{label}"), *count as f64);
    }
    ExperimentResult {
        experiment: format!("recurrence_{name}"),
        claim: "bounded affine recurrence execution authorizes only typed exact tasks and replays every accepted step".into(),
        commit: commit.into(),
        seed,
        dataset: Some(name.into()),
        baseline: "typed bounded recurrence executor".into(),
        metrics: values.into_iter().collect(),
        passed: metrics.false_authorizations == 0
            && metrics.false_denials == 0
            && metrics.replay_verified == metrics.executions,
        notes: format!("failure_taxonomy={:?}", metrics.failure_taxonomy),
    }
}

pub fn experiment_results(
    report: &RecurrenceBenchmarkReport,
    commit: impl Into<String>,
) -> Vec<ExperimentResult> {
    let commit = commit.into();
    vec![
        result_for("total", &report.total, report.seed, &commit),
        result_for("development", &report.development, report.seed, &commit),
        result_for("holdout", &report.holdout, report.seed, &commit),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_benchmark_is_deterministic_and_fail_closed() {
        let first = evaluate(500, 42);
        let second = evaluate(500, 42);
        assert_eq!(first, second);
        assert_eq!(first.total.cases, 500);
        assert_eq!(first.total.expected_authorized, 251);
        assert_eq!(first.total.false_authorizations, 0);
        assert_eq!(first.total.false_denials, 0);
        assert_eq!(first.total.replay_verified, first.total.executions);
        assert_eq!(first.holdout.cases, 100);
        assert!(first
            .total
            .failure_taxonomy
            .contains_key("initial_condition_missing"));
        assert!(first
            .total
            .failure_taxonomy
            .contains_key("arithmetic_failure"));
        assert!(first
            .holdout
            .failure_taxonomy
            .contains_key("initial_condition_missing"));
        assert!(first
            .holdout
            .failure_taxonomy
            .contains_key("target_before_base"));
    }

    #[test]
    fn experiment_results_preserve_split_metrics() {
        let report = evaluate(50, 7);
        let results = experiment_results(&report, "test");
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|result| result.passed));
        assert_eq!(results[0].metric("false_authorization_rate"), Some(0.0));
    }
}
