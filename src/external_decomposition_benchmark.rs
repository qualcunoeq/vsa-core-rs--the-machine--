//! Evaluation harness for a frozen, source-separated raw-problem corpus.
//!
//! Unlike the generated OOD corpora, this format records provenance and a
//! development/holdout split.  The holdout is evaluated without being mixed
//! into development diagnostics, so future parser changes can be checked
//! against an untouched slice.

use crate::raw_decomposition_benchmark::{decompose, realize, DecompositionDecision};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorpusSplit {
    Development,
    Holdout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedOutcome {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalCase {
    pub id: String,
    pub source: String,
    pub split: CorpusSplit,
    pub prompt: String,
    pub expected_outcome: ExpectedOutcome,
    pub expected_signature: Option<String>,
    #[serde(default)]
    pub expected_result: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalCorpus {
    pub schema_version: u32,
    pub oracle: String,
    pub holdout_locked: bool,
    pub cases: Vec<ExternalCase>,
}

impl ExternalCorpus {
    pub fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.schema_version != 1 {
            errors.push(format!("unsupported_schema:{}", self.schema_version));
        }
        if self.oracle.trim().is_empty() {
            errors.push("empty_oracle".into());
        }
        if !self.holdout_locked {
            errors.push("holdout_not_locked".into());
        }
        let mut ids = BTreeSet::new();
        let mut splits = BTreeSet::new();
        for case in &self.cases {
            if !ids.insert(case.id.clone()) {
                errors.push(format!("duplicate_case:{}", case.id));
            }
            if case.source.trim().is_empty() {
                errors.push(format!("empty_source:{}", case.id));
            }
            if case.prompt.trim().is_empty() {
                errors.push(format!("empty_prompt:{}", case.id));
            }
            splits.insert(case.split);
            match case.expected_outcome {
                ExpectedOutcome::Supported if case.expected_signature.is_none() => {
                    errors.push(format!("supported_case_missing_signature:{}", case.id));
                }
                ExpectedOutcome::Ambiguous | ExpectedOutcome::Unsupported
                    if case.expected_signature.is_some() =>
                {
                    errors.push(format!("negative_case_has_signature:{}", case.id));
                }
                _ => {}
            }
        }
        if !splits.contains(&CorpusSplit::Development) {
            errors.push("missing_development_split".into());
        }
        if !splits.contains(&CorpusSplit::Holdout) {
            errors.push("missing_holdout_split".into());
        }
        errors
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExternalMetrics {
    pub cases: usize,
    pub structural_correct: usize,
    pub supported_expected: usize,
    pub realized_plans: usize,
    pub replayed_stages: usize,
    pub ambiguous_expected: usize,
    pub unsupported_expected: usize,
    pub ambiguous_preserved: usize,
    pub false_authorizations: usize,
    pub false_denials: usize,
    pub results_checked: usize,
    pub result_correct: usize,
    pub result_mismatches: usize,
}

impl ExternalMetrics {
    fn empty() -> Self {
        Self {
            cases: 0,
            structural_correct: 0,
            supported_expected: 0,
            realized_plans: 0,
            replayed_stages: 0,
            ambiguous_expected: 0,
            unsupported_expected: 0,
            ambiguous_preserved: 0,
            false_authorizations: 0,
            false_denials: 0,
            results_checked: 0,
            result_correct: 0,
            result_mismatches: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExternalReport {
    pub corpus_cases: usize,
    pub development: ExternalMetrics,
    pub holdout: ExternalMetrics,
    pub metrics: ExternalMetrics,
    pub failure_taxonomy: BTreeMap<String, usize>,
    pub failures_by_source: BTreeMap<String, usize>,
    pub deterministic: bool,
}

fn outcome(decision: &DecompositionDecision) -> ExpectedOutcome {
    match decision {
        DecompositionDecision::Sketch(_) => ExpectedOutcome::Supported,
        DecompositionDecision::Ambiguous => ExpectedOutcome::Ambiguous,
        DecompositionDecision::NoDecomposition => ExpectedOutcome::Unsupported,
    }
}

fn signature(decision: &DecompositionDecision) -> Option<String> {
    match decision {
        DecompositionDecision::Sketch(sketch) => Some(
            sketch
                .steps
                .iter()
                .map(|step| format!("{:?}>{:?}", step.input, step.output))
                .collect::<Vec<_>>()
                .join("/"),
        ),
        _ => None,
    }
}

fn merge_metrics(into: &mut ExternalMetrics, other: &ExternalMetrics) {
    into.cases += other.cases;
    into.structural_correct += other.structural_correct;
    into.supported_expected += other.supported_expected;
    into.realized_plans += other.realized_plans;
    into.replayed_stages += other.replayed_stages;
    into.ambiguous_expected += other.ambiguous_expected;
    into.unsupported_expected += other.unsupported_expected;
    into.ambiguous_preserved += other.ambiguous_preserved;
    into.false_authorizations += other.false_authorizations;
    into.false_denials += other.false_denials;
    into.results_checked += other.results_checked;
    into.result_correct += other.result_correct;
    into.result_mismatches += other.result_mismatches;
}

fn evaluate_cases(
    cases: impl IntoIterator<Item = ExternalCase>,
    failures: &mut BTreeMap<String, usize>,
    failures_by_source: &mut BTreeMap<String, usize>,
) -> ExternalMetrics {
    let mut metrics = ExternalMetrics::empty();
    for case in cases {
        metrics.cases += 1;
        match case.expected_outcome {
            ExpectedOutcome::Supported => metrics.supported_expected += 1,
            ExpectedOutcome::Ambiguous => metrics.ambiguous_expected += 1,
            ExpectedOutcome::Unsupported => metrics.unsupported_expected += 1,
        }
        let decision = decompose(&case.prompt);
        let actual_outcome = outcome(&decision);
        let structurally_correct = actual_outcome == case.expected_outcome
            && (case.expected_outcome != ExpectedOutcome::Supported
                || signature(&decision) == case.expected_signature);
        metrics.structural_correct += usize::from(structurally_correct);
        metrics.ambiguous_preserved += usize::from(
            case.expected_outcome == ExpectedOutcome::Ambiguous
                && actual_outcome == ExpectedOutcome::Ambiguous,
        );
        let realized_result = match &decision {
            DecompositionDecision::Sketch(sketch) => {
                if let Some((result, stages)) = realize(sketch) {
                    metrics.replayed_stages += stages;
                    metrics.realized_plans += 1;
                    Some(result)
                } else {
                    None
                }
            }
            _ => None,
        };
        let realized = realized_result.is_some();
        if let Some(expected_result) = &case.expected_result {
            metrics.results_checked += 1;
            if realized_result.as_ref() == Some(expected_result) {
                metrics.result_correct += 1;
            } else {
                metrics.result_mismatches += 1;
            }
        }
        if realized && case.expected_outcome != ExpectedOutcome::Supported {
            metrics.false_authorizations += 1;
        }
        if !realized && case.expected_outcome == ExpectedOutcome::Supported {
            metrics.false_denials += 1;
        }
        if !structurally_correct {
            let label = match (case.expected_outcome, actual_outcome, realized) {
                (ExpectedOutcome::Supported, _, false) => "supported_case_not_realized",
                (ExpectedOutcome::Supported, actual, true)
                    if actual != ExpectedOutcome::Supported =>
                {
                    "supported_case_wrong_outcome"
                }
                (expected, actual, _) if expected != actual => "outcome_mismatch",
                _ => "signature_mismatch",
            };
            *failures.entry(label.into()).or_default() += 1;
            *failures_by_source.entry(case.source).or_default() += 1;
        }
    }
    metrics
}

pub fn evaluate(corpus: &ExternalCorpus) -> ExternalReport {
    let mut failures = BTreeMap::new();
    let mut failures_by_source = BTreeMap::new();
    let development_cases = corpus
        .cases
        .iter()
        .filter(|case| case.split == CorpusSplit::Development)
        .cloned()
        .collect::<Vec<_>>();
    let holdout_cases = corpus
        .cases
        .iter()
        .filter(|case| case.split == CorpusSplit::Holdout)
        .cloned()
        .collect::<Vec<_>>();
    let development = evaluate_cases(development_cases, &mut failures, &mut failures_by_source);
    let holdout = evaluate_cases(holdout_cases, &mut failures, &mut failures_by_source);
    let mut metrics = ExternalMetrics::empty();
    merge_metrics(&mut metrics, &development);
    merge_metrics(&mut metrics, &holdout);
    ExternalReport {
        corpus_cases: metrics.cases,
        development,
        holdout,
        metrics,
        failure_taxonomy: failures,
        failures_by_source,
        deterministic: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locked_split_and_outcome_validation_are_enforced() {
        let corpus = ExternalCorpus {
            schema_version: 1,
            oracle: "test oracle".into(),
            holdout_locked: true,
            cases: vec![
                ExternalCase {
                    id: "dev".into(),
                    source: "manual".into(),
                    split: CorpusSplit::Development,
                    prompt: "Compute 2 + 3".into(),
                    expected_outcome: ExpectedOutcome::Supported,
                    expected_signature: Some("None>Integer".into()),
                    expected_result: None,
                },
                ExternalCase {
                    id: "holdout".into(),
                    source: "manual".into(),
                    split: CorpusSplit::Holdout,
                    prompt: "Either compute 2 + 3 directly, or use another route.".into(),
                    expected_outcome: ExpectedOutcome::Ambiguous,
                    expected_signature: None,
                    expected_result: None,
                },
            ],
        };
        assert!(corpus.validation_errors().is_empty());
        let report = evaluate(&corpus);
        assert_eq!(report.metrics.structural_correct, 2);
        assert_eq!(report.development.cases, 1);
        assert_eq!(report.holdout.cases, 1);
        assert_eq!(report.metrics.false_authorizations, 0);
        assert_eq!(report.metrics.false_denials, 0);
    }
}
