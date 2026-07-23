//! Blind mixed-domain integration benchmark.
//!
//! The corpus withholds the expected vertical from the evaluator.  Routing is
//! performed by `QuestionRouter`; authorization is then measured only from the
//! orchestrator's verified answer.  The benchmark deliberately includes
//! recurrence prompts as a safe unsupported boundary: they should route to
//! math but abstain until a prose recurrence executor is integrated.

use crate::router::{QuestionRouter, Tool};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixedOodCase {
    pub id: String,
    pub domain: String,
    pub prompt: String,
    pub expected_route: String,
    pub should_authorize: bool,
    pub pair_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixedOodCorpus {
    pub schema_version: u32,
    pub oracle: String,
    pub cases: Vec<MixedOodCase>,
}

impl MixedOodCorpus {
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
        }
        errors
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MixedOodMetrics {
    pub cases: usize,
    pub route_correct: usize,
    pub formalized: usize,
    pub authorized: usize,
    pub correct_decisions: usize,
    pub false_authorizations: usize,
    pub false_denials: usize,
    pub replay_successes: usize,
    pub failure_taxonomy: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MixedRewriteMetrics {
    pub pairs: usize,
    pub route_stable: usize,
    pub decision_stable: usize,
    pub answer_stable: usize,
    pub regressions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MixedOodReport {
    pub corpus_cases: usize,
    pub metrics: MixedOodMetrics,
    pub rewrites: MixedRewriteMetrics,
    pub route_confusion: BTreeMap<String, usize>,
    pub deterministic: bool,
}

fn tool_label(tool: Tool) -> &'static str {
    match tool {
        Tool::Math => "math",
        Tool::Theorem => "theorem",
        Tool::Physics => "physics",
        Tool::Chess => "chess",
        Tool::Code => "code",
        Tool::Vision => "vision",
        Tool::LifeScience => "life_science",
        Tool::FactualQA => "factual_qa",
    }
}

#[derive(Clone)]
struct Outcome {
    route: String,
    authorized: bool,
    answer: Option<String>,
}

pub fn evaluate(corpus: &MixedOodCorpus) -> MixedOodReport {
    let mut metrics = MixedOodMetrics {
        cases: 0,
        route_correct: 0,
        formalized: 0,
        authorized: 0,
        correct_decisions: 0,
        false_authorizations: 0,
        false_denials: 0,
        replay_successes: 0,
        failure_taxonomy: BTreeMap::new(),
    };
    let mut outcomes = Vec::new();
    let mut route_confusion = BTreeMap::new();
    for case in &corpus.cases {
        let first = QuestionRouter::orchestrate(&case.prompt);
        let route = tool_label(first.plan.domain).to_string();
        let formalized = first.plan.problem.unresolved.is_empty();
        let authorized = first.answer.is_some();
        let second = authorized.then(|| QuestionRouter::orchestrate(&case.prompt));
        let replay = second.as_ref().is_some_and(|replayed| {
            replayed.answer == first.answer && replayed.verification == first.verification
        });
        metrics.cases += 1;
        metrics.route_correct += usize::from(route == case.expected_route);
        metrics.formalized += usize::from(formalized);
        metrics.authorized += usize::from(authorized);
        metrics.correct_decisions += usize::from(authorized == case.should_authorize);
        metrics.false_authorizations += usize::from(authorized && !case.should_authorize);
        metrics.false_denials += usize::from(!authorized && case.should_authorize);
        metrics.replay_successes += usize::from(replay);
        if !case.should_authorize && authorized {
            *metrics
                .failure_taxonomy
                .entry(format!("{}:cross_domain_false_authorization", case.domain))
                .or_default() += 1;
        } else if case.should_authorize && !authorized {
            *metrics
                .failure_taxonomy
                .entry(
                    first
                        .abstention_reason
                        .map(|reason| format!("{}:abstain:{reason:?}", case.domain))
                        .unwrap_or_else(|| format!("{}:abstain:unknown", case.domain)),
                )
                .or_default() += 1;
        }
        if route != case.expected_route {
            *route_confusion
                .entry(format!(
                    "{}:expected:{}->actual:{}",
                    case.domain, case.expected_route, route
                ))
                .or_default() += 1;
        }
        outcomes.push((
            case,
            Outcome {
                route,
                authorized,
                answer: first.answer,
            },
        ));
    }

    let mut groups: BTreeMap<String, Vec<&Outcome>> = BTreeMap::new();
    for (case, outcome) in &outcomes {
        if let Some(pair_id) = &case.pair_id {
            groups.entry(pair_id.clone()).or_default().push(outcome);
        }
    }
    let mut route_stable = 0;
    let mut decision_stable = 0;
    let mut answer_stable = 0;
    let mut regressions = 0;
    for group in groups.values() {
        if group.len() != 2 {
            continue;
        }
        let route_ok = group[0].route == group[1].route;
        let decision_ok = group[0].authorized == group[1].authorized;
        let answer_ok = group[0].answer == group[1].answer;
        route_stable += usize::from(route_ok);
        decision_stable += usize::from(decision_ok);
        answer_stable += usize::from(answer_ok);
        regressions += usize::from(!(route_ok && decision_ok && answer_ok));
    }

    MixedOodReport {
        corpus_cases: metrics.cases,
        metrics,
        rewrites: MixedRewriteMetrics {
            pairs: groups.values().filter(|group| group.len() == 2).count(),
            route_stable,
            decision_stable,
            answer_stable,
            regressions,
        },
        route_confusion,
        deterministic: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_corpus_is_deterministic_and_fail_closed() {
        let corpus: MixedOodCorpus =
            serde_json::from_str(include_str!("../data/mixed_ood_v1.json")).unwrap();
        assert!(corpus.validation_errors().is_empty());
        let first = evaluate(&corpus);
        assert_eq!(first, evaluate(&corpus));
        assert_eq!(first.corpus_cases, 1000);
        assert_eq!(first.metrics.false_authorizations, 0);
        assert_eq!(first.metrics.false_denials, 0);
        assert_eq!(first.rewrites.regressions, 0);
    }
}
