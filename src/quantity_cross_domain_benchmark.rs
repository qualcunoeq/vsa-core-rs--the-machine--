//! Planner-level pressure tests for quantity capabilities crossing existing
//! algebra and linear-system verticals.
//!
//! The corpus supplies alternative typed routes.  The planner executes only
//! routes whose source artifact is accepted and replay-verified, ranks valid
//! routes by cost/support, and preserves ties or abstention.  This module is
//! diagnostic-only: it does not modify the global router or capability
//! registry.

use crate::fractional_quantity::{formalize as formalize_fraction, FractionalQuantityDecision};
use crate::gsm8k_quantity_candidate::formalize as formalize_gsm_quantity;
use crate::multi_step_quantity::{
    execute as execute_multi_step, formalize as formalize_multi_step, MultiStepDecision,
};
use crate::percentage_quantity::{
    bridge_to_algebra as bridge_percentage_to_algebra,
    formalize as formalize_percentage, PercentageQuantityDecision,
};
use crate::quantity_relation::{formalize as formalize_quantity, QuantityRelationDecision};
use crate::quantity_relation_integration::{bridge_ratio_to_linear_system, bridge_to_algebra};
use crate::unit_aware_quantity::{formalize as formalize_unit, UnitQuantityDecision};
use crate::unit_quantity_composition::{compose_conversion_to_linear_system, compose_to_algebra};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteKind {
    QuantityToAlgebra,
    GsmQuantityToAlgebra,
    QuantityToSystem,
    UnitToAlgebra,
    UnitToSystem,
    FractionToAlgebra,
    MultiStepToAlgebra,
    PercentageToAlgebra,
    UnsupportedHandoff,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteCandidate {
    pub id: String,
    pub kind: RouteKind,
    pub prompt: String,
    pub cost: u32,
    pub support: u32,
}

/// Standard candidate set used by diagnostic external reclassification.
/// Production routing is deliberately not wired to this helper.
pub fn standard_quantity_route_candidates(prompt: &str) -> Vec<RouteCandidate> {
    vec![
        RouteCandidate {
            id: "planner_gsm_quantity".into(),
            kind: RouteKind::GsmQuantityToAlgebra,
            prompt: prompt.into(),
            cost: 2,
            support: 80,
        },
        RouteCandidate {
            id: "unit_aware".into(),
            kind: RouteKind::UnitToAlgebra,
            prompt: prompt.into(),
            cost: 2,
            support: 90,
        },
        RouteCandidate {
            id: "quantity_relation".into(),
            kind: RouteKind::QuantityToAlgebra,
            prompt: prompt.into(),
            cost: 2,
            support: 70,
        },
        RouteCandidate {
            id: "fractional_quantity".into(),
            kind: RouteKind::FractionToAlgebra,
            prompt: prompt.into(),
            cost: 2,
            support: 65,
        },
        RouteCandidate {
            id: "multi_step_quantity".into(),
            kind: RouteKind::MultiStepToAlgebra,
            prompt: prompt.into(),
            cost: 3,
            support: 60,
        },
        RouteCandidate {
            id: "percentage_quantity".into(),
            kind: RouteKind::PercentageToAlgebra,
            prompt: prompt.into(),
            cost: 2,
            support: 85,
        },
    ]
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossDomainTask {
    pub id: String,
    pub candidates: Vec<RouteCandidate>,
    pub expected: Option<String>,
    pub should_authorize: bool,
    pub pair_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossDomainCorpus {
    pub schema_version: u32,
    pub oracle: String,
    pub cases: Vec<CrossDomainTask>,
}

impl CrossDomainCorpus {
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
        }
        errors
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannerDecision {
    Preferred { route_id: String, result: String },
    Ambiguous,
    NoCandidates,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CrossDomainMetrics {
    pub cases: usize,
    pub authorized: usize,
    pub correct_decisions: usize,
    pub false_authorizations: usize,
    pub false_denials: usize,
    pub intermediate_replays: usize,
    pub final_replays: usize,
    pub invalid_handoffs_rejected: usize,
    pub route_failures: usize,
    pub ambiguous: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RewriteMetrics {
    pub pairs: usize,
    pub decision_stable: usize,
    pub result_stable: usize,
    pub regressions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CrossDomainReport {
    pub corpus_cases: usize,
    pub metrics: CrossDomainMetrics,
    pub rewrites: RewriteMetrics,
    pub failure_taxonomy: BTreeMap<String, usize>,
    pub deterministic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RouteFailure {
    Unsupported,
    Ambiguous,
    InvalidHandoff,
    ReplayFailed,
}

#[derive(Debug, Clone)]
struct RouteOutcome {
    candidate: RouteCandidate,
    result: String,
}

/// Execute one typed route.  Every accepted front-end artifact and every
/// downstream receipt is independently replay-verified before the route is
/// eligible for ranking.
fn execute_route(candidate: &RouteCandidate) -> Result<RouteOutcome, RouteFailure> {
    let result = match candidate.kind {
        RouteKind::QuantityToAlgebra => {
            let artifact = match formalize_quantity(&candidate.prompt) {
                QuantityRelationDecision::Accepted(artifact) => artifact,
                QuantityRelationDecision::Ambiguous => return Err(RouteFailure::Ambiguous),
                QuantityRelationDecision::Unsupported => return Err(RouteFailure::Unsupported),
            };
            let receipt = bridge_to_algebra(&artifact).ok_or(RouteFailure::ReplayFailed)?;
            receipt.algebra_replay_verified.then_some(receipt.result)
        }
        RouteKind::GsmQuantityToAlgebra => {
            let artifact =
                formalize_gsm_quantity(&candidate.prompt).ok_or(RouteFailure::Unsupported)?;
            let receipt = bridge_to_algebra(&artifact).ok_or(RouteFailure::ReplayFailed)?;
            receipt.algebra_replay_verified.then_some(receipt.result)
        }
        RouteKind::QuantityToSystem => {
            let artifact = match formalize_quantity(&candidate.prompt) {
                QuantityRelationDecision::Accepted(artifact) => artifact,
                QuantityRelationDecision::Ambiguous => return Err(RouteFailure::Ambiguous),
                QuantityRelationDecision::Unsupported => return Err(RouteFailure::Unsupported),
            };
            let receipt =
                bridge_ratio_to_linear_system(&artifact).ok_or(RouteFailure::ReplayFailed)?;
            receipt.replay_verified.then_some(receipt.result)
        }
        RouteKind::UnitToAlgebra => {
            let artifact = match formalize_unit(&candidate.prompt) {
                UnitQuantityDecision::Accepted(artifact) => artifact,
                UnitQuantityDecision::Ambiguous => return Err(RouteFailure::Ambiguous),
                UnitQuantityDecision::Unsupported => return Err(RouteFailure::Unsupported),
            };
            let receipt = compose_to_algebra(&artifact).ok_or(RouteFailure::ReplayFailed)?;
            receipt
                .algebra
                .algebra_replay_verified
                .then_some(receipt.algebra.result)
        }
        RouteKind::UnitToSystem => {
            let artifact = match formalize_unit(&candidate.prompt) {
                UnitQuantityDecision::Accepted(artifact) => artifact,
                UnitQuantityDecision::Ambiguous => return Err(RouteFailure::Ambiguous),
                UnitQuantityDecision::Unsupported => return Err(RouteFailure::Unsupported),
            };
            if artifact.operation != "conversion" {
                return Err(RouteFailure::InvalidHandoff);
            }
            let receipt =
                compose_conversion_to_linear_system(&artifact).ok_or(RouteFailure::ReplayFailed)?;
            receipt.replay_verified.then_some(receipt.result)
        }
        RouteKind::FractionToAlgebra => {
            let artifact = match formalize_fraction(&candidate.prompt) {
                FractionalQuantityDecision::Accepted(artifact) => artifact,
                FractionalQuantityDecision::Ambiguous => return Err(RouteFailure::Ambiguous),
                FractionalQuantityDecision::Unsupported => return Err(RouteFailure::Unsupported),
            };
            let receipt = crate::fractional_quantity::bridge_to_algebra(&artifact)
                .ok_or(RouteFailure::ReplayFailed)?;
            receipt.algebra_replay_verified.then_some(receipt.result)
        }
        RouteKind::MultiStepToAlgebra => {
            let MultiStepDecision::Accepted(plan) = formalize_multi_step(&candidate.prompt) else {
                return Err(RouteFailure::Unsupported);
            };
            let receipt = execute_multi_step(&plan).ok_or(RouteFailure::ReplayFailed)?;
            if !receipt.replay_verified {
                return Err(RouteFailure::ReplayFailed);
            }
            Some(receipt.final_result)
        }
        RouteKind::PercentageToAlgebra => {
            let artifact = match formalize_percentage(&candidate.prompt) {
                PercentageQuantityDecision::Accepted(artifact) => artifact,
                PercentageQuantityDecision::Ambiguous => return Err(RouteFailure::Ambiguous),
                PercentageQuantityDecision::Unsupported => return Err(RouteFailure::Unsupported),
            };
            let receipt =
                bridge_percentage_to_algebra(&artifact).ok_or(RouteFailure::ReplayFailed)?;
            receipt
                .algebra_replay_verified
                .then_some(receipt.result)
        }
        RouteKind::UnsupportedHandoff => return Err(RouteFailure::InvalidHandoff),
    }
    .ok_or(RouteFailure::ReplayFailed)?;
    Ok(RouteOutcome {
        candidate: candidate.clone(),
        result,
    })
}

pub fn plan(task: &CrossDomainTask) -> PlannerDecision {
    let mut valid = Vec::new();
    for candidate in &task.candidates {
        if let Ok(outcome) = execute_route(candidate) {
            valid.push(outcome);
        }
    }
    if valid.is_empty() {
        let has_ambiguous = task
            .candidates
            .iter()
            .any(|candidate| matches!(execute_route(candidate), Err(RouteFailure::Ambiguous)));
        return if has_ambiguous {
            PlannerDecision::Ambiguous
        } else {
            PlannerDecision::NoCandidates
        };
    }
    valid.sort_by(|left, right| {
        (
            left.candidate.cost,
            std::cmp::Reverse(left.candidate.support),
            left.candidate.id.as_str(),
        )
            .cmp(&(
                right.candidate.cost,
                std::cmp::Reverse(right.candidate.support),
                right.candidate.id.as_str(),
            ))
    });
    let best = &valid[0];
    let tied = valid.iter().filter(|entry| {
        entry.candidate.cost == best.candidate.cost
            && entry.candidate.support == best.candidate.support
    });
    if tied.clone().any(|entry| entry.result != best.result) {
        return PlannerDecision::Ambiguous;
    }
    PlannerDecision::Preferred {
        route_id: best.candidate.id.clone(),
        result: best.result.clone(),
    }
}

pub fn evaluate(corpus: &CrossDomainCorpus) -> CrossDomainReport {
    let mut metrics = CrossDomainMetrics {
        cases: 0,
        authorized: 0,
        correct_decisions: 0,
        false_authorizations: 0,
        false_denials: 0,
        intermediate_replays: 0,
        final_replays: 0,
        invalid_handoffs_rejected: 0,
        route_failures: 0,
        ambiguous: 0,
    };
    let mut failures = BTreeMap::new();
    let mut outcomes: Vec<(&CrossDomainTask, PlannerDecision)> = Vec::new();
    for task in &corpus.cases {
        metrics.cases += 1;
        let decision = plan(task);
        let authorized = matches!(decision, PlannerDecision::Preferred { .. });
        metrics.authorized += usize::from(authorized);
        metrics.correct_decisions += usize::from(authorized == task.should_authorize);
        metrics.false_authorizations += usize::from(authorized && !task.should_authorize);
        metrics.false_denials += usize::from(!authorized && task.should_authorize);
        metrics.ambiguous += usize::from(matches!(decision, PlannerDecision::Ambiguous));
        if let PlannerDecision::Preferred { result, .. } = &decision {
            metrics.intermediate_replays += 1;
            metrics.final_replays += 1;
            if task
                .expected
                .as_ref()
                .is_some_and(|expected| expected != result)
            {
                metrics.route_failures += 1;
                *failures
                    .entry(format!("result_mismatch:{}", task.id))
                    .or_default() += 1;
            }
        } else if task.should_authorize {
            *failures.entry(format!("{}:no_route", task.id)).or_default() += 1;
        }
        metrics.invalid_handoffs_rejected += task
            .candidates
            .iter()
            .filter(|candidate| {
                matches!(execute_route(candidate), Err(RouteFailure::InvalidHandoff))
            })
            .count();
        outcomes.push((task, decision));
    }
    let mut groups: BTreeMap<String, Vec<&PlannerDecision>> = BTreeMap::new();
    for (task, decision) in &outcomes {
        if let Some(pair_id) = &task.pair_id {
            groups.entry(pair_id.clone()).or_default().push(decision);
        }
    }
    let mut rewrites = RewriteMetrics {
        pairs: 0,
        decision_stable: 0,
        result_stable: 0,
        regressions: 0,
    };
    for group in groups.values().filter(|group| group.len() == 2) {
        rewrites.pairs += 1;
        let decision_stable = matches!(
            (&group[0], &group[1]),
            (
                PlannerDecision::Preferred { .. },
                PlannerDecision::Preferred { .. }
            ) | (PlannerDecision::Ambiguous, PlannerDecision::Ambiguous)
                | (PlannerDecision::NoCandidates, PlannerDecision::NoCandidates)
        );
        let result_stable = match (&group[0], &group[1]) {
            (
                PlannerDecision::Preferred { result: left, .. },
                PlannerDecision::Preferred { result: right, .. },
            ) => left == right,
            _ => decision_stable,
        };
        rewrites.decision_stable += usize::from(decision_stable);
        rewrites.result_stable += usize::from(result_stable);
        rewrites.regressions += usize::from(!decision_stable || !result_stable);
    }
    CrossDomainReport {
        corpus_cases: metrics.cases,
        metrics,
        rewrites,
        failure_taxonomy: failures,
        deterministic: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planner_prefers_valid_unit_algebra_route_over_invalid_handoff() {
        let task = CrossDomainTask {
            id: "unit".into(),
            expected: Some("230".into()),
            should_authorize: true,
            pair_id: None,
            candidates: vec![
                RouteCandidate {
                    id: "invalid-system".into(),
                    kind: RouteKind::UnsupportedHandoff,
                    prompt: "".into(),
                    cost: 1,
                    support: 100,
                },
                RouteCandidate {
                    id: "unit-algebra".into(),
                    kind: RouteKind::UnitToAlgebra,
                    prompt: "Add 2 meters and 30 centimeters; express the total in centimeters."
                        .into(),
                    cost: 2,
                    support: 80,
                },
            ],
        };
        assert!(
            matches!(plan(&task), PlannerDecision::Preferred { result, .. } if result == "230")
        );
    }

    #[test]
    fn planner_rejects_unsupported_fraction_handoff() {
        let task = CrossDomainTask {
            id: "unsupported".into(),
            expected: None,
            should_authorize: false,
            pair_id: None,
            candidates: vec![RouteCandidate {
                id: "fraction".into(),
                kind: RouteKind::FractionToAlgebra,
                prompt: "What is 20% of 50?".into(),
                cost: 1,
                support: 100,
            }],
        };
        assert!(matches!(plan(&task), PlannerDecision::NoCandidates));
    }

    #[test]
    fn planner_preserves_equal_cost_different_result_tie() {
        let task = CrossDomainTask {
            id: "tie".into(),
            expected: None,
            should_authorize: false,
            pair_id: None,
            candidates: vec![
                RouteCandidate {
                    id: "left".into(),
                    kind: RouteKind::QuantityToAlgebra,
                    prompt: "5 notebooks cost 20 dollars. What is the price per notebook?".into(),
                    cost: 2,
                    support: 80,
                },
                RouteCandidate {
                    id: "right".into(),
                    kind: RouteKind::QuantityToAlgebra,
                    prompt: "There are 8 red counters plus 3 blue counters in the box. Find the total count.".into(),
                    cost: 2,
                    support: 80,
                },
            ],
        };
        assert!(matches!(plan(&task), PlannerDecision::Ambiguous));
    }
}
