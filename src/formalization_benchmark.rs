//! Structured benchmark runner for the constrained formalization pipeline.
//!
//! This measures typed extraction and authorization decisions separately from
//! execution. It uses the versioned gold corpus and emits standard
//! `ExperimentResult` records for development, holdout, and curriculum tiers.

use crate::cognition::ExperimentResult;
use crate::failure_taxonomy::FailureTaxonomyReport;
use crate::formalization::{
    assess_direct_instantiation, assess_prompt, score_formalization, FormalizationCorpus,
    FormalizationGoldCase, FormalizationScore,
};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FormalizationGroupMetrics {
    pub group: String,
    pub cases: usize,
    pub structural_target_accuracy: f64,
    pub target_complete_rate: f64,
    pub authorization_accuracy: f64,
    pub false_authorizations: usize,
    pub false_denials: usize,
    pub failure_taxonomy: FailureTaxonomyReport,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FormalizationBenchmarkReport {
    pub corpus_cases: usize,
    pub groups: BTreeMap<String, FormalizationGroupMetrics>,
    pub deterministic: bool,
}

#[derive(Debug, Default)]
struct GroupAccumulator {
    cases: usize,
    structural_targets: usize,
    complete_targets: usize,
    authorization_correct: usize,
    false_authorizations: usize,
    false_denials: usize,
    taxonomy: FailureTaxonomyReport,
}

impl GroupAccumulator {
    fn add(
        &mut self,
        case: &FormalizationGoldCase,
        score: &FormalizationScore,
        authorized: bool,
        trace: &crate::formalization::FormalizationTrace,
        denial: &crate::formalization::AuthorizationDenialTrace,
    ) {
        self.cases += 1;
        self.structural_targets += usize::from(score.target_structural);
        self.complete_targets += usize::from(trace.target_completion.complete);
        self.authorization_correct += usize::from(score.authorization_correct);
        self.false_authorizations += usize::from(authorized && !case.authorization_expected);
        self.false_denials += usize::from(!authorized && case.authorization_expected);
        self.taxonomy.observe(trace, denial, authorized);
    }

    fn finish(mut self, group: String) -> FormalizationGroupMetrics {
        self.taxonomy.finalize();
        let denominator = self.cases.max(1) as f64;
        FormalizationGroupMetrics {
            group,
            cases: self.cases,
            structural_target_accuracy: self.structural_targets as f64 / denominator,
            target_complete_rate: self.complete_targets as f64 / denominator,
            authorization_accuracy: self.authorization_correct as f64 / denominator,
            false_authorizations: self.false_authorizations,
            false_denials: self.false_denials,
            failure_taxonomy: self.taxonomy,
        }
    }
}

fn holdout(id: &str) -> bool {
    id.rsplit('-')
        .next()
        .and_then(|suffix| suffix.parse::<u32>().ok())
        .map(|number| number >= 15)
        .unwrap_or(false)
}

fn evaluate_case(
    case: &FormalizationGoldCase,
) -> (
    crate::formalization::FormalizationTrace,
    FormalizationScore,
    bool,
    crate::formalization::AuthorizationDenialTrace,
) {
    let trace = assess_prompt(&case.id, &case.prompt, "Math", false);
    let assessment = assess_direct_instantiation(&trace);
    let authorized = assessment.authorization_safe();
    let score = score_formalization(case, &trace, authorized);
    let denial = assessment.denial_trace(case.authorization_expected);
    (trace, score, authorized, denial)
}

pub fn evaluate(corpus: &FormalizationCorpus) -> FormalizationBenchmarkReport {
    let mut groups = BTreeMap::<String, GroupAccumulator>::new();
    for case in &corpus.cases {
        let (trace, score, authorized, denial) = evaluate_case(case);
        groups
            .entry("total".into())
            .or_default()
            .add(case, &score, authorized, &trace, &denial);
        groups
            .entry(if holdout(&case.id) {
                "holdout".into()
            } else {
                "development".into()
            })
            .or_default()
            .add(case, &score, authorized, &trace, &denial);
        groups
            .entry(format!("tier:{}", case.tier.label()))
            .or_default()
            .add(case, &score, authorized, &trace, &denial);
    }
    let groups = groups
        .into_iter()
        .map(|(name, accumulator)| (name.clone(), accumulator.finish(name)))
        .collect();
    FormalizationBenchmarkReport {
        corpus_cases: corpus.cases.len(),
        groups,
        deterministic: true,
    }
}

pub fn experiment_results(
    report: &FormalizationBenchmarkReport,
    corpus_path: impl Into<String>,
    commit: impl Into<String>,
) -> Vec<ExperimentResult> {
    let dataset = corpus_path.into();
    let commit = commit.into();
    report
        .groups
        .values()
        .map(|group| {
            let mut metrics = HashMap::new();
            metrics.insert(
                "structural_target_accuracy".into(),
                group.structural_target_accuracy,
            );
            metrics.insert("target_complete_rate".into(), group.target_complete_rate);
            metrics.insert("authorization_accuracy".into(), group.authorization_accuracy);
            metrics.insert(
                "false_authorization_rate".into(),
                group.false_authorizations as f64 / group.cases.max(1) as f64,
            );
            metrics.insert(
                "false_denial_rate".into(),
                group.false_denials as f64 / group.cases.max(1) as f64,
            );
            metrics.insert(
                "failure_classification_coverage".into(),
                group.failure_taxonomy.classification_coverage,
            );
            for (class, count) in &group.failure_taxonomy.counts {
                metrics.insert(format!("failure_{class}"), *count as f64);
            }
            ExperimentResult {
                experiment: format!("formalization_{}", group.group.replace(':', "_")),
                claim: "constrained formalization preserves typed structure and explains abstentions".into(),
                commit: commit.clone(),
                seed: 0,
                dataset: Some(dataset.clone()),
                baseline: "report-only formalization baseline".into(),
                metrics,
                passed: group.false_authorizations == 0
                    && group.failure_taxonomy.classification_coverage >= 0.95,
                notes: format!("cases={}, taxonomy={:?}", group.cases, group.failure_taxonomy.counts),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formalization::FormalizationCorpus;

    #[test]
    fn seed_corpus_has_stable_holdout_and_taxonomy_metrics() {
        let corpus: FormalizationCorpus = serde_json::from_str(include_str!(
            "../data/formalization_seed_v1.json"
        ))
        .unwrap();
        let report = evaluate(&corpus);
        assert_eq!(report.corpus_cases, 60);
        assert!(report.deterministic);
        assert_eq!(report.groups["holdout"].cases, 18);
        assert!(report.groups["total"].failure_taxonomy.classification_coverage >= 0.95);
        assert_eq!(report.groups["total"].false_authorizations, 0);
    }

    #[test]
    fn results_use_standard_experiment_schema() {
        let corpus: FormalizationCorpus = serde_json::from_str(include_str!(
            "../data/formalization_seed_v1.json"
        ))
        .unwrap();
        let report = evaluate(&corpus);
        let results = experiment_results(&report, "data/formalization_seed_v1.json", "test");
        assert_eq!(results.len(), 6);
        assert!(results.iter().all(|result| result.metric("failure_classification_coverage").is_some()));
    }
}
