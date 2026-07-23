//! Independent prose-recurrence benchmark for the bounded affine island.

use crate::recurrence::{parse_prose_recurrence, replay_recurrence};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ProseRecurrenceCase {
    pub id: String,
    pub category: String,
    pub prompt: String,
    pub expected_route: String,
    pub expected_answer: Option<String>,
    pub should_authorize: bool,
    pub pair_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ProseRecurrenceCorpus {
    pub schema_version: u32,
    pub oracle: String,
    pub cases: Vec<ProseRecurrenceCase>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ProseRecurrenceMetrics {
    pub cases: usize,
    pub authorized: usize,
    pub correct_answers: usize,
    pub replay_verified: usize,
    pub tampered_receipts_rejected: usize,
    pub correct_decisions: usize,
    pub false_authorizations: usize,
    pub false_denials: usize,
    pub failure_taxonomy: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ProseRewriteMetrics {
    pub pairs: usize,
    pub decision_stable: usize,
    pub answer_stable: usize,
    pub regressions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProseRecurrenceReport {
    pub cases: usize,
    pub metrics: ProseRecurrenceMetrics,
    pub rewrites: ProseRewriteMetrics,
}

fn record_failure(metrics: &mut ProseRecurrenceMetrics, category: &str, reason: &str) {
    *metrics
        .failure_taxonomy
        .entry(format!("{category}:{reason}"))
        .or_default() += 1;
}

#[derive(Clone)]
struct Outcome {
    authorized: bool,
    answer: Option<String>,
}

pub fn evaluate(corpus: &ProseRecurrenceCorpus) -> ProseRecurrenceReport {
    let mut metrics = ProseRecurrenceMetrics::default();
    let mut outcomes = Vec::new();
    for case in &corpus.cases {
        metrics.cases += 1;
        let parsed = parse_prose_recurrence(&case.prompt);
        let mut authorized = false;
        let mut answer = None;
        if let Ok(request) = parsed {
            match request
                .definition
                .execute(request.target.clone(), request.contract)
            {
                Ok(execution) => {
                    authorized = true;
                    answer = Some(execution.value.format());
                    metrics.authorized += 1;
                    let answer_ok = case.expected_answer.as_deref() == answer.as_deref();
                    metrics.correct_answers += usize::from(answer_ok);
                    metrics.replay_verified += usize::from(replay_recurrence(
                        &request.definition,
                        request.target.clone(),
                        request.contract,
                        &execution.receipt,
                    ));
                    let mut tampered = execution.receipt.clone();
                    tampered.final_result.push_str(" (tampered)");
                    metrics.tampered_receipts_rejected += usize::from(!replay_recurrence(
                        &request.definition,
                        request.target,
                        request.contract,
                        &tampered,
                    ));
                    if !answer_ok {
                        record_failure(&mut metrics, &case.category, "oracle_answer_mismatch");
                    }
                }
                Err(failure) => record_failure(
                    &mut metrics,
                    &case.category,
                    &format!("execution:{failure:?}"),
                ),
            }
        } else if case.should_authorize {
            record_failure(&mut metrics, &case.category, "parse_rejected");
        }
        metrics.correct_decisions += usize::from(authorized == case.should_authorize);
        metrics.false_authorizations += usize::from(authorized && !case.should_authorize);
        metrics.false_denials += usize::from(!authorized && case.should_authorize);
        outcomes.push((case, Outcome { authorized, answer }));
    }

    let mut groups: BTreeMap<String, Vec<&Outcome>> = BTreeMap::new();
    for (case, outcome) in &outcomes {
        if let Some(pair_id) = &case.pair_id {
            groups.entry(pair_id.clone()).or_default().push(outcome);
        }
    }
    let mut rewrites = ProseRewriteMetrics {
        pairs: groups.values().filter(|group| group.len() == 2).count(),
        ..Default::default()
    };
    for group in groups.values().filter(|group| group.len() == 2) {
        let decisions = group[0].authorized == group[1].authorized;
        let answers = group[0].answer == group[1].answer;
        rewrites.decision_stable += usize::from(decisions);
        rewrites.answer_stable += usize::from(answers);
        rewrites.regressions += usize::from(!(decisions && answers));
    }
    ProseRecurrenceReport {
        cases: corpus.cases.len(),
        metrics,
        rewrites,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_corpus_is_fail_closed_and_replay_safe() {
        let corpus: ProseRecurrenceCorpus =
            serde_json::from_str(include_str!("../data/recurrence_ood_v1.json")).unwrap();
        let report = evaluate(&corpus);
        assert_eq!(report.cases, 500);
        assert_eq!(report.metrics.false_authorizations, 0);
        assert_eq!(report.metrics.false_denials, 0);
        assert_eq!(report.metrics.correct_answers, 150);
        assert_eq!(report.metrics.replay_verified, 150);
        assert_eq!(report.metrics.tampered_receipts_rejected, 150);
        assert_eq!(report.rewrites.regressions, 0);
    }
}
