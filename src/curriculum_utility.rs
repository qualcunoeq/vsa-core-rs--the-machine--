//! Utility-aware curriculum campaign planning.
//!
//! This module is intentionally usable from shadow campaign binaries without
//! mutating the core curriculum module. Utility is a ranking estimate only;
//! exact artifact overlap, source authority, prerequisites, and replay remain
//! hard gates.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use the_machine::curriculum::CurriculumManifest;
use the_machine::curriculum_campaign::{
    propose_learning_plans, GapObservation, PlanStatus, SourceModuleCandidate,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UtilityCandidate {
    pub candidate: SourceModuleCandidate,
    pub downstream_case_multiplier: usize,
    pub acquisition_cost: usize,
    pub authoritative_source: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LearningCampaignProposal {
    pub module_id: String,
    pub status: PlanStatus,
    pub covered_case_count: usize,
    pub expected_downstream_utility: usize,
    pub acquisition_cost: usize,
    pub prerequisite_packs: Vec<String>,
    pub source_ids: Vec<String>,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BudgetedPortfolio {
    pub budget: usize,
    pub selected_module_ids: Vec<String>,
    pub total_expected_utility: usize,
    pub total_acquisition_cost: usize,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn hash(proposal: &LearningCampaignProposal) -> String {
    digest(&(
        &proposal.module_id,
        &proposal.status,
        proposal.covered_case_count,
        proposal.expected_downstream_utility,
        proposal.acquisition_cost,
        &proposal.prerequisite_packs,
        &proposal.source_ids,
        &proposal.reasons,
    ))
}

impl LearningCampaignProposal {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == hash(self)
    }
}

fn portfolio_hash(portfolio: &BudgetedPortfolio) -> String {
    digest(&(
        portfolio.budget,
        &portfolio.selected_module_ids,
        portfolio.total_expected_utility,
        portfolio.total_acquisition_cost,
    ))
}

impl BudgetedPortfolio {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == portfolio_hash(self)
    }
}

fn better_at_same_cost(
    candidate: &[usize],
    incumbent: Option<&Vec<usize>>,
    proposals: &[&LearningCampaignProposal],
) -> bool {
    let Some(incumbent) = incumbent else {
        return true;
    };
    let utility = |indices: &[usize]| {
        indices
            .iter()
            .map(|index| proposals[*index].expected_downstream_utility)
            .sum::<usize>()
    };
    let candidate_utility = utility(candidate);
    let incumbent_utility = utility(incumbent);
    if candidate_utility != incumbent_utility {
        return candidate_utility > incumbent_utility;
    }
    let ids = |indices: &[usize]| {
        let mut ids = indices
            .iter()
            .map(|index| proposals[*index].module_id.clone())
            .collect::<Vec<_>>();
        ids.sort();
        ids
    };
    ids(candidate) < ids(incumbent)
}

/// Select a utility-maximizing eligible portfolio under a hard cost budget.
///
/// Only replay-valid, authoritative proposals already marked `Proposed` by
/// the base planner can enter the portfolio.  This is a shadow selection
/// result; it never mutates the curriculum or promotes a source.
pub fn select_budgeted_portfolio(
    proposals: &[LearningCampaignProposal],
    budget: usize,
) -> BudgetedPortfolio {
    let eligible = proposals
        .iter()
        .filter(|proposal| {
            proposal.status == PlanStatus::Proposed
                && proposal.acquisition_cost > 0
                && proposal.acquisition_cost <= budget
                && proposal.replay_verified()
        })
        .collect::<Vec<_>>();
    let mut states: Vec<Option<Vec<usize>>> = vec![None; budget + 1];
    states[0] = Some(Vec::new());
    for (index, proposal) in eligible.iter().enumerate() {
        let cost = proposal.acquisition_cost;
        for spent in (cost..=budget).rev() {
            let Some(previous) = states[spent - cost].clone() else {
                continue;
            };
            let mut candidate = previous;
            candidate.push(index);
            if better_at_same_cost(&candidate, states[spent].as_ref(), &eligible) {
                states[spent] = Some(candidate);
            }
        }
    }
    let mut best = Vec::new();
    let mut best_utility = 0;
    let mut best_cost = 0;
    for (cost, state) in states.into_iter().enumerate() {
        let Some(indices) = state else {
            continue;
        };
        let utility = indices
            .iter()
            .map(|index| eligible[*index].expected_downstream_utility)
            .sum::<usize>();
        let better = utility > best_utility
            || (utility == best_utility
                && (cost < best_cost
                    || (cost == best_cost && {
                        let mut candidate_ids = indices
                            .iter()
                            .map(|index| eligible[*index].module_id.clone())
                            .collect::<Vec<_>>();
                        let mut best_ids = best
                            .iter()
                            .map(|index: &usize| eligible[*index].module_id.clone())
                            .collect::<Vec<_>>();
                        candidate_ids.sort();
                        best_ids.sort();
                        candidate_ids < best_ids
                    })));
        if better {
            best = indices;
            best_utility = utility;
            best_cost = cost;
        }
    }
    let mut selected_module_ids = best
        .iter()
        .map(|index| eligible[*index].module_id.clone())
        .collect::<Vec<_>>();
    selected_module_ids.sort();
    let mut portfolio = BudgetedPortfolio {
        budget,
        selected_module_ids,
        total_expected_utility: best_utility,
        total_acquisition_cost: best_cost,
        replay_hash: String::new(),
    };
    portfolio.replay_hash = portfolio_hash(&portfolio);
    portfolio
}

pub fn propose_learning_campaigns(
    manifest: &CurriculumManifest,
    observations: &[GapObservation],
    candidates: &[UtilityCandidate],
) -> Vec<LearningCampaignProposal> {
    let base_candidates = candidates
        .iter()
        .map(|candidate| candidate.candidate.clone())
        .collect::<Vec<_>>();
    let base = propose_learning_plans(manifest, observations, &base_candidates);
    let mut proposals = Vec::new();
    for candidate in candidates {
        let plan = base
            .iter()
            .find(|plan| plan.module_id == candidate.candidate.module_id)
            .expect("utility candidate has one base plan");
        let utility = plan
            .covered_case_count
            .saturating_mul(candidate.downstream_case_multiplier);
        let mut status = plan.status.clone();
        let mut reasons = plan.reasons.clone();
        if candidate.acquisition_cost == 0 {
            status = PlanStatus::Blocked;
            reasons.push("acquisition cost must be positive".into());
        }
        if !candidate.authoritative_source {
            status = PlanStatus::Blocked;
            reasons.push("source authority is not established".into());
        }
        if utility == 0 {
            status = PlanStatus::Blocked;
            reasons.push("no expected downstream utility from exact overlap".into());
        } else {
            reasons.push(format!(
                "expected downstream utility {utility} at cost {}",
                candidate.acquisition_cost
            ));
        }
        let mut proposal = LearningCampaignProposal {
            module_id: plan.module_id.clone(),
            status,
            covered_case_count: plan.covered_case_count,
            expected_downstream_utility: utility,
            acquisition_cost: candidate.acquisition_cost,
            prerequisite_packs: plan.prerequisite_packs.clone(),
            source_ids: plan.source_ids.clone(),
            reasons,
            replay_hash: String::new(),
        };
        proposal.replay_hash = hash(&proposal);
        proposals.push(proposal);
    }
    proposals.sort_by(|left, right| {
        let left_eligible = left.status == PlanStatus::Proposed && left.acquisition_cost > 0;
        let right_eligible = right.status == PlanStatus::Proposed && right.acquisition_cost > 0;
        right_eligible
            .cmp(&left_eligible)
            .then_with(|| {
                right
                    .expected_downstream_utility
                    .saturating_mul(left.acquisition_cost)
                    .cmp(
                        &left
                            .expected_downstream_utility
                            .saturating_mul(right.acquisition_cost),
                    )
            })
            .then_with(|| {
                right
                    .expected_downstream_utility
                    .cmp(&left.expected_downstream_utility)
            })
            .then_with(|| left.module_id.cmp(&right.module_id))
    });
    proposals
}
