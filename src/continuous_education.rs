//! Deterministic, shadow-only continuous education planning.
//!
//! This layer turns the one-shot curriculum campaign planner into a bounded
//! sequence of learning decisions.  It consumes only replayable typed gap
//! observations and source-backed candidates.  It may select a sandbox
//! learning proposal, but it never mutates the curriculum manifest, registry,
//! or production routing.

use crate::curriculum::CurriculumManifest;
use crate::curriculum_campaign::{
    candidate_is_promotable, propose_learning_plans, GapKind, GapObservation, LearningPlan,
    SourceModuleCandidate,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// A source module plus the governance evidence required before it can be
/// selected by a continuous campaign.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EducationCandidate {
    pub source_module: SourceModuleCandidate,
    pub acquisition_cost: usize,
    pub authoritative_source_verified: bool,
    pub minimum_independent_exercises: usize,
}

/// Why a campaign round stopped or selected a module.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EducationDecision {
    Selected,
    Blocked,
    NoExactCoverage,
    Complete,
}

/// Immutable receipt for one bounded learning decision.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EducationStep {
    pub round: usize,
    pub decision: EducationDecision,
    pub module_id: Option<String>,
    pub covered_artifacts: Vec<String>,
    pub covered_case_count: usize,
    pub utility_score: i64,
    pub reason: String,
    pub plan_replay_verified: bool,
    pub replay_hash: String,
}

/// Result of a bounded self-directed campaign.  The manifest hashes must be
/// equal: education is proposed and evaluated in a sandbox only.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EducationCampaign {
    pub schema: String,
    pub initial_case_count: usize,
    pub resolved_case_count: usize,
    pub remaining_case_count: usize,
    pub rounds: Vec<EducationStep>,
    pub manifest_before: String,
    pub manifest_after: String,
    pub replay_hash: String,
}

/// Evidence returned by a sandbox source/exercise validator.  The continuous
/// planner consumes this record rather than embedding a domain-specific
/// executor or treating source text as executable fact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceValidationEvidence {
    pub module_id: String,
    pub source_document_hash: String,
    pub source_ids: Vec<String>,
    pub exercise_cases: usize,
    pub supported_cases: usize,
    pub replay_verified_cases: usize,
    pub tamper_rejected_cases: usize,
    pub provenance_preserved_cases: usize,
    pub boundary_cases: usize,
    pub boundary_refusals: usize,
    pub false_authorizations: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceValidationStatus {
    Validated,
    Rejected,
}

/// Immutable gate receipt for a source-derived module evaluated in a sandbox.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceValidationReceipt {
    pub module_id: String,
    pub status: SourceValidationStatus,
    pub source_document_hash: String,
    pub exercise_cases: usize,
    pub boundary_cases: usize,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("education campaign serializes"))
    )
}

fn step_hash(step: &EducationStep) -> String {
    digest(&(
        step.round,
        step.decision,
        &step.module_id,
        &step.covered_artifacts,
        step.covered_case_count,
        step.utility_score,
        &step.reason,
        step.plan_replay_verified,
    ))
}

fn campaign_hash(campaign: &EducationCampaign) -> String {
    digest(&(
        &campaign.schema,
        campaign.initial_case_count,
        campaign.resolved_case_count,
        campaign.remaining_case_count,
        &campaign.rounds,
        &campaign.manifest_before,
        &campaign.manifest_after,
    ))
}

impl EducationStep {
    fn new(
        round: usize,
        decision: EducationDecision,
        module_id: Option<String>,
        covered_artifacts: Vec<String>,
        covered_case_count: usize,
        utility_score: i64,
        reason: String,
        plan_replay_verified: bool,
    ) -> Self {
        let mut step = Self {
            round,
            decision,
            module_id,
            covered_artifacts,
            covered_case_count,
            utility_score,
            reason,
            plan_replay_verified,
            replay_hash: String::new(),
        };
        step.replay_hash = step_hash(&step);
        step
    }

    /// Verify that the receipt itself has not been modified.
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == step_hash(self)
    }
}

impl EducationCampaign {
    /// Verify the campaign and every contained step.
    pub fn replay_verified(&self) -> bool {
        self.rounds.iter().all(EducationStep::replay_verified)
            && self.replay_hash == campaign_hash(self)
    }

    /// The campaign is proposal-only when its manifest is unchanged.
    pub fn manifest_unchanged(&self) -> bool {
        self.manifest_before == self.manifest_after
    }
}

fn source_validation_hash(receipt: &SourceValidationReceipt) -> String {
    digest(&(
        &receipt.module_id,
        receipt.status,
        &receipt.source_document_hash,
        receipt.exercise_cases,
        receipt.boundary_cases,
        &receipt.reasons,
    ))
}

impl SourceValidationReceipt {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == source_validation_hash(self)
    }

    pub fn eligible_for_shadow_use(&self) -> bool {
        self.status == SourceValidationStatus::Validated && self.replay_verified()
    }
}

/// Apply generic source/exercise gates to evidence produced by a sandbox
/// evaluator.  This function knows nothing about the subject represented by
/// the source document.
pub fn validate_source_evidence(
    candidate: &EducationCandidate,
    evidence: &SourceValidationEvidence,
) -> SourceValidationReceipt {
    let mut reasons = Vec::new();
    if candidate.source_module.module_id != evidence.module_id {
        reasons.push("validation evidence module does not match candidate".into());
    }
    if evidence.source_document_hash.is_empty() {
        reasons.push("source document hash is missing".into());
    }
    if candidate.source_module.source_ids.is_empty()
        || !candidate
            .source_module
            .source_ids
            .iter()
            .all(|source| evidence.source_ids.contains(source))
    {
        reasons.push("candidate source provenance is incomplete or mismatched".into());
    }
    if !candidate.authoritative_source_verified {
        reasons.push("authoritative source gate is not verified".into());
    }
    if evidence.exercise_cases < candidate.minimum_independent_exercises {
        reasons.push("independent exercise count is below policy threshold".into());
    }
    if evidence.supported_cases != evidence.exercise_cases {
        reasons.push("not every independent exercise was supported".into());
    }
    if evidence.replay_verified_cases != evidence.exercise_cases {
        reasons.push("not every exercise replayed".into());
    }
    if evidence.tamper_rejected_cases != evidence.exercise_cases {
        reasons.push("not every exercise rejected tampering".into());
    }
    if evidence.provenance_preserved_cases != evidence.exercise_cases {
        reasons.push("not every exercise preserved provenance".into());
    }
    if evidence.boundary_cases == 0 || evidence.boundary_refusals != evidence.boundary_cases {
        reasons.push("boundary corpus was absent or not fully refused".into());
    }
    if evidence.false_authorizations != 0 {
        reasons.push("sandbox evidence contains false authorization".into());
    }
    let status = if reasons.is_empty() {
        SourceValidationStatus::Validated
    } else {
        SourceValidationStatus::Rejected
    };
    let mut receipt = SourceValidationReceipt {
        module_id: evidence.module_id.clone(),
        status,
        source_document_hash: evidence.source_document_hash.clone(),
        exercise_cases: evidence.exercise_cases,
        boundary_cases: evidence.boundary_cases,
        reasons,
        replay_hash: String::new(),
    };
    receipt.replay_hash = source_validation_hash(&receipt);
    receipt
}

/// Admit only candidates whose source/exercise receipt passed every gate.
/// This clones proposal data and never mutates a curriculum manifest.
pub fn admit_validated_candidates(
    candidates: &[EducationCandidate],
    receipts: &[SourceValidationReceipt],
) -> Vec<EducationCandidate> {
    candidates
        .iter()
        .filter(|candidate| {
            receipts.iter().any(|receipt| {
                receipt.module_id == candidate.source_module.module_id
                    && receipt.eligible_for_shadow_use()
            })
        })
        .cloned()
        .collect()
}

/// Score a candidate after exact gap coverage and source gates have been
/// established. Coverage dominates exercise evidence, then cost breaks ties.
/// This keeps a cheap but narrow lexical match from beating a broader typed
/// module, while retaining deterministic behavior.
fn utility_score(plan: &LearningPlan, candidate: &EducationCandidate) -> i64 {
    (plan.covered_case_count as i64) * 1_000_000
        + (candidate.source_module.independent_exercise_count as i64) * 1_000
        - candidate.acquisition_cost as i64
}

fn eligible(plan: &LearningPlan, candidate: &EducationCandidate) -> bool {
    candidate.authoritative_source_verified
        && candidate_is_promotable(plan, candidate.minimum_independent_exercises)
}

/// Run a bounded campaign.  The input observations and candidates are not
/// mutated; selected modules merely remove their exact covered artifacts from
/// the sandbox's residual gap set.
pub fn run_campaign(
    manifest: &CurriculumManifest,
    observations: &[GapObservation],
    candidates: &[EducationCandidate],
    max_rounds: usize,
) -> EducationCampaign {
    let manifest_hash = manifest.replay_hash();
    let initial_case_count = observations.len();
    let mut residuals = observations.to_vec();
    let mut rounds = Vec::new();

    for round in 0..max_rounds {
        if residuals.is_empty() {
            rounds.push(EducationStep::new(
                round,
                EducationDecision::Complete,
                None,
                Vec::new(),
                0,
                0,
                "all observed typed gaps were resolved in the sandbox".into(),
                true,
            ));
            break;
        }

        // Education can resolve missing capability/knowledge, but it must
        // not silently turn an ambiguous or explicitly unsupported request
        // into a solved case merely because an artifact name matches.
        let actionable: Vec<GapObservation> = residuals
            .iter()
            .filter(|observation| {
                matches!(
                    observation.kind,
                    GapKind::MissingCapability | GapKind::MissingKnowledge
                )
            })
            .cloned()
            .collect();
        let plans = propose_learning_plans(
            manifest,
            &actionable,
            &candidates
                .iter()
                .map(|candidate| candidate.source_module.clone())
                .collect::<Vec<_>>(),
        );
        let mut eligible_plans: Vec<(&LearningPlan, &EducationCandidate, i64)> = plans
            .iter()
            .filter_map(|plan| {
                let candidate = candidates
                    .iter()
                    .find(|candidate| candidate.source_module.module_id == plan.module_id)?;
                if eligible(plan, candidate) && plan.covered_case_count > 0 {
                    Some((plan, candidate, utility_score(plan, candidate)))
                } else {
                    None
                }
            })
            .collect();

        eligible_plans.sort_by(|left, right| {
            right
                .2
                .cmp(&left.2)
                .then_with(|| left.0.module_id.cmp(&right.0.module_id))
        });

        let Some((plan, candidate, score)) = eligible_plans.first().copied() else {
            let has_blocked_coverage = plans.iter().any(|plan| plan.covered_case_count > 0);
            rounds.push(EducationStep::new(
                round,
                if has_blocked_coverage {
                    EducationDecision::Blocked
                } else {
                    EducationDecision::NoExactCoverage
                },
                None,
                Vec::new(),
                0,
                0,
                if has_blocked_coverage {
                    "candidate coverage exists but source, exercise, or prerequisite gates block selection".into()
                } else {
                    "no candidate has exact typed overlap with the residual gaps".into()
                },
                plans.iter().all(LearningPlan::replay_verified),
            ));
            break;
        };

        let covered: BTreeSet<&str> = plan.covered_artifacts.iter().map(String::as_str).collect();
        let before = residuals.len();
        residuals.retain(|observation| {
            !matches!(
                observation.kind,
                GapKind::MissingCapability | GapKind::MissingKnowledge
            ) || !covered.contains(observation.requested_artifact.as_str())
        });
        let removed = before - residuals.len();
        rounds.push(EducationStep::new(
            round,
            EducationDecision::Selected,
            Some(candidate.source_module.module_id.clone()),
            plan.covered_artifacts.clone(),
            removed,
            score,
            format!(
                "selected exact-coverage module with {} validated exercises and cost {}",
                candidate.source_module.independent_exercise_count, candidate.acquisition_cost
            ),
            plan.replay_verified(),
        ));

        // A malformed planner should never create a non-progressing loop.
        if removed == 0 {
            break;
        }
    }

    let resolved_case_count = initial_case_count - residuals.len();
    let mut campaign = EducationCampaign {
        schema: "continuous-education-campaign-v1".into(),
        initial_case_count,
        resolved_case_count,
        remaining_case_count: residuals.len(),
        rounds,
        manifest_before: manifest_hash.clone(),
        manifest_after: manifest.replay_hash(),
        replay_hash: String::new(),
    };
    campaign.replay_hash = campaign_hash(&campaign);
    campaign
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curriculum::breadth_first_manifest;
    use crate::curriculum_campaign::{observe_gap, GapKind};

    fn candidate(id: &str, provides: &[&str], source: bool) -> EducationCandidate {
        EducationCandidate {
            source_module: SourceModuleCandidate {
                module_id: id.into(),
                title: id.into(),
                domain: "test".into(),
                provides: provides.iter().map(|value| (*value).into()).collect(),
                prerequisite_artifacts: vec!["distribution".into()],
                source_ids: if source {
                    vec!["source-1".into()]
                } else {
                    Vec::new()
                },
                independent_exercise_count: if source { 40 } else { 0 },
            },
            acquisition_cost: 1,
            authoritative_source_verified: source,
            minimum_independent_exercises: 20,
        }
    }

    #[test]
    fn campaign_selects_exact_source_backed_modules_and_replays() {
        let observations = vec![
            observe_gap("a", "mean", GapKind::MissingCapability, "missing"),
            observe_gap("b", "mean", GapKind::MissingCapability, "missing"),
            observe_gap("c", "variance", GapKind::MissingKnowledge, "missing"),
        ];
        let manifest = breadth_first_manifest();
        let campaign = run_campaign(
            &manifest,
            &observations,
            &[candidate("stats", &["mean", "variance"], true)],
            4,
        );
        assert_eq!(campaign.resolved_case_count, 3);
        assert_eq!(campaign.remaining_case_count, 0);
        assert!(campaign.replay_verified());
        assert!(campaign.manifest_unchanged());
    }

    #[test]
    fn unsupported_source_is_blocked_without_authorization() {
        let observations = vec![observe_gap(
            "a",
            "mean",
            GapKind::MissingCapability,
            "missing",
        )];
        let campaign = run_campaign(
            &breadth_first_manifest(),
            &observations,
            &[candidate("unproven", &["mean"], false)],
            2,
        );
        assert_eq!(campaign.resolved_case_count, 0);
        assert_eq!(campaign.remaining_case_count, 1);
        assert_eq!(campaign.rounds[0].decision, EducationDecision::Blocked);
        assert!(campaign.replay_verified());
    }

    #[test]
    fn ambiguous_gaps_are_not_resolved_by_artifact_overlap() {
        let observations = vec![observe_gap(
            "a",
            "mean",
            GapKind::Ambiguous,
            "target is not uniquely identified",
        )];
        let campaign = run_campaign(
            &breadth_first_manifest(),
            &observations,
            &[candidate("stats", &["mean"], true)],
            2,
        );
        assert_eq!(campaign.resolved_case_count, 0);
        assert_eq!(campaign.remaining_case_count, 1);
        assert_eq!(
            campaign.rounds[0].decision,
            EducationDecision::NoExactCoverage
        );
        assert!(campaign.replay_verified());
    }

    #[test]
    fn source_evidence_requires_all_independent_gates() {
        let candidate = candidate("stats", &["mean"], true);
        let evidence = SourceValidationEvidence {
            module_id: "stats".into(),
            source_document_hash: "document-hash".into(),
            source_ids: vec!["source-1".into()],
            exercise_cases: 40,
            supported_cases: 40,
            replay_verified_cases: 40,
            tamper_rejected_cases: 40,
            provenance_preserved_cases: 40,
            boundary_cases: 10,
            boundary_refusals: 10,
            false_authorizations: 0,
        };
        let receipt = validate_source_evidence(&candidate, &evidence);
        assert_eq!(receipt.status, SourceValidationStatus::Validated);
        assert!(receipt.eligible_for_shadow_use());

        let mut tampered = evidence;
        tampered.boundary_refusals = 9;
        let rejected = validate_source_evidence(&candidate, &tampered);
        assert_eq!(rejected.status, SourceValidationStatus::Rejected);
        assert!(!rejected.eligible_for_shadow_use());
        assert!(rejected
            .reasons
            .iter()
            .any(|reason| reason.contains("boundary")));
    }

    #[test]
    fn only_validated_receipts_admit_candidates() {
        let candidate = candidate("stats", &["mean"], true);
        let evidence = SourceValidationEvidence {
            module_id: "stats".into(),
            source_document_hash: "document-hash".into(),
            source_ids: vec!["source-1".into()],
            exercise_cases: 40,
            supported_cases: 40,
            replay_verified_cases: 40,
            tamper_rejected_cases: 40,
            provenance_preserved_cases: 40,
            boundary_cases: 10,
            boundary_refusals: 10,
            false_authorizations: 0,
        };
        let receipt = validate_source_evidence(&candidate, &evidence);
        assert_eq!(
            admit_validated_candidates(&[candidate], &[receipt]).len(),
            1
        );
    }
}
