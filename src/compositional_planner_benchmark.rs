//! Bounded planner-generated route selection over the governed verticals.
//!
//! The corpus supplies candidate edges, not the expected route.  The planner
//! enumerates executable candidates up to three stages, enforces typed
//! handoffs, replays every stage, and preserves ambiguity instead of turning
//! a tie into authority.

use crate::cross_vertical_benchmark::{
    execute_integer_stage, execute_system_stage, ArtifactKind, CompositionFailure,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannerStep {
    pub input: Option<ArtifactKind>,
    pub output: ArtifactKind,
    pub prompt: String,
    pub cost: u32,
    pub support: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidatePlan {
    pub id: String,
    pub steps: Vec<PlannerStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannerTask {
    pub id: String,
    pub candidates: Vec<CandidatePlan>,
    pub expected: Option<String>,
    pub should_authorize: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannerCorpus {
    pub schema_version: u32,
    pub oracle: String,
    pub cases: Vec<PlannerTask>,
}

impl PlannerCorpus {
    pub fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.schema_version != 1 {
            errors.push(format!("unsupported_schema:{}", self.schema_version));
        }
        let mut ids = std::collections::BTreeSet::new();
        for task in &self.cases {
            if !ids.insert(task.id.clone()) {
                errors.push(format!("duplicate_case:{}", task.id));
            }
            if task.candidates.is_empty() {
                errors.push(format!("no_candidates:{}", task.id));
            }
            if task.should_authorize && task.expected.is_none() {
                errors.push(format!("missing_expected:{}", task.id));
            }
            for candidate in &task.candidates {
                if candidate.steps.is_empty() || candidate.steps.len() > 3 {
                    errors.push(format!("invalid_depth:{}", task.id));
                }
            }
        }
        errors
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannerDecision {
    Preferred {
        plan_id: String,
        result: String,
        replayed_stages: usize,
    },
    Ambiguous,
    NoCandidates,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlannerMetrics {
    pub cases: usize,
    pub authorized: usize,
    pub correct_decisions: usize,
    pub false_authorizations: usize,
    pub false_denials: usize,
    pub accepted_replayed_stages: usize,
    pub ambiguous: usize,
    pub invalid_handoffs_rejected: usize,
    pub route_failures: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlannerReport {
    pub corpus_cases: usize,
    pub metrics: PlannerMetrics,
    pub failure_taxonomy: BTreeMap<String, usize>,
    pub deterministic: bool,
}

fn execute_step(
    step: &PlannerStep,
    previous_kind: Option<ArtifactKind>,
    previous: Option<&str>,
) -> Result<(ArtifactKind, String), CompositionFailure> {
    if step.input != previous_kind {
        return Err(CompositionFailure::ArtifactMismatch);
    }
    let prompt = match previous {
        Some(value) => step.prompt.replace("{intermediate}", value),
        None => step.prompt.clone(),
    };
    let result = match step.output {
        ArtifactKind::Integer => execute_integer_stage(&prompt)?,
        ArtifactKind::SolutionSet => execute_system_stage(&prompt)?,
    };
    let replay = match step.output {
        ArtifactKind::Integer => execute_integer_stage(&prompt)
            .ok()
            .is_some_and(|value| value == result),
        ArtifactKind::SolutionSet => execute_system_stage(&prompt)
            .ok()
            .is_some_and(|value| value == result),
    };
    replay
        .then_some((step.output, result))
        .ok_or(CompositionFailure::StageTwoReplayFailed)
}

fn execute_plan(candidate: &CandidatePlan) -> Result<(String, usize), CompositionFailure> {
    let mut kind = None;
    let mut artifact = None;
    for step in &candidate.steps {
        let (next_kind, next_artifact) = execute_step(step, kind, artifact.as_deref())?;
        kind = Some(next_kind);
        artifact = Some(next_artifact);
    }
    artifact
        .map(|value| (value, candidate.steps.len()))
        .ok_or(CompositionFailure::UnsupportedStage)
}

pub fn plan(task: &PlannerTask) -> PlannerDecision {
    let mut valid = Vec::new();
    for candidate in &task.candidates {
        if let Ok((result, stages)) = execute_plan(candidate) {
            let cost: u32 = candidate.steps.iter().map(|step| step.cost).sum();
            let support: u32 = candidate
                .steps
                .iter()
                .map(|step| step.support)
                .min()
                .unwrap_or(0);
            valid.push((candidate, result, stages, cost, support));
        }
    }
    if valid.is_empty() {
        return PlannerDecision::NoCandidates;
    }
    valid.sort_by(|left, right| {
        (left.3, std::cmp::Reverse(left.4), left.0.id.as_str()).cmp(&(
            right.3,
            std::cmp::Reverse(right.4),
            right.0.id.as_str(),
        ))
    });
    let best = &valid[0];
    let tied: Vec<_> = valid
        .iter()
        .filter(|entry| entry.3 == best.3 && entry.4 == best.4)
        .collect();
    if tied.iter().any(|entry| entry.1 != best.1) {
        return PlannerDecision::Ambiguous;
    }
    PlannerDecision::Preferred {
        plan_id: best.0.id.clone(),
        result: best.1.clone(),
        replayed_stages: best.2,
    }
}

pub fn evaluate(corpus: &PlannerCorpus) -> PlannerReport {
    let mut metrics = PlannerMetrics {
        cases: 0,
        authorized: 0,
        correct_decisions: 0,
        false_authorizations: 0,
        false_denials: 0,
        accepted_replayed_stages: 0,
        ambiguous: 0,
        invalid_handoffs_rejected: 0,
        route_failures: 0,
    };
    let mut failures = BTreeMap::new();
    for task in &corpus.cases {
        metrics.cases += 1;
        let decision = plan(task);
        let authorized = matches!(decision, PlannerDecision::Preferred { .. });
        metrics.authorized += usize::from(authorized);
        metrics.correct_decisions += usize::from(authorized == task.should_authorize);
        metrics.false_authorizations += usize::from(authorized && !task.should_authorize);
        metrics.false_denials += usize::from(!authorized && task.should_authorize);
        if matches!(decision, PlannerDecision::Ambiguous) {
            metrics.ambiguous += 1;
        }
        if let PlannerDecision::Preferred {
            result,
            replayed_stages,
            ..
        } = decision
        {
            metrics.accepted_replayed_stages += replayed_stages;
            if task
                .expected
                .as_ref()
                .is_some_and(|expected| expected != &result)
            {
                metrics.route_failures += 1;
            }
        } else {
            *failures
                .entry(format!("{}:{decision:?}", task.id))
                .or_default() += 1;
        }
        metrics.invalid_handoffs_rejected += task
            .candidates
            .iter()
            .filter(|candidate| {
                candidate
                    .steps
                    .windows(2)
                    .any(|pair| pair[1].input != Some(pair[0].output))
            })
            .count();
    }
    PlannerReport {
        corpus_cases: metrics.cases,
        metrics,
        failure_taxonomy: failures,
        deterministic: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn planner_rejects_invalid_handoff_and_replays_three_stages() {
        let task = PlannerTask {
            id: "three".into(),
            expected: Some("17".into()),
            should_authorize: true,
            candidates: vec![
                CandidatePlan {
                    id: "three-stage".into(),
                    steps: vec![
                        PlannerStep {
                            input: None,
                            output: ArtifactKind::Integer,
                            prompt: "Evaluate 2 + 3".into(),
                            cost: 1,
                            support: 100,
                        },
                        PlannerStep {
                            input: Some(ArtifactKind::Integer),
                            output: ArtifactKind::Integer,
                            prompt: "Evaluate {intermediate} + 4".into(),
                            cost: 1,
                            support: 100,
                        },
                        PlannerStep {
                            input: Some(ArtifactKind::Integer),
                            output: ArtifactKind::Integer,
                            prompt: "Evaluate {intermediate} + 8".into(),
                            cost: 1,
                            support: 100,
                        },
                    ],
                },
                CandidatePlan {
                    id: "invalid".into(),
                    steps: vec![
                        PlannerStep {
                            input: None,
                            output: ArtifactKind::Integer,
                            prompt: "Evaluate 2 + 3".into(),
                            cost: 1,
                            support: 100,
                        },
                        PlannerStep {
                            input: Some(ArtifactKind::SolutionSet),
                            output: ArtifactKind::Integer,
                            prompt: "Evaluate {intermediate} + 4".into(),
                            cost: 1,
                            support: 100,
                        },
                    ],
                },
            ],
        };
        assert!(matches!(
            plan(&task),
            PlannerDecision::Preferred {
                replayed_stages: 3,
                ..
            }
        ));
        assert_eq!(
            evaluate(&PlannerCorpus {
                schema_version: 1,
                oracle: "test".into(),
                cases: vec![task]
            })
            .metrics
            .invalid_handoffs_rejected,
            1
        );
    }
}
