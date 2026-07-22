//! Independent out-of-distribution evaluation for the governed algebra slice.
//!
//! The corpus is intentionally separate from the development and generated
//! algebra corpora.  This module does not add capabilities or alter
//! authorization.  It measures the existing boundary on hand-authored cases
//! and paired semantic-preserving rewrites.

use crate::algebra_benchmark::{evaluate_case_independently, AlgebraCase, AlgebraCaseEvaluation};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OodVariant {
    pub id: String,
    pub base_id: String,
    pub case: AlgebraCase,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OodCorpus {
    pub schema_version: u32,
    pub cases: Vec<AlgebraCase>,
    #[serde(default)]
    pub variants: Vec<OodVariant>,
}

impl OodCorpus {
    pub fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.schema_version != 1 {
            errors.push(format!("unsupported_schema:{}", self.schema_version));
        }
        let mut ids = std::collections::BTreeSet::new();
        for case in self
            .cases
            .iter()
            .chain(self.variants.iter().map(|v| &v.case))
        {
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
        let base_ids = self
            .cases
            .iter()
            .map(|case| case.id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        for variant in &self.variants {
            if !base_ids.contains(variant.base_id.as_str()) {
                errors.push(format!("unknown_variant_base:{}", variant.id));
            }
            if variant.case.id != variant.id {
                errors.push(format!("variant_id_mismatch:{}", variant.id));
            }
        }
        errors
    }

    pub fn all_cases(&self) -> Vec<&AlgebraCase> {
        self.cases
            .iter()
            .chain(self.variants.iter().map(|variant| &variant.case))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OodMetrics {
    pub cases: usize,
    pub positives: usize,
    pub correct_decisions: usize,
    pub correct_results: usize,
    pub formalized: usize,
    pub authorized: usize,
    pub execution_successes: usize,
    pub replay_successes: usize,
    pub false_authorizations: usize,
    pub false_denials: usize,
    pub refusal_taxonomy: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InvarianceMetrics {
    pub pairs: usize,
    pub decision_stable: usize,
    pub result_stable: usize,
    pub canonical_stable: usize,
    pub rewrite_regressions: usize,
    pub base_results: Vec<AlgebraCaseEvaluation>,
    pub variant_results: Vec<AlgebraCaseEvaluation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OodReport {
    pub corpus_cases: usize,
    pub independent_cases: usize,
    pub variant_cases: usize,
    pub metrics: OodMetrics,
    pub invariance: InvarianceMetrics,
    pub deterministic: bool,
    pub divergence_stages: BTreeMap<String, usize>,
}

/// Compare benchmark results semantically. The executor preserves exact
/// rationals (for example `20/7`), while independently authored corpora may
/// record the same value as a decimal (`2.857142857...`). Structured system
/// results are therefore compared by numeric value, never by formatting.
fn results_match(actual: Option<&str>, expected: Option<&str>) -> bool {
    if actual == expected {
        return true;
    }
    let (Some(actual), Some(expected)) = (actual, expected) else {
        return false;
    };
    let (Ok(actual), Ok(expected)) = (
        serde_json::from_str::<serde_json::Value>(actual),
        serde_json::from_str::<serde_json::Value>(expected),
    ) else {
        return false;
    };
    let (Some(actual), Some(expected)) = (actual.as_object(), expected.as_object()) else {
        return false;
    };
    actual.len() == expected.len()
        && actual.iter().all(|(key, value)| {
            let Some(expected_value) = expected.get(key) else {
                return false;
            };
            let Some(actual_text) = value.as_str() else {
                return false;
            };
            let Some(expected_text) = expected_value.as_str() else {
                return false;
            };
            match (
                parse_exact_or_decimal(actual_text),
                parse_exact_or_decimal(expected_text),
            ) {
                (Some(actual), Some(expected)) => (actual - expected).abs() <= 1e-12,
                _ => actual_text == expected_text,
            }
        })
}

fn parse_exact_or_decimal(value: &str) -> Option<f64> {
    if let Ok(number) = value.parse::<f64>() {
        return Some(number);
    }
    let (numerator, denominator) = value.split_once('/')?;
    Some(numerator.parse::<f64>().ok()? / denominator.parse::<f64>().ok()?)
}

fn metric_for(cases: &[&AlgebraCase]) -> OodMetrics {
    let mut metrics = OodMetrics {
        cases: cases.len(),
        positives: 0,
        correct_decisions: 0,
        correct_results: 0,
        formalized: 0,
        authorized: 0,
        execution_successes: 0,
        replay_successes: 0,
        false_authorizations: 0,
        false_denials: 0,
        refusal_taxonomy: BTreeMap::new(),
    };
    for case in cases {
        let result = evaluate_case_independently(case);
        metrics.positives += usize::from(case.should_authorize);
        metrics.formalized += usize::from(result.formalized);
        metrics.authorized += usize::from(result.authorized);
        metrics.execution_successes += usize::from(result.execution_success);
        metrics.replay_successes += usize::from(result.replayed);
        let decision_correct = result.authorized == case.should_authorize;
        metrics.correct_decisions += usize::from(decision_correct);
        let result_correct = if case.should_authorize {
            result.execution_success
                && result.replayed
                && results_match(result.result.as_deref(), case.expected_result.as_deref())
        } else {
            !result.authorized && !result.execution_success
        };
        metrics.correct_results += usize::from(result_correct);
        metrics.false_authorizations += usize::from(result.authorized && !case.should_authorize);
        metrics.false_denials += usize::from(!result.authorized && case.should_authorize);
        if !result.authorized {
            let reason = result
                .abstention_reason
                .as_deref()
                .unwrap_or("unclassified_refusal");
            *metrics
                .refusal_taxonomy
                .entry(reason.to_string())
                .or_default() += 1;
        }
    }
    metrics
}

pub fn evaluate(corpus: &OodCorpus) -> OodReport {
    let independent = corpus.cases.iter().collect::<Vec<_>>();
    let all = corpus.all_cases();
    let mut base_results = Vec::new();
    let mut variant_results = Vec::new();
    let mut decision_stable = 0;
    let mut result_stable = 0;
    let mut canonical_stable = 0;
    let mut rewrite_regressions = 0;
    for variant in &corpus.variants {
        let base = corpus.cases.iter().find(|case| case.id == variant.base_id);
        let (Some(base), variant_case) = (base, &variant.case) else {
            continue;
        };
        let base_result = evaluate_case_independently(base);
        let variant_result = evaluate_case_independently(variant_case);
        decision_stable += usize::from(base_result.authorized == variant_result.authorized);
        let stable_result = base_result.execution_success == variant_result.execution_success
            && base_result.replayed == variant_result.replayed
            && base_result.result == variant_result.result;
        result_stable += usize::from(stable_result);
        canonical_stable +=
            usize::from(base_result.canonical_signature == variant_result.canonical_signature);
        if base_result.authorized != variant_result.authorized || !stable_result {
            rewrite_regressions += 1;
        }
        base_results.push(base_result);
        variant_results.push(variant_result);
    }
    OodReport {
        corpus_cases: all.len(),
        independent_cases: independent.len(),
        variant_cases: corpus.variants.len(),
        metrics: metric_for(&all),
        invariance: InvarianceMetrics {
            pairs: corpus.variants.len(),
            decision_stable,
            result_stable,
            canonical_stable,
            rewrite_regressions,
            base_results,
            variant_results,
        },
        deterministic: true,
        divergence_stages: all
            .iter()
            .map(|case| evaluate_case_independently(case).divergence_stage)
            .fold(BTreeMap::new(), |mut counts, stage| {
                *counts.entry(stage).or_default() += 1;
                counts
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_corpus_is_valid_and_deterministic() {
        let corpus: OodCorpus = serde_json::from_str(include_str!("../data/algebra_ood_v1.json"))
            .expect("valid OOD corpus");
        assert!(corpus.validation_errors().is_empty());
        let first = evaluate(&corpus);
        let second = evaluate(&corpus);
        assert_eq!(first, second);
        assert!(first.deterministic);
        // The corpus remains frozen and adversarial.  These assertions ensure
        // the hardening pass does not regress its safety and rewrite gates.
        assert_eq!(first.metrics.false_authorizations, 0);
        assert_eq!(first.metrics.false_denials, 0);
        assert_eq!(first.invariance.canonical_stable, 7);
        assert_eq!(first.invariance.rewrite_regressions, 0);
    }

    #[test]
    fn structured_results_accept_exact_and_decimal_encodings() {
        assert!(results_match(
            Some(r#"{"x":"20/7","y":"19/7"}"#),
            Some(r#"{"x":"2.857142857142857","y":"2.7142857142857144"}"#),
        ));
        assert!(!results_match(
            Some(r#"{"x":"20/7","y":"19/7"}"#),
            Some(r#"{"x":"4","y":"7/3"}"#),
        ));
    }
}
