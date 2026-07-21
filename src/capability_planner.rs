//! Deterministic planning over the verified capability dependency graph.
//!
//! This planner is intentionally not a theorem planner and does not infer
//! missing modeling steps.  It only expands the unique capability selected
//! for an already-grounded target and returns its dependency-first closure.

use crate::capabilities::{CapabilityIoType, CapabilityRegistry, CapabilitySelection, CapabilitySpec};
use crate::constant_rate_model::{ModelArtifactType, ModelConstructorRegistry, ModelSelection};
use crate::equation_classification::{
    execute_equation_classification, route_classified_equation, EquationClassificationFailure,
    EquationClassificationReceipt, EquationRoutingFailure,
};
use crate::equation_normalization::{
    execute_equation_normalization, EquationNormalizationFailure,
};
use crate::evidence::{
    DerivedFact, DerivedFactIndex, FactConflict, FactDerivationRejection, FactIndexInsert,
    FactIndexQueryFailure, FactIndexRejection, FactPolicy, FactPolicyRejection, FactStatus,
};
use crate::formalization::{AnswerForm, FormalizedTarget, OperationKind, SubjectObjectType};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilityPlanningFailure {
    NoEligibleCapability,
    AmbiguousCapabilities(Vec<String>),
    DependencyUnavailable(String),
    DependencyCycle(String),
    NoProducer(CapabilityIoType),
    MissingInputs {
        capability: String,
        missing: Vec<CapabilityIoType>,
    },
    MissingFactPolicy(String),
    InvalidDerivedFacts {
        capability: String,
        rejections: Vec<DerivedFactRejection>,
    },
    FactIndex(FactIndexQueryFailure),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainPlan {
    pub goal: CapabilityIoType,
    pub steps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum EquationChainPlanningFailure {
    UnsupportedGoal(CapabilityIoType),
    MissingTargetVariable,
    Normalization(EquationNormalizationFailure),
    Classification(EquationClassificationFailure),
    Routing(EquationRoutingFailure),
    CapabilityUnavailable(String),
    TrustPolicy(VerifiedArtifactPlanningFailure),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum VerifiedArtifactPlanningFailure {
    InsufficientProofSteps { required: usize, available: usize },
    MissingStepVerifier(String),
    MissingFinalVerificationStep,
}

/// A source-grounded equation plan. Classification is performed and verified
/// before the solver is selected; this is still a planning receipt and does
/// not authorize execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EquationChainPlan {
    pub source: String,
    pub target_variable: String,
    pub normalized_equation: String,
    pub classification: EquationClassificationReceipt,
    pub selected_solver: String,
    pub chain: CapabilityChainPlan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilityChainPlanningFailure {
    NoProducer(CapabilityIoType),
    AmbiguousProducers {
        goal: CapabilityIoType,
        candidates: Vec<String>,
    },
    DependencyUnavailable {
        capability: String,
        dependency: String,
    },
    DependencyCycle(String),
    UnknownCapability(String),
    TrustPolicy(VerifiedArtifactPlanningFailure),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilityChainRepairFailure {
    ExecutionNotFailed(CapabilityChainExecutionStatus),
    MissingFailedStep,
    InvalidFailedStep(usize),
    EmptyReplacement,
    UnknownCapability(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainRankedCandidate {
    pub candidate_id: String,
    pub cost: PlanCost,
    pub diagnostics: CapabilityChainDiagnostics,
}

/// Secondary, non-authorizing signals for comparing capability chains. These
/// expose verification coverage and contract burden without pretending that a
/// scalar score can resolve a semantic tradeoff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CapabilityChainDiagnostics {
    pub verification_evidence: usize,
    pub contract_burden: usize,
    pub quality_failures: usize,
}

/// Diagnostic preference among already-valid capability chains. A preference
/// is never an authorization decision: callers must apply an independent
/// policy before selecting or executing any candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilityChainPreference {
    NoCandidates,
    Preferred(String),
    Ambiguous(Vec<String>),
}

/// Auditable record of chain ranking and any deterministic preference it
/// exposes. Equal-cost candidates remain ambiguous even when their IDs give
/// the ranked list a stable order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainPreferenceReceipt {
    pub ranked_candidates: Vec<CapabilityChainRankedCandidate>,
    pub preference: CapabilityChainPreference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilityChainExplanationNote {
    LowestCost { candidate_id: String },
    EqualCost { candidate_ids: Vec<String> },
    HigherCost { candidate_id: String, cost: PlanCost },
    VerificationEvidence { candidate_id: String, cases: usize },
    ContractBurden { candidate_id: String, requirements: usize },
    QualityFailures { candidate_id: String, failures: usize },
}

/// Structured, deterministic explanation of a preference receipt. Notes are
/// observations about tradeoffs; they do not alter authorization or ranking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainPreferenceExplanation {
    pub preference: CapabilityChainPreference,
    pub preferred_because: Vec<CapabilityChainExplanationNote>,
    pub tradeoffs: Vec<CapabilityChainExplanationNote>,
    pub alternatives: Vec<String>,
}

impl CapabilityChainPreferenceReceipt {
    pub fn explain(&self) -> CapabilityChainPreferenceExplanation {
        let alternatives = self
            .ranked_candidates
            .iter()
            .map(|candidate| candidate.candidate_id.clone())
            .collect::<Vec<_>>();
        let mut preferred_because = Vec::new();
        let mut tradeoffs = Vec::new();
        match &self.preference {
            CapabilityChainPreference::Preferred(candidate_id) => {
                preferred_because.push(CapabilityChainExplanationNote::LowestCost {
                    candidate_id: candidate_id.clone(),
                });
                for candidate in &self.ranked_candidates {
                    if &candidate.candidate_id != candidate_id {
                        tradeoffs.push(CapabilityChainExplanationNote::HigherCost {
                            candidate_id: candidate.candidate_id.clone(),
                            cost: candidate.cost,
                        });
                    }
                }
            }
            CapabilityChainPreference::Ambiguous(candidate_ids) => {
                tradeoffs.push(CapabilityChainExplanationNote::EqualCost {
                    candidate_ids: candidate_ids.clone(),
                });
            }
            CapabilityChainPreference::NoCandidates => {}
        }
        for candidate in &self.ranked_candidates {
            if candidate.diagnostics.verification_evidence > 0 {
                tradeoffs.push(CapabilityChainExplanationNote::VerificationEvidence {
                    candidate_id: candidate.candidate_id.clone(),
                    cases: candidate.diagnostics.verification_evidence,
                });
            }
            if candidate.diagnostics.contract_burden > 0 {
                tradeoffs.push(CapabilityChainExplanationNote::ContractBurden {
                    candidate_id: candidate.candidate_id.clone(),
                    requirements: candidate.diagnostics.contract_burden,
                });
            }
            if candidate.diagnostics.quality_failures > 0 {
                tradeoffs.push(CapabilityChainExplanationNote::QualityFailures {
                    candidate_id: candidate.candidate_id.clone(),
                    failures: candidate.diagnostics.quality_failures,
                });
            }
        }
        CapabilityChainPreferenceExplanation {
            preference: self.preference.clone(),
            preferred_because,
            tradeoffs,
            alternatives,
        }
    }
}

/// Proposal-only replacement for one failed chain step. Constructing a
/// candidate never installs, authorizes, or executes the replacement plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainRepairCandidate {
    pub execution_id: String,
    pub failed_step: usize,
    pub original_plan: CapabilityChainPlan,
    pub replacement_capabilities: Vec<String>,
    pub proposed_plan: CapabilityChainPlan,
    pub evaluation: PlanRepairEvaluation,
}

/// Generate replacement-subchain proposals for the failed step of a terminal
/// chain execution. The caller must revalidate artifact compatibility and
/// apply a separate decision policy before any proposal can be used.
pub fn propose_capability_chain_repairs(
    execution: &CapabilityChainExecutionReceipt,
    replacements: impl IntoIterator<Item = Vec<String>>,
    registry: &CapabilityRegistry,
) -> Result<Vec<CapabilityChainRepairCandidate>, CapabilityChainRepairFailure> {
    if execution.status != CapabilityChainExecutionStatus::Failed {
        return Err(CapabilityChainRepairFailure::ExecutionNotFailed(
            execution.status,
        ));
    }
    let failed_step = execution
        .failed_step
        .ok_or(CapabilityChainRepairFailure::MissingFailedStep)?;
    if failed_step >= execution.plan.steps.len() {
        return Err(CapabilityChainRepairFailure::InvalidFailedStep(failed_step));
    }
    let mut proposals = Vec::new();
    for replacement_capabilities in replacements {
        if replacement_capabilities.is_empty() {
            return Err(CapabilityChainRepairFailure::EmptyReplacement);
        }
        for capability_id in &replacement_capabilities {
            if registry.get(capability_id).is_none() {
                return Err(CapabilityChainRepairFailure::UnknownCapability(
                    capability_id.clone(),
                ));
            }
        }
        let mut steps = execution.plan.steps[..failed_step].to_vec();
        steps.extend(replacement_capabilities.iter().cloned());
        steps.extend(execution.plan.steps[failed_step + 1..].iter().cloned());
        let proposed_plan = CapabilityChainPlan {
            goal: execution.plan.goal,
            steps,
        };
        let original_cost = execution
            .plan
            .cost(registry)
            .map_err(|error| CapabilityChainRepairFailure::UnknownCapability(format!(
                "{error:?}"
            )))?;
        let replacement_cost = proposed_plan
            .cost(registry)
            .map_err(|error| CapabilityChainRepairFailure::UnknownCapability(format!(
                "{error:?}"
            )))?;
        let evaluation = PlanRepairEvaluation {
            plan_id: execution.execution_id.clone(),
            old_cost: original_cost,
            replacement_cost,
            cost_delta: PlanCostDelta {
                steps: replacement_cost.steps as i64 - original_cost.steps as i64,
                dependency_edges: replacement_cost.dependency_edges as i64
                    - original_cost.dependency_edges as i64,
                verification_steps: replacement_cost.verification_steps as i64
                    - original_cost.verification_steps as i64,
            },
            added_capabilities: replacement_capabilities.clone(),
            removed_capabilities: vec![execution.plan.steps[failed_step].clone()],
            invalidated_fact_ids: Vec::new(),
            replacement_fact_ids: Vec::new(),
        };
        proposals.push(CapabilityChainRepairCandidate {
            execution_id: execution.execution_id.clone(),
            failed_step,
            original_plan: execution.plan.clone(),
            replacement_capabilities,
            proposed_plan,
            evaluation,
        });
    }
    Ok(proposals)
}

/// Compare chain-repair proposals using the same non-authorizing preference
/// machinery as ordinary chains. Candidate IDs are deterministic fingerprints
/// of the failed execution, step, and replacement subchain.
pub fn diagnose_capability_chain_repair_preferences(
    candidates: &[CapabilityChainRepairCandidate],
    registry: &CapabilityRegistry,
) -> Result<CapabilityChainPreferenceReceipt, CapabilityChainPlanningFailure> {
    let chains = candidates.iter().map(|candidate| {
        let candidate_id = format!(
            "{}:step{}:{}",
            candidate.execution_id,
            candidate.failed_step,
            candidate.replacement_capabilities.join("->")
        );
        (candidate_id, candidate.proposed_plan.clone())
    });
    diagnose_capability_chain_preferences(chains, registry)
}

/// Repair-specific explanation context around the shared chain preference
/// explanation. An empty proposal set yields no explanation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainRepairPreferenceExplanation {
    pub execution_id: String,
    pub failed_step: usize,
    pub original_capability: String,
    pub preference: CapabilityChainPreference,
    pub explanation: CapabilityChainPreferenceExplanation,
}

pub fn explain_capability_chain_repair_preferences(
    candidates: &[CapabilityChainRepairCandidate],
    registry: &CapabilityRegistry,
) -> Result<Option<CapabilityChainRepairPreferenceExplanation>, CapabilityChainPlanningFailure> {
    let Some(first) = candidates.first() else {
        return Ok(None);
    };
    let receipt = diagnose_capability_chain_repair_preferences(candidates, registry)?;
    Ok(Some(CapabilityChainRepairPreferenceExplanation {
        execution_id: first.execution_id.clone(),
        failed_step: first.failed_step,
        original_capability: first.original_plan.steps[first.failed_step].clone(),
        preference: receipt.preference.clone(),
        explanation: receipt.explain(),
    }))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilityChainRepairValidationFailure {
    ExecutionNotSucceeded(CapabilityChainExecutionStatus),
    PlanMismatch,
    IncompleteChain { expected: usize, recorded: usize },
    MissingVerification { step: usize },
}

/// Receipt proving that a proposed replacement was executed separately and
/// completed with verification. Validation does not install or authorize the
/// repaired plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainRepairValidationReceipt {
    pub source_execution_id: String,
    pub repaired_execution_id: String,
    pub failed_step: usize,
    pub verified_steps: usize,
}

pub fn validate_capability_chain_repair(
    candidate: &CapabilityChainRepairCandidate,
    repaired_execution: &CapabilityChainExecutionReceipt,
) -> Result<CapabilityChainRepairValidationReceipt, CapabilityChainRepairValidationFailure> {
    if repaired_execution.status != CapabilityChainExecutionStatus::Succeeded {
        return Err(CapabilityChainRepairValidationFailure::ExecutionNotSucceeded(
            repaired_execution.status,
        ));
    }
    if repaired_execution.plan != candidate.proposed_plan {
        return Err(CapabilityChainRepairValidationFailure::PlanMismatch);
    }
    if repaired_execution.steps.len() != repaired_execution.plan.steps.len() {
        return Err(CapabilityChainRepairValidationFailure::IncompleteChain {
            expected: repaired_execution.plan.steps.len(),
            recorded: repaired_execution.steps.len(),
        });
    }
    for (step, receipt) in repaired_execution.steps.iter().enumerate() {
        if receipt.verification_receipt.trim().is_empty() {
            return Err(CapabilityChainRepairValidationFailure::MissingVerification { step });
        }
    }
    Ok(CapabilityChainRepairValidationReceipt {
        source_execution_id: candidate.execution_id.clone(),
        repaired_execution_id: repaired_execution.execution_id.clone(),
        failed_step: candidate.failed_step,
        verified_steps: repaired_execution.steps.len(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CapabilityChainRepairApprovalDecision {
    Approved,
    Deferred,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainRepairApprovalReceipt {
    pub approval_id: String,
    pub execution_id: String,
    pub proposed_plan: CapabilityChainPlan,
    pub validation: CapabilityChainRepairValidationReceipt,
    pub decision: CapabilityChainRepairApprovalDecision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilityChainRepairApprovalFailure {
    DuplicateApproval(String),
    ValidationMismatch,
    UnknownApproval(String),
    ApprovalNotGranted(CapabilityChainRepairApprovalDecision),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct CapabilityChainRepairApprovalLedger {
    approvals: BTreeMap<String, CapabilityChainRepairApprovalReceipt>,
}

impl CapabilityChainRepairApprovalLedger {
    pub fn record(
        &mut self,
        approval_id: impl Into<String>,
        candidate: &CapabilityChainRepairCandidate,
        validation: CapabilityChainRepairValidationReceipt,
        decision: CapabilityChainRepairApprovalDecision,
    ) -> Result<CapabilityChainRepairApprovalReceipt, CapabilityChainRepairApprovalFailure> {
        let approval_id = approval_id.into();
        if self.approvals.contains_key(&approval_id) {
            return Err(CapabilityChainRepairApprovalFailure::DuplicateApproval(
                approval_id,
            ));
        }
        if validation.source_execution_id != candidate.execution_id
            || validation.failed_step != candidate.failed_step
            || validation.repaired_execution_id.is_empty()
        {
            return Err(CapabilityChainRepairApprovalFailure::ValidationMismatch);
        }
        let receipt = CapabilityChainRepairApprovalReceipt {
            approval_id: approval_id.clone(),
            execution_id: candidate.execution_id.clone(),
            proposed_plan: candidate.proposed_plan.clone(),
            validation,
            decision,
        };
        self.approvals.insert(approval_id, receipt.clone());
        Ok(receipt)
    }

    pub fn receipt(&self, approval_id: &str) -> Option<&CapabilityChainRepairApprovalReceipt> {
        self.approvals.get(approval_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CapabilityChainRepairInstallationStatus {
    Prepared,
    Applied,
    Failed,
    RolledBack,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainRepairInstallationReceipt {
    pub installation_id: String,
    pub approval_id: String,
    pub plan: CapabilityChainPlan,
    pub status: CapabilityChainRepairInstallationStatus,
    pub verification_receipt: Option<String>,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilityChainRepairInstallationFailure {
    DuplicateInstallation(String),
    ApprovalNotGranted(CapabilityChainRepairApprovalDecision),
    PlanMismatch,
    UnknownInstallation(String),
    InvalidTransition(CapabilityChainRepairInstallationStatus),
    MissingVerificationReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct CapabilityChainRepairInstallationLedger {
    installations: BTreeMap<String, CapabilityChainRepairInstallationReceipt>,
}

impl CapabilityChainRepairInstallationLedger {
    pub fn prepare(
        &mut self,
        installation_id: impl Into<String>,
        approval: &CapabilityChainRepairApprovalReceipt,
        plan: CapabilityChainPlan,
    ) -> Result<CapabilityChainRepairInstallationReceipt, CapabilityChainRepairInstallationFailure> {
        let installation_id = installation_id.into();
        if self.installations.contains_key(&installation_id) {
            return Err(CapabilityChainRepairInstallationFailure::DuplicateInstallation(
                installation_id,
            ));
        }
        if approval.decision != CapabilityChainRepairApprovalDecision::Approved {
            return Err(CapabilityChainRepairInstallationFailure::ApprovalNotGranted(
                approval.decision,
            ));
        }
        if plan != approval.proposed_plan {
            return Err(CapabilityChainRepairInstallationFailure::PlanMismatch);
        }
        let receipt = CapabilityChainRepairInstallationReceipt {
            installation_id: installation_id.clone(),
            approval_id: approval.approval_id.clone(),
            plan,
            status: CapabilityChainRepairInstallationStatus::Prepared,
            verification_receipt: None,
            failure_reason: None,
        };
        self.installations.insert(installation_id, receipt.clone());
        Ok(receipt)
    }

    pub fn mark_applied(
        &mut self,
        installation_id: &str,
        verification_receipt: impl Into<String>,
    ) -> Result<CapabilityChainRepairInstallationReceipt, CapabilityChainRepairInstallationFailure> {
        let receipt = self
            .installations
            .get_mut(installation_id)
            .ok_or_else(|| CapabilityChainRepairInstallationFailure::UnknownInstallation(
                installation_id.into(),
            ))?;
        if receipt.status != CapabilityChainRepairInstallationStatus::Prepared {
            return Err(CapabilityChainRepairInstallationFailure::InvalidTransition(
                receipt.status,
            ));
        }
        let verification_receipt = verification_receipt.into();
        if verification_receipt.trim().is_empty() {
            return Err(CapabilityChainRepairInstallationFailure::MissingVerificationReceipt);
        }
        receipt.status = CapabilityChainRepairInstallationStatus::Applied;
        receipt.verification_receipt = Some(verification_receipt);
        Ok(receipt.clone())
    }

    pub fn mark_failed(
        &mut self,
        installation_id: &str,
        reason: impl Into<String>,
    ) -> Result<CapabilityChainRepairInstallationReceipt, CapabilityChainRepairInstallationFailure> {
        let receipt = self
            .installations
            .get_mut(installation_id)
            .ok_or_else(|| CapabilityChainRepairInstallationFailure::UnknownInstallation(
                installation_id.into(),
            ))?;
        if receipt.status != CapabilityChainRepairInstallationStatus::Prepared {
            return Err(CapabilityChainRepairInstallationFailure::InvalidTransition(
                receipt.status,
            ));
        }
        receipt.status = CapabilityChainRepairInstallationStatus::Failed;
        receipt.failure_reason = Some(reason.into());
        Ok(receipt.clone())
    }

    pub fn rollback(
        &mut self,
        installation_id: &str,
    ) -> Result<CapabilityChainRepairInstallationReceipt, CapabilityChainRepairInstallationFailure> {
        let receipt = self
            .installations
            .get_mut(installation_id)
            .ok_or_else(|| CapabilityChainRepairInstallationFailure::UnknownInstallation(
                installation_id.into(),
            ))?;
        if receipt.status != CapabilityChainRepairInstallationStatus::Applied {
            return Err(CapabilityChainRepairInstallationFailure::InvalidTransition(
                receipt.status,
            ));
        }
        receipt.status = CapabilityChainRepairInstallationStatus::RolledBack;
        Ok(receipt.clone())
    }
}

impl CapabilityChainPlan {
    /// Compute deterministic chain cost for diagnostics and preference
    /// reporting. Cost never authorizes a plan or resolves an ambiguity.
    pub fn cost(
        &self,
        registry: &CapabilityRegistry,
    ) -> Result<PlanCost, CapabilityChainPlanningFailure> {
        let mut dependency_edges = 0;
        let mut verification_steps = 0;
        for capability_id in &self.steps {
            let capability = registry
                .get(capability_id)
                .ok_or_else(|| CapabilityChainPlanningFailure::UnknownCapability(
                    capability_id.clone(),
                ))?;
            dependency_edges += capability.dependencies.len();
            if !capability.verifier.trim().is_empty() {
                verification_steps += 1;
            }
        }
        Ok(PlanCost {
            steps: self.steps.len(),
            dependency_edges,
            verification_steps,
        })
    }

    /// Compute non-authorizing quality diagnostics for a chain.
    pub fn diagnostics(
        &self,
        registry: &CapabilityRegistry,
    ) -> Result<CapabilityChainDiagnostics, CapabilityChainPlanningFailure> {
        let mut verification_evidence = 0;
        let mut contract_burden = 0;
        let mut quality_failures = 0;
        for capability_id in &self.steps {
            let capability = registry
                .get(capability_id)
                .ok_or_else(|| CapabilityChainPlanningFailure::UnknownCapability(
                    capability_id.clone(),
                ))?;
            verification_evidence += capability.regression_cases.len();
            contract_burden += capability.input_requirements.len();
            quality_failures += capability.quality_gate.false_authorizations;
            quality_failures += capability.quality_gate.replay_failures;
        }
        Ok(CapabilityChainDiagnostics {
            verification_evidence,
            contract_burden,
            quality_failures,
        })
    }
}

/// Rank already-valid candidate chains for diagnostics only. The caller
/// must still apply a separate authorization policy; this never selects or
/// executes a candidate.
pub fn rank_capability_chains(
    candidates: impl IntoIterator<Item = (String, CapabilityChainPlan)>,
    registry: &CapabilityRegistry,
) -> Result<Vec<CapabilityChainRankedCandidate>, CapabilityChainPlanningFailure> {
    let mut ranked = candidates
        .into_iter()
        .map(|(candidate_id, plan)| {
            let cost = plan.cost(registry)?;
            let diagnostics = plan.diagnostics(registry)?;
            Ok(CapabilityChainRankedCandidate {
                candidate_id,
                cost,
                diagnostics,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    ranked.sort_by(|left, right| {
        left.cost
            .cmp(&right.cost)
            .then(left.candidate_id.cmp(&right.candidate_id))
    });
    Ok(ranked)
}

/// Produce a diagnostic chain-preference receipt. This records a unique
/// lowest-cost preference only; ties are explicitly reported as ambiguous.
/// The receipt does not select, authorize, or execute a chain.
pub fn diagnose_capability_chain_preferences(
    candidates: impl IntoIterator<Item = (String, CapabilityChainPlan)>,
    registry: &CapabilityRegistry,
) -> Result<CapabilityChainPreferenceReceipt, CapabilityChainPlanningFailure> {
    let ranked_candidates = rank_capability_chains(candidates, registry)?;
    let preference = match ranked_candidates.first() {
        None => CapabilityChainPreference::NoCandidates,
        Some(first) => {
            let tied = ranked_candidates
                .iter()
                .take_while(|candidate| candidate.cost == first.cost)
                .map(|candidate| candidate.candidate_id.clone())
                .collect::<Vec<_>>();
            if tied.len() == 1 {
                CapabilityChainPreference::Preferred(tied[0].clone())
            } else {
                CapabilityChainPreference::Ambiguous(tied)
            }
        }
    };
    Ok(CapabilityChainPreferenceReceipt {
        ranked_candidates,
        preference,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CapabilityChainExecutionStatus {
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainStepReceipt {
    pub step_index: usize,
    pub capability_id: String,
    pub input_artifacts: Vec<String>,
    pub output_artifacts: Vec<String>,
    pub verification_receipt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainExecutionReceipt {
    pub execution_id: String,
    pub plan: CapabilityChainPlan,
    pub status: CapabilityChainExecutionStatus,
    pub steps: Vec<CapabilityChainStepReceipt>,
    pub failed_step: Option<usize>,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofStep {
    pub step_index: usize,
    pub capability_id: String,
    pub input_artifacts: Vec<String>,
    pub output_artifacts: Vec<String>,
    pub verification_receipt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofTrace {
    pub execution_id: String,
    pub plan: CapabilityChainPlan,
    pub steps: Vec<CapabilityChainProofStep>,
    pub retrieved_facts: Vec<DerivedFactProof>,
    pub final_artifacts: Vec<String>,
    pub replay_verified: bool,
}

impl CapabilityChainProofTrace {
    /// Return a deterministic identity for the reasoning content of this
    /// trace. Execution IDs identify runs, not the reasoning itself, so they
    /// are excluded from the fingerprint used for comparison and reuse.
    pub fn reasoning_fingerprint(&self) -> String {
        let mut canonical = self.clone();
        canonical.execution_id.clear();
        let encoded = serde_json::to_vec(&canonical)
            .expect("capability-chain proof traces must be serializable");
        format!("{:x}", Sha256::digest(encoded))
    }

    /// Compare two traces by canonical reasoning content while ignoring the
    /// execution-run identifier.
    pub fn same_reasoning(&self, other: &Self) -> bool {
        self.reasoning_fingerprint() == other.reasoning_fingerprint()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilityChainProofIndexFailure {
    UnverifiedProof,
    IncompleteSteps { expected: usize, actual: usize },
    MissingVerificationReceipt(usize),
    MissingFactRetrievalReceipt(String),
    DuplicateFactRetrieval { capability: String, fact_id: String },
    DuplicateFingerprint(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofCost {
    pub proof_steps: usize,
    pub retrieved_facts: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofPreferenceCandidate {
    pub fingerprint: String,
    pub cost: CapabilityChainProofCost,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofPreferenceReceipt {
    pub goal_artifact: String,
    pub candidates: Vec<CapabilityChainProofPreferenceCandidate>,
    pub preferred_fingerprint: Option<String>,
    pub ambiguous: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofPolicyRejection {
    pub fingerprint: String,
    pub reason: VerifiedArtifactPolicyFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofPolicyPreferenceReceipt {
    pub goal_artifact: String,
    pub policy: VerifiedArtifactPolicy,
    pub accepted: CapabilityChainProofPreferenceReceipt,
    pub rejected: Vec<CapabilityChainProofPolicyRejection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilityChainProofSynthesisSource {
    ReusedProof(CapabilityChainProofTrace),
    CapabilityPlanPending(CapabilityChainPlan),
    MixedPrefixPlanPending {
        prefix: CapabilityChainProofTrace,
        handoff_artifacts: Vec<String>,
        plan: CapabilityChainPlan,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofSynthesisReceipt {
    pub goal_artifact: String,
    pub policy: VerifiedArtifactPolicy,
    pub source: CapabilityChainProofSynthesisSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilityChainProofSynthesisFailure {
    NoProofOrCapabilityPlan,
    CapabilityPlanInsufficientProofSteps { required: usize, available: usize },
    PrefixNotIndexed(String),
    UnverifiedPrefix(String),
    IncompatibleHandoff { produced: Vec<String>, handoff: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilityChainProofSynthesisValidationFailure {
    NotMixedSynthesis,
    PlanMismatch,
    HandoffNotRecorded(Vec<String>),
    Composition(CapabilityChainProofFailure),
    Artifact(VerifiedArtifactFailure),
    Policy(VerifiedArtifactPolicyFailure),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofSynthesisValidationReceipt<T> {
    pub artifact: VerifiedArtifact<T>,
    pub prefix_fingerprint: String,
    pub execution_id: String,
    pub handoff_artifacts: Vec<String>,
    pub policy: VerifiedArtifactPolicyReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofSynthesisPublicationReceipt {
    pub proof_fingerprint: String,
    pub fact_publication: VerifiedArtifactFactPublicationReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilityChainProofSynthesisPublicationFailure {
    ProofIndex(CapabilityChainProofIndexFailure),
    FactBridge(VerifiedArtifactFactBridgeFailure),
    FactIndex(FactIndexRejection),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofSynthesisCandidate {
    pub candidate_id: String,
    pub prefix_fingerprint: String,
    pub handoff_artifacts: Vec<String>,
    pub pending_plan: CapabilityChainPlan,
    pub cost: CapabilityChainProofCost,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofSynthesisPreferenceReceipt {
    pub goal_artifact: String,
    pub policy: VerifiedArtifactPolicy,
    pub candidates: Vec<CapabilityChainProofSynthesisCandidate>,
    pub preferred_candidate_id: Option<String>,
    pub ambiguous: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofAbstraction {
    pub pattern_id: String,
    pub goal: CapabilityIoType,
    pub capabilities: Vec<String>,
    pub final_artifacts: Vec<String>,
    pub instances: usize,
    pub representative_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofAbstractionProposal {
    pub pattern: CapabilityChainProofAbstraction,
    pub minimum_instances: usize,
    pub expected_effect: String,
    pub risk: ImprovementRisk,
    pub validation_requirements: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofAbstractionExperimentReceipt {
    pub proposal: CapabilityChainProofAbstractionProposal,
    pub baseline_instances: usize,
    pub post_instances: usize,
    pub baseline_false_authorizations: usize,
    pub post_false_authorizations: usize,
    pub pattern_recurred: bool,
    pub safety_preserved: bool,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofAbstractionExperimentRiskSummary {
    pub risk: ImprovementRisk,
    pub attempts: usize,
    pub passed: usize,
    pub safety_failures: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofAbstractionMetaLearningProfile {
    pub total_experiments: usize,
    pub total_passed: usize,
    pub risk_summaries: Vec<CapabilityChainProofAbstractionExperimentRiskSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilityChainProofAbstractionExperimentLedgerRejection {
    DuplicateExperiment(String),
}

/// Durable record of abstraction experiments.  The profile derived from this
/// ledger is descriptive evidence for future planning, not an automatic
/// change to risk weights or authorization policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct CapabilityChainProofAbstractionExperimentLedger {
    receipts: BTreeMap<String, CapabilityChainProofAbstractionExperimentReceipt>,
}

/// Evidence that an abstraction hypothesis transferred beyond the instances
/// from which it was discovered.  This is a validation receipt, not a proof
/// of universal generality.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofAbstractionGeneralizationReceipt {
    pub pattern_id: String,
    pub held_out_cases: usize,
    pub required_held_out_cases: usize,
    pub held_out_replay_failures: usize,
    pub held_out_false_authorizations: usize,
    pub passed: bool,
}

/// Evidence that a proposed abstraction contributes a distinct capability
/// contract rather than merely renaming an existing registry entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofAbstractionNoveltyReceipt {
    pub pattern_id: String,
    pub candidate_id: String,
    pub equivalent_existing_capabilities: Vec<String>,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofAbstractionValueScore {
    pub proof_steps: usize,
    pub dependency_count: usize,
    pub contract_burden: usize,
    pub replay_failures: usize,
    pub false_authorizations: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofAbstractionValueAlternative {
    pub capability_id: String,
    pub score: CapabilityChainProofAbstractionValueScore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CapabilityChainProofAbstractionValueDecision {
    Preferred,
    Ambiguous,
    NotPreferred,
    NoBaseline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofAbstractionValueReceipt {
    pub pattern_id: String,
    pub candidate_id: String,
    pub goal: CapabilityIoType,
    pub candidate_score: CapabilityChainProofAbstractionValueScore,
    pub alternatives: Vec<CapabilityChainProofAbstractionValueAlternative>,
    pub decision: CapabilityChainProofAbstractionValueDecision,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofAbstractionPriorityInput {
    pub proposal: CapabilityChainProofAbstractionProposal,
    pub value: CapabilityChainProofAbstractionValueReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofAbstractionPriorityScore {
    pub recurrence_signal: usize,
    pub value_signal: usize,
    pub risk_signal: usize,
    pub complexity_penalty: usize,
    pub expected_gain: usize,
    pub validation_cost: usize,
    pub efficiency_numerator: usize,
    pub efficiency_denominator: usize,
    pub total: isize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofAbstractionPriorityCandidate {
    pub pattern_id: String,
    pub score: CapabilityChainProofAbstractionPriorityScore,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofAbstractionPriorityReceipt {
    pub candidates: Vec<CapabilityChainProofAbstractionPriorityCandidate>,
    pub preferred_pattern_ids: Vec<String>,
    pub ambiguous: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofAbstractionPortfolioReceipt {
    pub budget: usize,
    pub selected_pattern_ids: Vec<String>,
    pub selected_expected_gain: usize,
    pub selected_validation_cost: usize,
    pub tied_portfolios: Vec<Vec<String>>,
    pub ambiguous: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilityChainProofAbstractionPriorityFailure {
    PatternMismatch(String),
    DuplicatePattern(String),
}

/// A diagnostic recommendation for a recurring proof abstraction.  This is
/// deliberately separate from the proposal and experiment: recurrence and
/// safety evidence may justify review, but do not install a new capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofAbstractionRecommendation {
    pub pattern_id: String,
    pub proposal: CapabilityChainProofAbstractionProposal,
    pub experiment_passed: bool,
    pub action: ImprovementRecommendationAction,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CapabilityChainProofAbstractionApprovalDecision {
    Approved,
    Deferred,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofAbstractionApprovalReceipt {
    pub approval_id: String,
    pub pattern_id: String,
    pub recommendation: CapabilityChainProofAbstractionRecommendation,
    pub decision: CapabilityChainProofAbstractionApprovalDecision,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilityChainProofAbstractionApprovalRejection {
    DuplicateApproval(String),
    RecommendationNotReviewable {
        pattern_id: String,
        action: ImprovementRecommendationAction,
    },
}

/// Records an explicit promotion decision without mutating the capability
/// registry, planner, or proof index.  A later deployment step can consume
/// this receipt, but approval alone never makes an abstraction executable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct CapabilityChainProofAbstractionApprovalLedger {
    approvals: BTreeMap<String, CapabilityChainProofAbstractionApprovalReceipt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CapabilityChainProofAbstractionDeploymentStatus {
    Prepared,
    Applied,
    Failed,
    RolledBack,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofAbstractionDeploymentReceipt {
    pub deployment_id: String,
    pub approval_id: String,
    pub pattern_id: String,
    pub previous_revision: String,
    pub proposed_revision: String,
    pub status: CapabilityChainProofAbstractionDeploymentStatus,
    pub verification_receipt: Option<String>,
    pub failure_reason: Option<String>,
    pub rollback_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilityChainProofAbstractionDeploymentRejection {
    DuplicateDeployment(String),
    ApprovalNotGranted(String),
    UnknownDeployment(String),
    DeploymentAlreadyTerminal(CapabilityChainProofAbstractionDeploymentStatus),
    MissingVerificationReceipt,
    RollbackRequiresApplied,
}

/// A deployed abstraction's governed descriptor.  This is a first-class
/// capability candidate, but not a registry entry: callers must explicitly
/// adapt and register it with an executor before it can affect planning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityChainProofAbstractionCapability {
    pub capability_id: String,
    pub version: u32,
    pub deployment_id: String,
    pub pattern_id: String,
    pub goal: CapabilityIoType,
    pub capabilities: Vec<String>,
    pub final_artifacts: Vec<String>,
    pub verification_receipt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilityChainProofAbstractionMaterializationRejection {
    DeploymentNotApplied(CapabilityChainProofAbstractionDeploymentStatus),
    ApprovalMismatch,
    ApprovalNotGranted,
    MissingVerificationReceipt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CapabilityRegistryEvolutionStatus {
    Prepared,
    Applied,
    Failed,
    RolledBack,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityRegistryEvolutionReceipt {
    pub evolution_id: String,
    pub capability_id: String,
    pub pattern_id: String,
    pub deployment_id: String,
    pub candidate: CapabilitySpec,
    pub status: CapabilityRegistryEvolutionStatus,
    pub verification_receipt: Option<String>,
    pub failure_reason: Option<String>,
    pub rollback_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilityRegistryEvolutionRejection {
    DuplicateEvolution(String),
    CapabilityIdMismatch,
    CapabilityVersionMismatch,
    GoalNotProduced(CapabilityIoType),
    CandidateNotExecutable,
    CapabilityAlreadyRegistered(String),
    UnknownEvolution(String),
    CandidateMismatch,
    EvolutionAlreadyTerminal(CapabilityRegistryEvolutionStatus),
    MissingVerificationReceipt,
    GeneralizationPatternMismatch,
    GeneralizationNotValidated,
    NoveltyPatternMismatch,
    NoveltyNotEstablished(Vec<String>),
    RollbackRequiresApplied,
    RegistryStateMismatch,
}

/// Explicitly integrates an approved abstraction into the live registry.
/// Preparation remains a pure contract check; only `apply` mutates the
/// registry, and it registers exactly the candidate that was prepared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct CapabilityRegistryEvolutionLedger {
    evolutions: BTreeMap<String, CapabilityRegistryEvolutionReceipt>,
}

/// Records the lifecycle of an explicitly approved abstraction deployment.
/// This ledger is intentionally observational: it does not modify the
/// capability registry or planner, so applying a receipt still requires a
/// separate implementation-controlled deployment mechanism.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct CapabilityChainProofAbstractionDeploymentLedger {
    deployments: BTreeMap<String, CapabilityChainProofAbstractionDeploymentReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilityChainProofAbstractionProposalFailure {
    UnknownPattern(String),
    InsufficientInstances { required: usize, actual: usize },
}

/// Explicit cache of proof-bearing reasoning traces. Insertion is deliberate:
/// the index never executes, authorizes, or silently replaces a proof. A
/// caller can retrieve an existing trace by its canonical reasoning identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct CapabilityChainProofIndex {
    proofs: BTreeMap<String, CapabilityChainProofTrace>,
}

impl CapabilityChainProofIndex {
    pub fn insert(
        &mut self,
        proof: CapabilityChainProofTrace,
    ) -> Result<String, CapabilityChainProofIndexFailure> {
        if !proof.replay_verified {
            return Err(CapabilityChainProofIndexFailure::UnverifiedProof);
        }
        if proof.steps.len() != proof.plan.steps.len() {
            return Err(CapabilityChainProofIndexFailure::IncompleteSteps {
                expected: proof.plan.steps.len(),
                actual: proof.steps.len(),
            });
        }
        for step in &proof.steps {
            if step.verification_receipt.trim().is_empty() {
                return Err(CapabilityChainProofIndexFailure::MissingVerificationReceipt(
                    step.step_index,
                ));
            }
        }
        let mut seen_retrievals = BTreeSet::new();
        for fact in &proof.retrieved_facts {
            if fact
                .retrieval_receipt
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                return Err(
                    CapabilityChainProofIndexFailure::MissingFactRetrievalReceipt(
                        fact.fact_id.clone(),
                    ),
                );
            }
            let key = (fact.capability.clone(), fact.fact_id.clone());
            if !seen_retrievals.insert(key) {
                return Err(
                    CapabilityChainProofIndexFailure::DuplicateFactRetrieval {
                        capability: fact.capability.clone(),
                        fact_id: fact.fact_id.clone(),
                    },
                );
            }
        }
        let fingerprint = proof.reasoning_fingerprint();
        if self.proofs.contains_key(&fingerprint) {
            return Err(CapabilityChainProofIndexFailure::DuplicateFingerprint(
                fingerprint,
            ));
        }
        self.proofs.insert(fingerprint.clone(), proof);
        Ok(fingerprint)
    }

    pub fn get(&self, fingerprint: &str) -> Option<&CapabilityChainProofTrace> {
        self.proofs.get(fingerprint)
    }

    pub fn find_equivalent(
        &self,
        proof: &CapabilityChainProofTrace,
    ) -> Option<&CapabilityChainProofTrace> {
        self.get(&proof.reasoning_fingerprint())
    }

    /// Rank indexed proofs as a diagnostic only. Authorization and proof
    /// validity remain governed by the existing verification policies. A tie
    /// at the minimum cost is reported as ambiguous rather than resolved by
    /// map or insertion order.
    pub fn rank_goal_proofs(
        &self,
        goal_artifact: &str,
    ) -> CapabilityChainProofPreferenceReceipt {
        let mut candidates = self
            .proofs
            .iter()
            .filter(|(_, proof)| {
                proof
                    .final_artifacts
                    .iter()
                    .any(|artifact| artifact == goal_artifact)
            })
            .map(|(fingerprint, proof)| CapabilityChainProofPreferenceCandidate {
                fingerprint: fingerprint.clone(),
                cost: CapabilityChainProofCost {
                    proof_steps: proof.steps.len(),
                    retrieved_facts: proof.retrieved_facts.len(),
                },
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            left.cost
                .proof_steps
                .cmp(&right.cost.proof_steps)
                .then_with(|| {
                    left.cost
                        .retrieved_facts
                        .cmp(&right.cost.retrieved_facts)
                })
                .then_with(|| left.fingerprint.cmp(&right.fingerprint))
        });
        let minimum = candidates.first().map(|candidate| &candidate.cost);
        let tied = minimum
            .map(|cost| {
                candidates
                    .iter()
                    .filter(|candidate| &candidate.cost == cost)
                    .count()
            })
            .unwrap_or(0);
        CapabilityChainProofPreferenceReceipt {
            goal_artifact: goal_artifact.into(),
            preferred_fingerprint: (tied == 1).then(|| candidates[0].fingerprint.clone()),
            ambiguous: tied > 1,
            candidates,
        }
    }

    /// Apply a consumer trust policy before ranking proof candidates. Policy
    /// rejection is reported separately from preference, so a cheap proof
    /// that fails the contract cannot compete with admissible proofs.
    pub fn rank_goal_proofs_with_policy(
        &self,
        goal_artifact: &str,
        policy: &VerifiedArtifactPolicy,
    ) -> CapabilityChainProofPolicyPreferenceReceipt {
        let mut accepted = CapabilityChainProofIndex::default();
        let mut rejected = Vec::new();
        for (fingerprint, proof) in &self.proofs {
            if !proof
                .final_artifacts
                .iter()
                .any(|artifact| artifact == goal_artifact)
            {
                continue;
            }
            let final_receipt = proof
                .steps
                .last()
                .map(|step| step.verification_receipt.clone())
                .unwrap_or_default();
            let candidate = VerifiedArtifact {
                artifact: (),
                proof_trace: proof.clone(),
                final_verification_receipt: final_receipt,
            };
            match policy.evaluate(&candidate) {
                Ok(_) => {
                    accepted.proofs.insert(fingerprint.clone(), proof.clone());
                }
                Err(reason) => rejected.push(CapabilityChainProofPolicyRejection {
                    fingerprint: fingerprint.clone(),
                    reason,
                }),
            }
        }
        CapabilityChainProofPolicyPreferenceReceipt {
            goal_artifact: goal_artifact.into(),
            policy: policy.clone(),
            accepted: accepted.rank_goal_proofs(goal_artifact),
            rejected,
        }
    }

    /// Summarize recurring proof structure without generalizing its
    /// provenance. These are diagnostic patterns, not executable proofs or
    /// authorization rules; every instance remains individually indexed.
    pub fn abstract_proof_shapes(&self) -> Vec<CapabilityChainProofAbstraction> {
        let mut grouped: BTreeMap<String, CapabilityChainProofAbstraction> = BTreeMap::new();
        for (fingerprint, proof) in &self.proofs {
            let capabilities = proof
                .steps
                .iter()
                .map(|step| step.capability_id.clone())
                .collect::<Vec<_>>();
            let signature = (
                proof.plan.goal,
                capabilities.clone(),
                proof.final_artifacts.clone(),
            );
            let encoded = serde_json::to_vec(&signature)
                .expect("proof abstraction signature must serialize");
            let pattern_id = format!("{:x}", Sha256::digest(encoded));
            grouped
                .entry(pattern_id.clone())
                .and_modify(|abstraction| {
                    abstraction.instances += 1;
                    if fingerprint < &abstraction.representative_fingerprint {
                        abstraction.representative_fingerprint = fingerprint.clone();
                    }
                })
                .or_insert_with(|| CapabilityChainProofAbstraction {
                    pattern_id,
                    goal: proof.plan.goal,
                    capabilities,
                    final_artifacts: proof.final_artifacts.clone(),
                    instances: 1,
                    representative_fingerprint: fingerprint.clone(),
                });
        }
        grouped.into_values().collect()
    }

    /// Turn a sufficiently recurring structural pattern into an advisory
    /// abstraction hypothesis. The proposal does not alter the registry or
    /// make the pattern executable.
    pub fn propose_proof_abstraction(
        &self,
        pattern_id: &str,
        minimum_instances: usize,
    ) -> Result<CapabilityChainProofAbstractionProposal, CapabilityChainProofAbstractionProposalFailure>
    {
        let pattern = self
            .abstract_proof_shapes()
            .into_iter()
            .find(|pattern| pattern.pattern_id == pattern_id)
            .ok_or_else(|| {
                CapabilityChainProofAbstractionProposalFailure::UnknownPattern(
                    pattern_id.into(),
                )
            })?;
        if pattern.instances < minimum_instances {
            return Err(
                CapabilityChainProofAbstractionProposalFailure::InsufficientInstances {
                    required: minimum_instances,
                    actual: pattern.instances,
                },
            );
        }
        Ok(CapabilityChainProofAbstractionProposal {
            pattern,
            minimum_instances,
            expected_effect:
                "repeatedly observed proof shape may support a reusable abstraction".into(),
            risk: ImprovementRisk::Medium,
            validation_requirements: vec![
                "reproduce the proof shape on held-out inputs".into(),
                "require replay verification for every generated instance".into(),
                "introduce no new false authorizations".into(),
            ],
        })
    }

    /// Search indexed proof fragments for a compatible composed proof. The
    /// search is bounded and deterministic: available inputs must satisfy the
    /// first fragment, each extension must consume an artifact produced by the
    /// current trace, and candidates are visited in fingerprint order.
    pub fn search_composed_proof(
        &self,
        available_inputs: &BTreeSet<String>,
        goal_artifact: &str,
        max_depth: usize,
    ) -> Option<CapabilityChainProofTrace> {
        self.search_composed_proof_internal(available_inputs, goal_artifact, max_depth, None)
    }

    /// Search proof fragments while applying a consumer trust policy at the
    /// goal boundary. Incompatible paths are never returned as solutions;
    /// preference and ranking remain separate diagnostics.
    pub fn search_composed_proof_with_policy(
        &self,
        available_inputs: &BTreeSet<String>,
        goal_artifact: &str,
        max_depth: usize,
        policy: &VerifiedArtifactPolicy,
    ) -> Option<CapabilityChainProofTrace> {
        self.search_composed_proof_internal(
            available_inputs,
            goal_artifact,
            max_depth,
            Some(policy),
        )
    }

    /// Prefer an existing policy-admissible proof; otherwise expose an
    /// explicit, not-yet-executed capability plan as the synthesis fallback.
    /// This method never executes or wraps the pending plan as trusted.
    pub fn synthesize_or_plan(
        &self,
        available_inputs: &BTreeSet<String>,
        goal_artifact: &str,
        max_depth: usize,
        policy: &VerifiedArtifactPolicy,
        fallback_plan: Option<CapabilityChainPlan>,
    ) -> Result<CapabilityChainProofSynthesisReceipt, CapabilityChainProofSynthesisFailure> {
        if let Some(proof) = self.search_composed_proof_with_policy(
            available_inputs,
            goal_artifact,
            max_depth,
            policy,
        ) {
            return Ok(CapabilityChainProofSynthesisReceipt {
                goal_artifact: goal_artifact.into(),
                policy: policy.clone(),
                source: CapabilityChainProofSynthesisSource::ReusedProof(proof),
            });
        }
        let Some(plan) = fallback_plan else {
            return Err(CapabilityChainProofSynthesisFailure::NoProofOrCapabilityPlan);
        };
        if plan.steps.len() < policy.minimum_proof_steps {
            return Err(
                CapabilityChainProofSynthesisFailure::CapabilityPlanInsufficientProofSteps {
                    required: policy.minimum_proof_steps,
                    available: plan.steps.len(),
                },
            );
        }
        Ok(CapabilityChainProofSynthesisReceipt {
            goal_artifact: goal_artifact.into(),
            policy: policy.clone(),
            source: CapabilityChainProofSynthesisSource::CapabilityPlanPending(plan),
        })
    }

    /// Build a mixed-source synthesis draft from an indexed proof prefix and
    /// a pending capability plan. The handoff is explicit and validated, but
    /// no capability executes and no trust wrapper is materialized.
    pub fn synthesize_mixed_prefix_plan(
        &self,
        prefix: &CapabilityChainProofTrace,
        handoff_artifacts: Vec<String>,
        goal_artifact: &str,
        policy: &VerifiedArtifactPolicy,
        pending_plan: CapabilityChainPlan,
    ) -> Result<CapabilityChainProofSynthesisReceipt, CapabilityChainProofSynthesisFailure> {
        let fingerprint = prefix.reasoning_fingerprint();
        if self.get(&fingerprint).is_none() {
            return Err(CapabilityChainProofSynthesisFailure::PrefixNotIndexed(
                fingerprint,
            ));
        }
        if !prefix.replay_verified {
            return Err(CapabilityChainProofSynthesisFailure::UnverifiedPrefix(
                prefix.execution_id.clone(),
            ));
        }
        if handoff_artifacts.is_empty()
            || !handoff_artifacts
                .iter()
                .any(|artifact| prefix.final_artifacts.contains(artifact))
        {
            return Err(CapabilityChainProofSynthesisFailure::IncompatibleHandoff {
                produced: prefix.final_artifacts.clone(),
                handoff: handoff_artifacts,
            });
        }
        if pending_plan.steps.len() < policy.minimum_proof_steps {
            return Err(
                CapabilityChainProofSynthesisFailure::CapabilityPlanInsufficientProofSteps {
                    required: policy.minimum_proof_steps,
                    available: pending_plan.steps.len(),
                },
            );
        }
        Ok(CapabilityChainProofSynthesisReceipt {
            goal_artifact: goal_artifact.into(),
            policy: policy.clone(),
            source: CapabilityChainProofSynthesisSource::MixedPrefixPlanPending {
                prefix: prefix.clone(),
                handoff_artifacts,
                plan: pending_plan,
            },
        })
    }

    /// Compare feasible mixed proof/capability continuations. This is a
    /// preference diagnostic only: the selected path remains pending until
    /// its capability plan executes and produces verification receipts.
    pub fn rank_mixed_synthesis(
        &self,
        goal_artifact: &str,
        continuations: &[(Vec<String>, CapabilityChainPlan)],
        policy: &VerifiedArtifactPolicy,
    ) -> CapabilityChainProofSynthesisPreferenceReceipt {
        let mut candidates = Vec::new();
        for prefix in self.proofs.values() {
            let prefix_fingerprint = prefix.reasoning_fingerprint();
            for (handoff_artifacts, pending_plan) in continuations {
                if handoff_artifacts.is_empty()
                    || !handoff_artifacts
                        .iter()
                        .any(|artifact| prefix.final_artifacts.contains(artifact))
                {
                    continue;
                }
                let total_steps = prefix.steps.len() + pending_plan.steps.len();
                if total_steps < policy.minimum_proof_steps {
                    continue;
                }
                let identity = (
                    goal_artifact,
                    &prefix_fingerprint,
                    handoff_artifacts,
                    &pending_plan.steps,
                );
                let encoded = serde_json::to_vec(&identity)
                    .expect("proof synthesis candidate identity must serialize");
                let candidate_id = format!("{:x}", Sha256::digest(encoded));
                candidates.push(CapabilityChainProofSynthesisCandidate {
                    candidate_id,
                    prefix_fingerprint: prefix_fingerprint.clone(),
                    handoff_artifacts: handoff_artifacts.clone(),
                    pending_plan: pending_plan.clone(),
                    cost: CapabilityChainProofCost {
                        proof_steps: total_steps,
                        retrieved_facts: prefix.retrieved_facts.len(),
                    },
                });
            }
        }
        candidates.sort_by(|left, right| {
            left.cost
                .proof_steps
                .cmp(&right.cost.proof_steps)
                .then_with(|| {
                    left.cost
                        .retrieved_facts
                        .cmp(&right.cost.retrieved_facts)
                })
                .then_with(|| left.candidate_id.cmp(&right.candidate_id))
        });
        let minimum = candidates.first().map(|candidate| &candidate.cost);
        let tied = minimum
            .map(|cost| {
                candidates
                    .iter()
                    .filter(|candidate| &candidate.cost == cost)
                    .count()
            })
            .unwrap_or(0);
        CapabilityChainProofSynthesisPreferenceReceipt {
            goal_artifact: goal_artifact.into(),
            policy: policy.clone(),
            preferred_candidate_id: (tied == 1).then(|| candidates[0].candidate_id.clone()),
            ambiguous: tied > 1,
            candidates,
        }
    }

    fn search_composed_proof_internal(
        &self,
        available_inputs: &BTreeSet<String>,
        goal_artifact: &str,
        max_depth: usize,
        policy: Option<&VerifiedArtifactPolicy>,
    ) -> Option<CapabilityChainProofTrace> {
        if max_depth == 0 {
            return None;
        }
        let mut frontier = self
            .proofs
            .values()
            .filter(|proof| {
                proof.steps.first().is_some_and(|step| {
                    step.input_artifacts
                        .iter()
                        .all(|input| available_inputs.contains(input))
                })
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut seen = frontier
            .iter()
            .map(CapabilityChainProofTrace::reasoning_fingerprint)
            .collect::<BTreeSet<_>>();
        let acceptable = |proof: &CapabilityChainProofTrace| {
            if !proof
                .final_artifacts
                .iter()
                .any(|artifact| artifact == goal_artifact)
            {
                return false;
            }
            let Some(policy) = policy else {
                return true;
            };
            let final_receipt = proof
                .steps
                .last()
                .map(|step| step.verification_receipt.clone())
                .unwrap_or_default();
            let candidate = VerifiedArtifact {
                artifact: (),
                proof_trace: proof.clone(),
                final_verification_receipt: final_receipt,
            };
            policy.evaluate(&candidate).is_ok()
        };

        for _ in 0..max_depth {
            frontier.sort_by_key(CapabilityChainProofTrace::reasoning_fingerprint);
            for proof in &frontier {
                if acceptable(proof) {
                    return Some(proof.clone());
                }
            }
            let mut next = Vec::new();
            for current in &frontier {
                for candidate in self.proofs.values() {
                    let Some(first_step) = candidate.steps.first() else {
                        continue;
                    };
                    if !first_step
                        .input_artifacts
                        .iter()
                        .all(|input| current.final_artifacts.contains(input))
                    {
                        continue;
                    }
                    let Ok(composed) = compose_capability_chain_proofs(current, candidate)
                    else {
                        continue;
                    };
                    let fingerprint = composed.reasoning_fingerprint();
                    if seen.insert(fingerprint) {
                        next.push(composed);
                    }
                }
            }
            if next.is_empty() {
                return None;
            }
            frontier = next;
        }
        None
    }

    pub fn len(&self) -> usize {
        self.proofs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.proofs.is_empty()
    }
}

impl CapabilityChainProofAbstractionProposal {
    /// Compare a novel abstraction with active producers of the same goal.
    /// This is an informational value signal only; it never makes an
    /// otherwise inadmissible candidate executable.
    pub fn assess_value(
        &self,
        candidate: &CapabilitySpec,
        registry: &CapabilityRegistry,
    ) -> CapabilityChainProofAbstractionValueReceipt {
        let candidate_score = CapabilityChainProofAbstractionValueScore {
            proof_steps: self.pattern.capabilities.len(),
            dependency_count: candidate.dependencies.len(),
            contract_burden: candidate.input_requirements.len(),
            replay_failures: candidate.quality_gate.replay_failures,
            false_authorizations: candidate.quality_gate.false_authorizations,
        };
        let mut alternatives = registry
            .capabilities
            .values()
            .filter(|capability| {
                capability.id != candidate.id && capability.produces.contains(&self.pattern.goal)
            })
            .map(|capability| CapabilityChainProofAbstractionValueAlternative {
                capability_id: capability.id.clone(),
                score: CapabilityChainProofAbstractionValueScore {
                    proof_steps: 1,
                    dependency_count: capability.dependencies.len(),
                    contract_burden: capability.input_requirements.len(),
                    replay_failures: capability.quality_gate.replay_failures,
                    false_authorizations: capability.quality_gate.false_authorizations,
                },
            })
            .collect::<Vec<_>>();
        alternatives.sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
        let decision = if alternatives.is_empty() {
            CapabilityChainProofAbstractionValueDecision::NoBaseline
        } else if candidate_score.replay_failures > 0
            || candidate_score.false_authorizations > 0
        {
            CapabilityChainProofAbstractionValueDecision::NotPreferred
        } else {
            let candidate_key = candidate_score.ordering_key();
            let equal = alternatives
                .iter()
                .any(|alternative| alternative.score.ordering_key() == candidate_key);
            let lower = alternatives
                .iter()
                .all(|alternative| candidate_key < alternative.score.ordering_key());
            if equal {
                CapabilityChainProofAbstractionValueDecision::Ambiguous
            } else if lower {
                CapabilityChainProofAbstractionValueDecision::Preferred
            } else {
                CapabilityChainProofAbstractionValueDecision::NotPreferred
            }
        };
        let rationale = match decision {
            CapabilityChainProofAbstractionValueDecision::Preferred => {
                "candidate has a lower diagnostic cost than every active producer".into()
            }
            CapabilityChainProofAbstractionValueDecision::Ambiguous => {
                "candidate and an active producer have equal diagnostic cost".into()
            }
            CapabilityChainProofAbstractionValueDecision::NotPreferred => {
                "candidate is novel but does not dominate the active producer set".into()
            }
            CapabilityChainProofAbstractionValueDecision::NoBaseline => {
                "no active producer exists for a comparative value baseline".into()
            }
        };
        CapabilityChainProofAbstractionValueReceipt {
            pattern_id: self.pattern.pattern_id.clone(),
            candidate_id: candidate.id.clone(),
            goal: self.pattern.goal,
            candidate_score,
            alternatives,
            decision,
            rationale,
        }
    }

    /// Compare the proposed executable contract with active registry entries.
    /// A new identifier alone is not novelty if the complete contract is
    /// already present under another identifier.
    pub fn assess_novelty(
        &self,
        candidate: &CapabilitySpec,
        registry: &CapabilityRegistry,
    ) -> CapabilityChainProofAbstractionNoveltyReceipt {
        let mut equivalent_existing_capabilities = registry
            .capabilities
            .values()
            .filter(|existing| {
                existing.version == candidate.version
                    && existing.kind == candidate.kind
                    && existing.dependencies == candidate.dependencies
                    && existing.consumes == candidate.consumes
                    && existing.produces == candidate.produces
                    && existing.supported_object_types == candidate.supported_object_types
                    && existing.supported_operations == candidate.supported_operations
                    && existing.supported_answer_forms == candidate.supported_answer_forms
                    && existing.input_requirements == candidate.input_requirements
                    && existing.fact_policy == candidate.fact_policy
                    && existing.executor == candidate.executor
                    && existing.verifier == candidate.verifier
            })
            .map(|existing| existing.id.clone())
            .collect::<Vec<_>>();
        equivalent_existing_capabilities.sort();
        CapabilityChainProofAbstractionNoveltyReceipt {
            pattern_id: self.pattern.pattern_id.clone(),
            candidate_id: candidate.id.clone(),
            passed: equivalent_existing_capabilities.is_empty(),
            equivalent_existing_capabilities,
        }
    }

    /// Assess held-out replay evidence separately from recurrence.  A passing
    /// result supports controlled promotion, but does not claim universal
    /// generality or mutate any runtime component.
    pub fn assess_generalization(
        &self,
        held_out_cases: usize,
        required_held_out_cases: usize,
        held_out_replay_failures: usize,
        held_out_false_authorizations: usize,
    ) -> CapabilityChainProofAbstractionGeneralizationReceipt {
        let passed = required_held_out_cases > 0
            && held_out_cases >= required_held_out_cases
            && held_out_replay_failures == 0
            && held_out_false_authorizations == 0;
        CapabilityChainProofAbstractionGeneralizationReceipt {
            pattern_id: self.pattern.pattern_id.clone(),
            held_out_cases,
            required_held_out_cases,
            held_out_replay_failures,
            held_out_false_authorizations,
            passed,
        }
    }

    /// Assess a validation run without applying the proposed abstraction.
    pub fn assess(
        &self,
        post_instances: usize,
        baseline_false_authorizations: usize,
        post_false_authorizations: usize,
    ) -> CapabilityChainProofAbstractionExperimentReceipt {
        let pattern_recurred = post_instances >= self.minimum_instances;
        let safety_preserved = post_false_authorizations <= baseline_false_authorizations;
        CapabilityChainProofAbstractionExperimentReceipt {
            proposal: self.clone(),
            baseline_instances: self.pattern.instances,
            post_instances,
            baseline_false_authorizations,
            post_false_authorizations,
            pattern_recurred,
            safety_preserved,
            passed: pattern_recurred && safety_preserved,
        }
    }
}

impl CapabilityChainProofAbstractionValueScore {
    fn ordering_key(&self) -> (usize, usize, usize, usize, usize) {
        (
            self.proof_steps,
            self.dependency_count,
            self.contract_burden,
            self.replay_failures,
            self.false_authorizations,
        )
    }
}

/// Rank abstraction experiments by expected value for limited validation
/// resources.  The score is a diagnostic priority signal, never an approval
/// or authorization decision; equal maxima remain explicitly ambiguous.
pub fn rank_proof_abstraction_priorities(
    inputs: Vec<CapabilityChainProofAbstractionPriorityInput>,
) -> Result<CapabilityChainProofAbstractionPriorityReceipt, CapabilityChainProofAbstractionPriorityFailure>
{
    let mut seen = BTreeSet::new();
    let mut candidates = Vec::with_capacity(inputs.len());
    for input in inputs {
        let pattern_id = input.proposal.pattern.pattern_id.clone();
        if input.value.pattern_id != pattern_id {
            return Err(CapabilityChainProofAbstractionPriorityFailure::PatternMismatch(
                pattern_id,
            ));
        }
        if !seen.insert(pattern_id.clone()) {
            return Err(CapabilityChainProofAbstractionPriorityFailure::DuplicatePattern(
                pattern_id,
            ));
        }
        let value_signal = match input.value.decision {
            CapabilityChainProofAbstractionValueDecision::Preferred => 100,
            CapabilityChainProofAbstractionValueDecision::Ambiguous => 50,
            CapabilityChainProofAbstractionValueDecision::NoBaseline => 10,
            CapabilityChainProofAbstractionValueDecision::NotPreferred => 0,
        };
        let risk_signal = match input.proposal.risk {
            ImprovementRisk::Low => 30,
            ImprovementRisk::Medium => 15,
            ImprovementRisk::High => 0,
        };
        let recurrence_signal = input.proposal.pattern.instances.saturating_mul(10);
        let complexity_penalty = input.value.candidate_score.proof_steps
            + input.value.candidate_score.dependency_count
            + input.value.candidate_score.contract_burden;
        let expected_gain = recurrence_signal + value_signal + risk_signal;
        let validation_cost = 1 + complexity_penalty;
        let total = expected_gain as isize - complexity_penalty as isize;
        candidates.push(CapabilityChainProofAbstractionPriorityCandidate {
            pattern_id,
            score: CapabilityChainProofAbstractionPriorityScore {
                recurrence_signal,
                value_signal,
                risk_signal,
                complexity_penalty,
                expected_gain,
                validation_cost,
                efficiency_numerator: expected_gain,
                efficiency_denominator: validation_cost,
                total,
            },
        });
    }
    candidates.sort_by(|left, right| {
        let right_efficiency = (right.score.efficiency_numerator as u128)
            * (left.score.efficiency_denominator as u128);
        let left_efficiency = (left.score.efficiency_numerator as u128)
            * (right.score.efficiency_denominator as u128);
        right_efficiency
            .cmp(&left_efficiency)
            .then_with(|| right.score.total.cmp(&left.score.total))
            .then_with(|| left.pattern_id.cmp(&right.pattern_id))
    });
    let top_efficiency = candidates.first().map(|candidate| {
        (
            candidate.score.efficiency_numerator,
            candidate.score.efficiency_denominator,
        )
    });
    let preferred_pattern_ids = top_efficiency
        .map(|(numerator, denominator)| {
            candidates
                .iter()
                .filter(|candidate| {
                    (candidate.score.efficiency_numerator as u128) * (denominator as u128)
                        == (numerator as u128)
                            * (candidate.score.efficiency_denominator as u128)
                })
                .map(|candidate| candidate.pattern_id.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(CapabilityChainProofAbstractionPriorityReceipt {
        ambiguous: preferred_pattern_ids.len() > 1,
        preferred_pattern_ids,
        candidates,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AbstractionPortfolioState {
    expected_gain: usize,
    validation_cost: usize,
    pattern_ids: Vec<String>,
    tied_portfolios: Vec<Vec<String>>,
}

fn merge_portfolio_solutions(
    mut solutions: Vec<Vec<String>>,
    mut additions: Vec<Vec<String>>,
) -> Vec<Vec<String>> {
    solutions.append(&mut additions);
    for ids in &mut solutions {
        ids.sort();
        ids.dedup();
    }
    solutions.sort();
    solutions.dedup();
    solutions.truncate(2);
    solutions
}

/// Select a bounded portfolio of experiments maximizing total expected gain
/// under a validation budget. This is a planning diagnostic: it schedules no
/// experiment and preserves tied portfolios rather than inventing a choice.
pub fn select_abstraction_experiment_portfolio(
    priorities: &CapabilityChainProofAbstractionPriorityReceipt,
    budget: usize,
) -> CapabilityChainProofAbstractionPortfolioReceipt {
    let mut states = vec![None; budget.saturating_add(1)];
    states[0] = Some(AbstractionPortfolioState {
        expected_gain: 0,
        validation_cost: 0,
        pattern_ids: Vec::new(),
        tied_portfolios: vec![Vec::new()],
    });
    for candidate in &priorities.candidates {
        let cost = candidate.score.validation_cost;
        if cost == 0 || cost > budget {
            continue;
        }
        for capacity in (cost..=budget).rev() {
            let Some(previous) = states[capacity - cost].clone() else {
                continue;
            };
            let mut pattern_ids = previous.pattern_ids.clone();
            pattern_ids.push(candidate.pattern_id.clone());
            pattern_ids.sort();
            let combined = AbstractionPortfolioState {
                expected_gain: previous
                    .expected_gain
                    .saturating_add(candidate.score.expected_gain),
                validation_cost: previous.validation_cost.saturating_add(cost),
                pattern_ids,
                tied_portfolios: previous
                    .tied_portfolios
                    .into_iter()
                    .map(|mut ids| {
                        ids.push(candidate.pattern_id.clone());
                        ids
                    })
                    .collect(),
            };
            let replace = match &states[capacity] {
                None => true,
                Some(existing) => {
                    combined.expected_gain > existing.expected_gain
                        || (combined.expected_gain == existing.expected_gain
                            && combined.validation_cost < existing.validation_cost)
                }
            };
            if replace {
                states[capacity] = Some(combined);
            } else if let Some(existing) = states[capacity].as_mut() {
                if combined.expected_gain == existing.expected_gain
                    && combined.validation_cost == existing.validation_cost
                {
                    existing.tied_portfolios = merge_portfolio_solutions(
                        existing.tied_portfolios.clone(),
                        combined.tied_portfolios,
                    );
                }
            }
        }
    }
    let selected = states[budget].clone().unwrap_or(AbstractionPortfolioState {
        expected_gain: 0,
        validation_cost: 0,
        pattern_ids: Vec::new(),
        tied_portfolios: vec![Vec::new()],
    });
    CapabilityChainProofAbstractionPortfolioReceipt {
        budget,
        selected_pattern_ids: selected.pattern_ids,
        selected_expected_gain: selected.expected_gain,
        selected_validation_cost: selected.validation_cost,
        ambiguous: selected.tied_portfolios.len() > 1,
        tied_portfolios: selected.tied_portfolios,
    }
}

impl CapabilityChainProofAbstractionExperimentReceipt {
    /// Convert validation evidence into an advisory governance recommendation.
    /// Passing evidence permits review; it never installs or authorizes the
    /// proposed abstraction by itself.
    pub fn recommendation(&self) -> CapabilityChainProofAbstractionRecommendation {
        let (action, rationale) = if !self.safety_preserved {
            (
                ImprovementRecommendationAction::Reject,
                "validation introduced a new false authorization or failed the safety invariant"
                    .into(),
            )
        } else if self.passed {
            (
                ImprovementRecommendationAction::ReviewForApproval,
                "recurrence and validation evidence support explicit review of the abstraction"
                    .into(),
            )
        } else {
            (
                ImprovementRecommendationAction::GatherMoreEvidence,
                "validation preserved safety but did not yet establish sufficient recurrence"
                    .into(),
            )
        };
        CapabilityChainProofAbstractionRecommendation {
            pattern_id: self.proposal.pattern.pattern_id.clone(),
            proposal: self.proposal.clone(),
            experiment_passed: self.passed,
            action,
            rationale,
        }
    }
}

impl CapabilityChainProofAbstractionApprovalLedger {
    pub fn record(
        &mut self,
        approval_id: impl Into<String>,
        experiment: &CapabilityChainProofAbstractionExperimentReceipt,
        decision: CapabilityChainProofAbstractionApprovalDecision,
        rationale: impl Into<String>,
    ) -> Result<CapabilityChainProofAbstractionApprovalReceipt, CapabilityChainProofAbstractionApprovalRejection>
    {
        let approval_id = approval_id.into();
        if self.approvals.contains_key(&approval_id) {
            return Err(
                CapabilityChainProofAbstractionApprovalRejection::DuplicateApproval(
                    approval_id,
                ),
            );
        }
        let recommendation = experiment.recommendation();
        if decision == CapabilityChainProofAbstractionApprovalDecision::Approved
            && recommendation.action != ImprovementRecommendationAction::ReviewForApproval
        {
            return Err(
                CapabilityChainProofAbstractionApprovalRejection::RecommendationNotReviewable {
                    pattern_id: recommendation.pattern_id.clone(),
                    action: recommendation.action,
                },
            );
        }
        let receipt = CapabilityChainProofAbstractionApprovalReceipt {
            approval_id: approval_id.clone(),
            pattern_id: recommendation.pattern_id.clone(),
            recommendation,
            decision,
            rationale: rationale.into(),
        };
        self.approvals.insert(approval_id, receipt.clone());
        Ok(receipt)
    }

    pub fn receipt(&self, approval_id: &str) -> Option<&CapabilityChainProofAbstractionApprovalReceipt> {
        self.approvals.get(approval_id)
    }

    pub fn receipts(
        &self,
    ) -> impl Iterator<Item = &CapabilityChainProofAbstractionApprovalReceipt> {
        self.approvals.values()
    }
}

impl CapabilityChainProofAbstractionDeploymentLedger {
    pub fn prepare(
        &mut self,
        deployment_id: impl Into<String>,
        approval: &CapabilityChainProofAbstractionApprovalReceipt,
        previous_revision: impl Into<String>,
        proposed_revision: impl Into<String>,
    ) -> Result<CapabilityChainProofAbstractionDeploymentReceipt, CapabilityChainProofAbstractionDeploymentRejection>
    {
        let deployment_id = deployment_id.into();
        if self.deployments.contains_key(&deployment_id) {
            return Err(
                CapabilityChainProofAbstractionDeploymentRejection::DuplicateDeployment(
                    deployment_id,
                ),
            );
        }
        if approval.decision != CapabilityChainProofAbstractionApprovalDecision::Approved {
            return Err(
                CapabilityChainProofAbstractionDeploymentRejection::ApprovalNotGranted(
                    approval.approval_id.clone(),
                ),
            );
        }
        let receipt = CapabilityChainProofAbstractionDeploymentReceipt {
            deployment_id: deployment_id.clone(),
            approval_id: approval.approval_id.clone(),
            pattern_id: approval.pattern_id.clone(),
            previous_revision: previous_revision.into(),
            proposed_revision: proposed_revision.into(),
            status: CapabilityChainProofAbstractionDeploymentStatus::Prepared,
            verification_receipt: None,
            failure_reason: None,
            rollback_reason: None,
        };
        self.deployments.insert(deployment_id, receipt.clone());
        Ok(receipt)
    }

    pub fn mark_applied(
        &mut self,
        deployment_id: &str,
        verification_receipt: impl Into<String>,
    ) -> Result<CapabilityChainProofAbstractionDeploymentReceipt, CapabilityChainProofAbstractionDeploymentRejection>
    {
        let receipt = self.deployments.get_mut(deployment_id).ok_or_else(|| {
            CapabilityChainProofAbstractionDeploymentRejection::UnknownDeployment(
                deployment_id.into(),
            )
        })?;
        if receipt.status != CapabilityChainProofAbstractionDeploymentStatus::Prepared {
            return Err(
                CapabilityChainProofAbstractionDeploymentRejection::DeploymentAlreadyTerminal(
                    receipt.status,
                ),
            );
        }
        let verification_receipt = verification_receipt.into();
        if verification_receipt.trim().is_empty() {
            return Err(
                CapabilityChainProofAbstractionDeploymentRejection::MissingVerificationReceipt,
            );
        }
        receipt.status = CapabilityChainProofAbstractionDeploymentStatus::Applied;
        receipt.verification_receipt = Some(verification_receipt);
        Ok(receipt.clone())
    }

    pub fn mark_failed(
        &mut self,
        deployment_id: &str,
        reason: impl Into<String>,
    ) -> Result<CapabilityChainProofAbstractionDeploymentReceipt, CapabilityChainProofAbstractionDeploymentRejection>
    {
        let receipt = self.deployments.get_mut(deployment_id).ok_or_else(|| {
            CapabilityChainProofAbstractionDeploymentRejection::UnknownDeployment(
                deployment_id.into(),
            )
        })?;
        if receipt.status != CapabilityChainProofAbstractionDeploymentStatus::Prepared {
            return Err(
                CapabilityChainProofAbstractionDeploymentRejection::DeploymentAlreadyTerminal(
                    receipt.status,
                ),
            );
        }
        receipt.status = CapabilityChainProofAbstractionDeploymentStatus::Failed;
        receipt.failure_reason = Some(reason.into());
        Ok(receipt.clone())
    }

    pub fn rollback(
        &mut self,
        deployment_id: &str,
        reason: impl Into<String>,
    ) -> Result<CapabilityChainProofAbstractionDeploymentReceipt, CapabilityChainProofAbstractionDeploymentRejection>
    {
        let receipt = self.deployments.get_mut(deployment_id).ok_or_else(|| {
            CapabilityChainProofAbstractionDeploymentRejection::UnknownDeployment(
                deployment_id.into(),
            )
        })?;
        if receipt.status != CapabilityChainProofAbstractionDeploymentStatus::Applied {
            return Err(
                CapabilityChainProofAbstractionDeploymentRejection::RollbackRequiresApplied,
            );
        }
        receipt.status = CapabilityChainProofAbstractionDeploymentStatus::RolledBack;
        receipt.rollback_reason = Some(reason.into());
        Ok(receipt.clone())
    }

    pub fn receipt(
        &self,
        deployment_id: &str,
    ) -> Option<&CapabilityChainProofAbstractionDeploymentReceipt> {
        self.deployments.get(deployment_id)
    }

    pub fn receipts(
        &self,
    ) -> impl Iterator<Item = &CapabilityChainProofAbstractionDeploymentReceipt> {
        self.deployments.values()
    }
}

impl CapabilityChainProofAbstractionDeploymentReceipt {
    /// Materialize an applied deployment into a governed capability
    /// descriptor.  This intentionally stops before registry insertion or
    /// executor wiring; both remain explicit control-plane operations.
    pub fn materialize_capability(
        &self,
        approval: &CapabilityChainProofAbstractionApprovalReceipt,
    ) -> Result<CapabilityChainProofAbstractionCapability, CapabilityChainProofAbstractionMaterializationRejection>
    {
        if self.status != CapabilityChainProofAbstractionDeploymentStatus::Applied {
            return Err(
                CapabilityChainProofAbstractionMaterializationRejection::DeploymentNotApplied(
                    self.status,
                ),
            );
        }
        if approval.approval_id != self.approval_id
            || approval.pattern_id != self.pattern_id
        {
            return Err(
                CapabilityChainProofAbstractionMaterializationRejection::ApprovalMismatch,
            );
        }
        if approval.decision != CapabilityChainProofAbstractionApprovalDecision::Approved {
            return Err(
                CapabilityChainProofAbstractionMaterializationRejection::ApprovalNotGranted,
            );
        }
        let verification_receipt = self
            .verification_receipt
            .clone()
            .ok_or(
                CapabilityChainProofAbstractionMaterializationRejection::MissingVerificationReceipt,
            )?;
        let proposal = &approval.recommendation.proposal.pattern;
        Ok(CapabilityChainProofAbstractionCapability {
            capability_id: self.proposed_revision.clone(),
            version: 1,
            deployment_id: self.deployment_id.clone(),
            pattern_id: self.pattern_id.clone(),
            goal: proposal.goal,
            capabilities: proposal.capabilities.clone(),
            final_artifacts: proposal.final_artifacts.clone(),
            verification_receipt,
        })
    }
}

impl CapabilityChainProofAbstractionExperimentLedger {
    pub fn record(
        &mut self,
        experiment_id: impl Into<String>,
        receipt: CapabilityChainProofAbstractionExperimentReceipt,
    ) -> Result<CapabilityChainProofAbstractionExperimentReceipt, CapabilityChainProofAbstractionExperimentLedgerRejection>
    {
        let experiment_id = experiment_id.into();
        if self.receipts.contains_key(&experiment_id) {
            return Err(
                CapabilityChainProofAbstractionExperimentLedgerRejection::DuplicateExperiment(
                    experiment_id,
                ),
            );
        }
        self.receipts.insert(experiment_id, receipt.clone());
        Ok(receipt)
    }

    pub fn receipt(
        &self,
        experiment_id: &str,
    ) -> Option<&CapabilityChainProofAbstractionExperimentReceipt> {
        self.receipts.get(experiment_id)
    }

    pub fn receipts(
        &self,
    ) -> impl Iterator<Item = &CapabilityChainProofAbstractionExperimentReceipt> {
        self.receipts.values()
    }

    /// Summarize observed outcomes by declared risk class.  Counts are kept
    /// as integers so consumers can choose their own statistical policy.
    pub fn meta_learning_profile(&self) -> CapabilityChainProofAbstractionMetaLearningProfile {
        let mut summaries = BTreeMap::<u8, CapabilityChainProofAbstractionExperimentRiskSummary>::new();
        for receipt in self.receipts.values() {
            let risk = receipt.proposal.risk;
            let key = match risk {
                ImprovementRisk::Low => 0,
                ImprovementRisk::Medium => 1,
                ImprovementRisk::High => 2,
            };
            let summary = summaries.entry(key).or_insert_with(|| {
                CapabilityChainProofAbstractionExperimentRiskSummary {
                    risk,
                    attempts: 0,
                    passed: 0,
                    safety_failures: 0,
                }
            });
            summary.attempts += 1;
            summary.passed += usize::from(receipt.passed);
            summary.safety_failures += usize::from(!receipt.safety_preserved);
        }
        let risk_summaries = summaries.into_values().collect::<Vec<_>>();
        CapabilityChainProofAbstractionMetaLearningProfile {
            total_experiments: self.receipts.len(),
            total_passed: self.receipts.values().filter(|receipt| receipt.passed).count(),
            risk_summaries,
        }
    }
}

impl CapabilityRegistryEvolutionLedger {
    /// Stage an abstraction only when both held-out generalization and
    /// contract-novelty evidence pass.
    pub fn prepare_with_generalization_and_novelty(
        &mut self,
        evolution_id: impl Into<String>,
        descriptor: &CapabilityChainProofAbstractionCapability,
        candidate: CapabilitySpec,
        registry: &CapabilityRegistry,
        generalization: &CapabilityChainProofAbstractionGeneralizationReceipt,
        novelty: &CapabilityChainProofAbstractionNoveltyReceipt,
    ) -> Result<CapabilityRegistryEvolutionReceipt, CapabilityRegistryEvolutionRejection> {
        if generalization.pattern_id != descriptor.pattern_id {
            return Err(CapabilityRegistryEvolutionRejection::GeneralizationPatternMismatch);
        }
        if !generalization.passed {
            return Err(CapabilityRegistryEvolutionRejection::GeneralizationNotValidated);
        }
        if novelty.pattern_id != descriptor.pattern_id
            || novelty.candidate_id != candidate.id
        {
            return Err(CapabilityRegistryEvolutionRejection::NoveltyPatternMismatch);
        }
        if !novelty.passed {
            return Err(CapabilityRegistryEvolutionRejection::NoveltyNotEstablished(
                novelty.equivalent_existing_capabilities.clone(),
            ));
        }
        self.prepare(evolution_id, descriptor, candidate, registry)
    }

    /// Stage an executable candidate only after held-out generalization
    /// evidence has passed.  This is stricter than `prepare`, which remains
    /// available for callers migrating older governance records.
    pub fn prepare_with_generalization(
        &mut self,
        evolution_id: impl Into<String>,
        descriptor: &CapabilityChainProofAbstractionCapability,
        candidate: CapabilitySpec,
        registry: &CapabilityRegistry,
        generalization: &CapabilityChainProofAbstractionGeneralizationReceipt,
    ) -> Result<CapabilityRegistryEvolutionReceipt, CapabilityRegistryEvolutionRejection> {
        if generalization.pattern_id != descriptor.pattern_id {
            return Err(CapabilityRegistryEvolutionRejection::GeneralizationPatternMismatch);
        }
        if !generalization.passed {
            return Err(CapabilityRegistryEvolutionRejection::GeneralizationNotValidated);
        }
        self.prepare(evolution_id, descriptor, candidate, registry)
    }

    pub fn prepare(
        &mut self,
        evolution_id: impl Into<String>,
        descriptor: &CapabilityChainProofAbstractionCapability,
        candidate: CapabilitySpec,
        registry: &CapabilityRegistry,
    ) -> Result<CapabilityRegistryEvolutionReceipt, CapabilityRegistryEvolutionRejection> {
        let evolution_id = evolution_id.into();
        if self.evolutions.contains_key(&evolution_id) {
            return Err(CapabilityRegistryEvolutionRejection::DuplicateEvolution(
                evolution_id,
            ));
        }
        if candidate.id != descriptor.capability_id {
            return Err(CapabilityRegistryEvolutionRejection::CapabilityIdMismatch);
        }
        if candidate.version != descriptor.version {
            return Err(CapabilityRegistryEvolutionRejection::CapabilityVersionMismatch);
        }
        if !candidate.produces.contains(&descriptor.goal) {
            return Err(CapabilityRegistryEvolutionRejection::GoalNotProduced(
                descriptor.goal,
            ));
        }
        if candidate.executor.trim().is_empty()
            || candidate.verifier.trim().is_empty()
            || !candidate.quality_gate.enabled()
        {
            return Err(CapabilityRegistryEvolutionRejection::CandidateNotExecutable);
        }
        if registry.get(&candidate.id).is_some() {
            return Err(CapabilityRegistryEvolutionRejection::CapabilityAlreadyRegistered(
                candidate.id,
            ));
        }
        let receipt = CapabilityRegistryEvolutionReceipt {
            evolution_id: evolution_id.clone(),
            capability_id: candidate.id.clone(),
            pattern_id: descriptor.pattern_id.clone(),
            deployment_id: descriptor.deployment_id.clone(),
            candidate,
            status: CapabilityRegistryEvolutionStatus::Prepared,
            verification_receipt: None,
            failure_reason: None,
            rollback_reason: None,
        };
        self.evolutions.insert(evolution_id, receipt.clone());
        Ok(receipt)
    }

    pub fn apply(
        &mut self,
        evolution_id: &str,
        registry: &mut CapabilityRegistry,
        candidate: &CapabilitySpec,
        verification_receipt: impl Into<String>,
    ) -> Result<CapabilityRegistryEvolutionReceipt, CapabilityRegistryEvolutionRejection> {
        let receipt = self.evolutions.get_mut(evolution_id).ok_or_else(|| {
            CapabilityRegistryEvolutionRejection::UnknownEvolution(evolution_id.into())
        })?;
        if receipt.status != CapabilityRegistryEvolutionStatus::Prepared {
            return Err(CapabilityRegistryEvolutionRejection::EvolutionAlreadyTerminal(
                receipt.status,
            ));
        }
        if &receipt.candidate != candidate {
            return Err(CapabilityRegistryEvolutionRejection::CandidateMismatch);
        }
        if registry.get(&candidate.id).is_some() {
            return Err(CapabilityRegistryEvolutionRejection::CapabilityAlreadyRegistered(
                candidate.id.clone(),
            ));
        }
        let verification_receipt = verification_receipt.into();
        if verification_receipt.trim().is_empty() {
            return Err(CapabilityRegistryEvolutionRejection::MissingVerificationReceipt);
        }
        registry.register(candidate.clone());
        receipt.status = CapabilityRegistryEvolutionStatus::Applied;
        receipt.verification_receipt = Some(verification_receipt);
        Ok(receipt.clone())
    }

    pub fn mark_failed(
        &mut self,
        evolution_id: &str,
        reason: impl Into<String>,
    ) -> Result<CapabilityRegistryEvolutionReceipt, CapabilityRegistryEvolutionRejection> {
        let receipt = self.evolutions.get_mut(evolution_id).ok_or_else(|| {
            CapabilityRegistryEvolutionRejection::UnknownEvolution(evolution_id.into())
        })?;
        if receipt.status != CapabilityRegistryEvolutionStatus::Prepared {
            return Err(CapabilityRegistryEvolutionRejection::EvolutionAlreadyTerminal(
                receipt.status,
            ));
        }
        receipt.status = CapabilityRegistryEvolutionStatus::Failed;
        receipt.failure_reason = Some(reason.into());
        Ok(receipt.clone())
    }

    pub fn rollback(
        &mut self,
        evolution_id: &str,
        registry: &mut CapabilityRegistry,
        reason: impl Into<String>,
    ) -> Result<CapabilityRegistryEvolutionReceipt, CapabilityRegistryEvolutionRejection> {
        let receipt = self.evolutions.get_mut(evolution_id).ok_or_else(|| {
            CapabilityRegistryEvolutionRejection::UnknownEvolution(evolution_id.into())
        })?;
        if receipt.status != CapabilityRegistryEvolutionStatus::Applied {
            return Err(CapabilityRegistryEvolutionRejection::RollbackRequiresApplied);
        }
        if registry.get(&receipt.capability_id) != Some(&receipt.candidate) {
            return Err(CapabilityRegistryEvolutionRejection::RegistryStateMismatch);
        }
        registry.capabilities.remove(&receipt.capability_id);
        receipt.status = CapabilityRegistryEvolutionStatus::RolledBack;
        receipt.rollback_reason = Some(reason.into());
        Ok(receipt.clone())
    }

    pub fn receipt(&self, evolution_id: &str) -> Option<&CapabilityRegistryEvolutionReceipt> {
        self.evolutions.get(evolution_id)
    }

    pub fn receipts(&self) -> impl Iterator<Item = &CapabilityRegistryEvolutionReceipt> {
        self.evolutions.values()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifiedArtifact<T> {
    pub artifact: T,
    pub proof_trace: CapabilityChainProofTrace,
    pub final_verification_receipt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum VerifiedArtifactFailure {
    UnverifiedProof,
    MissingFinalVerificationReceipt,
}

impl<T> VerifiedArtifact<T> {
    /// Bind an arbitrary typed artifact to a completed proof-carrying chain.
    /// The wrapper does not execute the chain or infer that the artifact is
    /// equal to an output; callers must provide the artifact from that chain.
    pub fn from_chain(
        artifact: T,
        proof_trace: CapabilityChainProofTrace,
    ) -> Result<Self, VerifiedArtifactFailure> {
        if !proof_trace.replay_verified {
            return Err(VerifiedArtifactFailure::UnverifiedProof);
        }
        let Some(final_step) = proof_trace.steps.last() else {
            return Err(VerifiedArtifactFailure::MissingFinalVerificationReceipt);
        };
        if final_step.verification_receipt.trim().is_empty() {
            return Err(VerifiedArtifactFailure::MissingFinalVerificationReceipt);
        }
        Ok(Self {
            artifact,
            final_verification_receipt: final_step.verification_receipt.clone(),
            proof_trace,
        })
    }
}

/// Consumer-side requirements for accepting a proof-bearing artifact.
///
/// `VerifiedArtifact<T>` records that a chain produced verification evidence;
/// this policy decides whether that evidence is sufficient for a particular
/// consumer.  Keeping the decision on the consumer side prevents the wrapper
/// from being treated as universally trusted merely because it exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifiedArtifactPolicy {
    pub require_replay_verified: bool,
    pub minimum_proof_steps: usize,
    pub require_final_verification_receipt: bool,
}

impl Default for VerifiedArtifactPolicy {
    fn default() -> Self {
        Self {
            require_replay_verified: true,
            minimum_proof_steps: 1,
            require_final_verification_receipt: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum VerifiedArtifactPolicyFailure {
    ReplayNotVerified,
    InsufficientProofSteps { required: usize, actual: usize },
    MissingStepVerificationReceipt(usize),
    MissingFactRetrievalReceipt(String),
    MissingFinalVerificationReceipt,
    FinalVerificationReceiptMismatch,
    IncompleteProofTrace { expected: usize, actual: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifiedArtifactPolicyReceipt {
    pub execution_id: String,
    pub proof_steps: usize,
    pub retrieved_facts: usize,
    pub replay_verified: bool,
    pub final_verification_receipt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifiedArtifactFactBridgeReceipt {
    pub fact_id: String,
    pub execution_id: String,
    pub parent_lineage: Vec<String>,
    pub policy: VerifiedArtifactPolicyReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifiedArtifactFactPublicationReceipt {
    pub key: String,
    pub fact_id: String,
    pub bridge: VerifiedArtifactFactBridgeReceipt,
    pub index_result: FactIndexInsert,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum VerifiedArtifactFactPublicationFailure {
    Bridge(VerifiedArtifactFactBridgeFailure),
    Index(FactIndexRejection),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum VerifiedArtifactFactBridgeFailure {
    TrustPolicy(VerifiedArtifactPolicyFailure),
    Derivation(FactDerivationRejection),
}

/// Convert a policy-admitted verified artifact into a lineage-bearing fact.
/// The bridge requires parents and records the chain receipt in provenance;
/// it does not insert into a fact index or bypass its separate FactPolicy.
pub fn derive_fact_from_verified_artifact<T>(
    artifact: &VerifiedArtifact<T>,
    trust_policy: &VerifiedArtifactPolicy,
    id: impl Into<String>,
    content: impl Into<String>,
    parents: &[&DerivedFact],
    provenance: impl Into<String>,
    assumptions: &[String],
    domain: Option<String>,
) -> Result<(DerivedFact, VerifiedArtifactFactBridgeReceipt), VerifiedArtifactFactBridgeFailure> {
    let policy = trust_policy
        .evaluate(artifact)
        .map_err(VerifiedArtifactFactBridgeFailure::TrustPolicy)?;
    let id = id.into();
    let provenance = format!(
        "{}; verified_execution={}; final_verification_receipt={}",
        provenance.into(),
        policy.execution_id,
        policy.final_verification_receipt
    );
    let fact = DerivedFact::derive_from(
        id.clone(),
        content,
        parents,
        provenance,
        assumptions,
        domain,
    )
    .map_err(VerifiedArtifactFactBridgeFailure::Derivation)?;
    let receipt = VerifiedArtifactFactBridgeReceipt {
        fact_id: fact.id.clone(),
        execution_id: policy.execution_id.clone(),
        parent_lineage: fact.parent_lineage.clone(),
        policy,
    };
    Ok((fact, receipt))
}

/// Explicitly publish a verified artifact as governed knowledge.  This is a
/// convenience boundary, not an implicit memory side effect: callers choose
/// the ledger key and FactPolicy, and conflicts are returned as an indexed
/// outcome in the publication receipt.
pub fn publish_verified_artifact_fact<T>(
    artifact: &VerifiedArtifact<T>,
    trust_policy: &VerifiedArtifactPolicy,
    id: impl Into<String>,
    content: impl Into<String>,
    parents: &[&DerivedFact],
    provenance: impl Into<String>,
    assumptions: &[String],
    domain: Option<String>,
    key: impl Into<String>,
    index: &mut DerivedFactIndex,
    fact_policy: &FactPolicy,
) -> Result<VerifiedArtifactFactPublicationReceipt, VerifiedArtifactFactPublicationFailure> {
    let (fact, bridge) = derive_fact_from_verified_artifact(
        artifact,
        trust_policy,
        id,
        content,
        parents,
        provenance,
        assumptions,
        domain,
    )
    .map_err(VerifiedArtifactFactPublicationFailure::Bridge)?;
    let key = key.into();
    let fact_id = fact.id.clone();
    let index_result = index
        .insert(key.clone(), fact, fact_policy)
        .map_err(VerifiedArtifactFactPublicationFailure::Index)?;
    Ok(VerifiedArtifactFactPublicationReceipt {
        key,
        fact_id,
        bridge,
        index_result,
    })
}

impl VerifiedArtifactPolicy {
    /// Evaluate whether an artifact's proof satisfies this consumer's policy.
    /// No execution or mutation occurs; the returned receipt records the
    /// evidence inspected by the policy decision.
    pub fn evaluate<T>(
        &self,
        artifact: &VerifiedArtifact<T>,
    ) -> Result<VerifiedArtifactPolicyReceipt, VerifiedArtifactPolicyFailure> {
        let proof = &artifact.proof_trace;
        if self.require_replay_verified && !proof.replay_verified {
            return Err(VerifiedArtifactPolicyFailure::ReplayNotVerified);
        }
        let actual_steps = proof.steps.len();
        if actual_steps < self.minimum_proof_steps {
            return Err(VerifiedArtifactPolicyFailure::InsufficientProofSteps {
                required: self.minimum_proof_steps,
                actual: actual_steps,
            });
        }
        if actual_steps != proof.plan.steps.len() {
            return Err(VerifiedArtifactPolicyFailure::IncompleteProofTrace {
                expected: proof.plan.steps.len(),
                actual: actual_steps,
            });
        }
        for step in &proof.steps {
            if step.verification_receipt.trim().is_empty() {
                return Err(VerifiedArtifactPolicyFailure::MissingStepVerificationReceipt(
                    step.step_index,
                ));
            }
        }
        for fact in &proof.retrieved_facts {
            if fact
                .retrieval_receipt
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                return Err(VerifiedArtifactPolicyFailure::MissingFactRetrievalReceipt(
                    fact.fact_id.clone(),
                ));
            }
        }
        if self.require_final_verification_receipt {
            if artifact.final_verification_receipt.trim().is_empty() {
                return Err(VerifiedArtifactPolicyFailure::MissingFinalVerificationReceipt);
            }
            if proof
                .steps
                .last()
                .map(|step| step.verification_receipt.as_str())
                != Some(artifact.final_verification_receipt.as_str())
            {
                return Err(VerifiedArtifactPolicyFailure::FinalVerificationReceiptMismatch);
            }
        }
        Ok(VerifiedArtifactPolicyReceipt {
            execution_id: proof.execution_id.clone(),
            proof_steps: actual_steps,
            retrieved_facts: proof.retrieved_facts.len(),
            replay_verified: proof.replay_verified,
            final_verification_receipt: artifact.final_verification_receipt.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilityChainProofFailure {
    ExecutionNotSuccessful(CapabilityChainExecutionStatus),
    UnverifiedProofInput(String),
    IncompatibleProofs {
        produced: Vec<String>,
        required: Vec<String>,
    },
    IncompleteSteps { expected: usize, recorded: usize },
    MissingVerificationReceipt(usize),
    MissingFactRetrievalReceipt(String),
    DuplicateFactRetrieval { capability: String, fact_id: String },
}

/// Compose independently recorded step receipts into one proof artifact.
/// This does not execute or authorize the chain; it only preserves the
/// already-recorded verification lineage at chain scope.
pub fn compose_capability_chain_proof(
    execution: &CapabilityChainExecutionReceipt,
) -> Result<CapabilityChainProofTrace, CapabilityChainProofFailure> {
    compose_capability_chain_proof_with_retrieved_facts(execution, &[])
}

/// Compose execution receipts together with facts retrieved from the
/// governed index. Retrieved facts are proof inputs, not execution steps, and
/// must carry their own retrieval receipts.
pub fn compose_capability_chain_proof_with_retrieved_facts(
    execution: &CapabilityChainExecutionReceipt,
    retrieved_facts: &[DerivedFactProof],
) -> Result<CapabilityChainProofTrace, CapabilityChainProofFailure> {
    if execution.status != CapabilityChainExecutionStatus::Succeeded {
        return Err(CapabilityChainProofFailure::ExecutionNotSuccessful(
            execution.status,
        ));
    }
    if execution.steps.len() != execution.plan.steps.len() {
        return Err(CapabilityChainProofFailure::IncompleteSteps {
            expected: execution.plan.steps.len(),
            recorded: execution.steps.len(),
        });
    }
    for step in &execution.steps {
        if step.verification_receipt.trim().is_empty() {
            return Err(CapabilityChainProofFailure::MissingVerificationReceipt(
                step.step_index,
            ));
        }
    }
    let mut seen_retrievals = BTreeSet::new();
    for fact in retrieved_facts {
        if fact
            .retrieval_receipt
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            return Err(CapabilityChainProofFailure::MissingFactRetrievalReceipt(
                fact.fact_id.clone(),
            ));
        }
        let retrieval_key = (fact.capability.clone(), fact.fact_id.clone());
        if !seen_retrievals.insert(retrieval_key) {
            return Err(CapabilityChainProofFailure::DuplicateFactRetrieval {
                capability: fact.capability.clone(),
                fact_id: fact.fact_id.clone(),
            });
        }
    }
    let steps = execution
        .steps
        .iter()
        .map(|step| CapabilityChainProofStep {
            step_index: step.step_index,
            capability_id: step.capability_id.clone(),
            input_artifacts: step.input_artifacts.clone(),
            output_artifacts: step.output_artifacts.clone(),
            verification_receipt: step.verification_receipt.clone(),
        })
        .collect::<Vec<_>>();
    let final_artifacts = steps
        .last()
        .map(|step| step.output_artifacts.clone())
        .unwrap_or_default();
    let mut retrieved_facts = retrieved_facts.to_vec();
    retrieved_facts.sort_by(|left, right| {
        left.fact_id
            .cmp(&right.fact_id)
            .then_with(|| left.capability.cmp(&right.capability))
            .then_with(|| left.retrieval_receipt.cmp(&right.retrieval_receipt))
    });
    Ok(CapabilityChainProofTrace {
        execution_id: execution.execution_id.clone(),
        plan: execution.plan.clone(),
        steps,
        retrieved_facts,
        final_artifacts,
        replay_verified: true,
    })
}

/// Compose two independently verified traces when the first trace produces
/// an artifact consumed by the second. This is proof composition, not
/// execution: every input trace must already be replay-verified.
pub fn compose_capability_chain_proofs(
    first: &CapabilityChainProofTrace,
    second: &CapabilityChainProofTrace,
) -> Result<CapabilityChainProofTrace, CapabilityChainProofFailure> {
    if !first.replay_verified {
        return Err(CapabilityChainProofFailure::UnverifiedProofInput(
            first.execution_id.clone(),
        ));
    }
    if !second.replay_verified {
        return Err(CapabilityChainProofFailure::UnverifiedProofInput(
            second.execution_id.clone(),
        ));
    }
    if first.steps.len() != first.plan.steps.len() {
        return Err(CapabilityChainProofFailure::IncompleteSteps {
            expected: first.plan.steps.len(),
            recorded: first.steps.len(),
        });
    }
    if second.steps.len() != second.plan.steps.len() {
        return Err(CapabilityChainProofFailure::IncompleteSteps {
            expected: second.plan.steps.len(),
            recorded: second.steps.len(),
        });
    }
    for step in first.steps.iter().chain(second.steps.iter()) {
        if step.verification_receipt.trim().is_empty() {
            return Err(CapabilityChainProofFailure::MissingVerificationReceipt(
                step.step_index,
            ));
        }
    }
    let required = second
        .steps
        .first()
        .map(|step| step.input_artifacts.clone())
        .unwrap_or_default();
    if first.steps.is_empty() || second.steps.is_empty() {
        return Err(CapabilityChainProofFailure::IncompatibleProofs {
            produced: first.final_artifacts.clone(),
            required,
        });
    }
    if !first
        .final_artifacts
        .iter()
        .any(|artifact| required.contains(artifact))
    {
        return Err(CapabilityChainProofFailure::IncompatibleProofs {
            produced: first.final_artifacts.clone(),
            required,
        });
    }

    let mut steps = first.steps.clone();
    let offset = steps.len();
    steps.extend(second.steps.iter().map(|step| CapabilityChainProofStep {
        step_index: step.step_index + offset,
        capability_id: step.capability_id.clone(),
        input_artifacts: step.input_artifacts.clone(),
        output_artifacts: step.output_artifacts.clone(),
        verification_receipt: step.verification_receipt.clone(),
    }));
    let mut retrieved_facts = first.retrieved_facts.clone();
    retrieved_facts.extend(second.retrieved_facts.clone());
    let mut seen_retrievals = BTreeSet::new();
    for fact in &retrieved_facts {
        if fact
            .retrieval_receipt
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            return Err(CapabilityChainProofFailure::MissingFactRetrievalReceipt(
                fact.fact_id.clone(),
            ));
        }
        let key = (fact.capability.clone(), fact.fact_id.clone());
        if !seen_retrievals.insert(key) {
            return Err(CapabilityChainProofFailure::DuplicateFactRetrieval {
                capability: fact.capability.clone(),
                fact_id: fact.fact_id.clone(),
            });
        }
    }
    retrieved_facts.sort_by(|left, right| {
        left.fact_id
            .cmp(&right.fact_id)
            .then_with(|| left.capability.cmp(&right.capability))
            .then_with(|| left.retrieval_receipt.cmp(&right.retrieval_receipt))
    });
    let mut plan_steps = first.plan.steps.clone();
    plan_steps.extend(second.plan.steps.clone());
    Ok(CapabilityChainProofTrace {
        execution_id: format!("compose:{}+{}", first.execution_id, second.execution_id),
        plan: CapabilityChainPlan {
            goal: second.plan.goal,
            steps: plan_steps,
        },
        steps,
        retrieved_facts,
        final_artifacts: second.final_artifacts.clone(),
        replay_verified: true,
    })
}

/// Validate execution of a selected mixed synthesis draft and materialize a
/// trusted artifact only after prefix composition and consumer policy checks
/// succeed. The execution receipt must match the pending plan exactly.
pub fn validate_mixed_synthesis_execution<T>(
    source: &CapabilityChainProofSynthesisSource,
    execution: &CapabilityChainExecutionReceipt,
    artifact: T,
    policy: &VerifiedArtifactPolicy,
) -> Result<CapabilityChainProofSynthesisValidationReceipt<T>, CapabilityChainProofSynthesisValidationFailure>
{
    let CapabilityChainProofSynthesisSource::MixedPrefixPlanPending {
        prefix,
        handoff_artifacts,
        plan,
    } = source
    else {
        return Err(CapabilityChainProofSynthesisValidationFailure::NotMixedSynthesis);
    };
    if execution.plan != *plan {
        return Err(CapabilityChainProofSynthesisValidationFailure::PlanMismatch);
    }
    let recorded_inputs = execution
        .steps
        .first()
        .map(|step| &step.input_artifacts)
        .cloned()
        .unwrap_or_default();
    if !handoff_artifacts
        .iter()
        .any(|artifact| recorded_inputs.contains(artifact))
    {
        return Err(
            CapabilityChainProofSynthesisValidationFailure::HandoffNotRecorded(
                handoff_artifacts.clone(),
            ),
        );
    }
    let continuation = compose_capability_chain_proof(execution)
        .map_err(CapabilityChainProofSynthesisValidationFailure::Composition)?;
    let composite = compose_capability_chain_proofs(prefix, &continuation)
        .map_err(CapabilityChainProofSynthesisValidationFailure::Composition)?;
    let verified = VerifiedArtifact::from_chain(artifact, composite)
        .map_err(CapabilityChainProofSynthesisValidationFailure::Artifact)?;
    let policy_receipt = policy
        .evaluate(&verified)
        .map_err(CapabilityChainProofSynthesisValidationFailure::Policy)?;
    Ok(CapabilityChainProofSynthesisValidationReceipt {
        artifact: verified,
        prefix_fingerprint: prefix.reasoning_fingerprint(),
        execution_id: execution.execution_id.clone(),
        handoff_artifacts: handoff_artifacts.clone(),
        policy: policy_receipt,
    })
}

/// Publish a validated mixed synthesis result into both proof and fact
/// memory atomically. Staging on cloned indexes prevents one store from
/// observing a new entry when the other store rejects it.
pub fn publish_validated_mixed_synthesis<T>(
    validation: &CapabilityChainProofSynthesisValidationReceipt<T>,
    trust_policy: &VerifiedArtifactPolicy,
    fact_id: impl Into<String>,
    content: impl Into<String>,
    parents: &[&DerivedFact],
    provenance: impl Into<String>,
    assumptions: &[String],
    domain: Option<String>,
    fact_key: impl Into<String>,
    proof_index: &mut CapabilityChainProofIndex,
    fact_index: &mut DerivedFactIndex,
    fact_policy: &FactPolicy,
) -> Result<CapabilityChainProofSynthesisPublicationReceipt, CapabilityChainProofSynthesisPublicationFailure>
{
    let (fact, bridge) = derive_fact_from_verified_artifact(
        &validation.artifact,
        trust_policy,
        fact_id,
        content,
        parents,
        provenance,
        assumptions,
        domain,
    )
    .map_err(CapabilityChainProofSynthesisPublicationFailure::FactBridge)?;
    let fact_key = fact_key.into();
    let mut staged_proofs = proof_index.clone();
    let proof_fingerprint = staged_proofs
        .insert(validation.artifact.proof_trace.clone())
        .map_err(CapabilityChainProofSynthesisPublicationFailure::ProofIndex)?;
    let mut staged_facts = fact_index.clone();
    let fact_id = fact.id.clone();
    let index_result = staged_facts
        .insert(fact_key.clone(), fact, fact_policy)
        .map_err(CapabilityChainProofSynthesisPublicationFailure::FactIndex)?;
    *proof_index = staged_proofs;
    *fact_index = staged_facts;
    Ok(CapabilityChainProofSynthesisPublicationReceipt {
        proof_fingerprint,
        fact_publication: VerifiedArtifactFactPublicationReceipt {
            key: fact_key,
            fact_id,
            bridge,
            index_result,
        },
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CapabilityChainExecutionRejection {
    DuplicateExecution(String),
    UnknownExecution(String),
    ExecutionAlreadyTerminal(CapabilityChainExecutionStatus),
    WrongStepIndex { expected: usize, actual: usize },
    WrongCapability { expected: String, actual: String },
    MissingVerificationReceipt,
    IncompleteChain { expected: usize, recorded: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct CapabilityChainExecutionLedger {
    executions: BTreeMap<String, CapabilityChainExecutionReceipt>,
}

impl CapabilityChainExecutionLedger {
    pub fn start(
        &mut self,
        execution_id: impl Into<String>,
        plan: CapabilityChainPlan,
    ) -> Result<CapabilityChainExecutionReceipt, CapabilityChainExecutionRejection> {
        let execution_id = execution_id.into();
        if self.executions.contains_key(&execution_id) {
            return Err(CapabilityChainExecutionRejection::DuplicateExecution(
                execution_id,
            ));
        }
        let receipt = CapabilityChainExecutionReceipt {
            execution_id: execution_id.clone(),
            plan,
            status: CapabilityChainExecutionStatus::Running,
            steps: Vec::new(),
            failed_step: None,
            failure_reason: None,
        };
        self.executions.insert(execution_id, receipt.clone());
        Ok(receipt)
    }

    pub fn record_step(
        &mut self,
        execution_id: &str,
        step: CapabilityChainStepReceipt,
    ) -> Result<CapabilityChainExecutionReceipt, CapabilityChainExecutionRejection> {
        let receipt = self
            .executions
            .get_mut(execution_id)
            .ok_or_else(|| CapabilityChainExecutionRejection::UnknownExecution(execution_id.into()))?;
        if receipt.status != CapabilityChainExecutionStatus::Running {
            return Err(CapabilityChainExecutionRejection::ExecutionAlreadyTerminal(
                receipt.status,
            ));
        }
        let expected_index = receipt.steps.len();
        if step.step_index != expected_index {
            return Err(CapabilityChainExecutionRejection::WrongStepIndex {
                expected: expected_index,
                actual: step.step_index,
            });
        }
        if expected_index >= receipt.plan.steps.len() {
            return Err(CapabilityChainExecutionRejection::IncompleteChain {
                expected: receipt.plan.steps.len(),
                recorded: receipt.steps.len(),
            });
        }
        let expected_capability = receipt.plan.steps[expected_index].clone();
        if step.capability_id != expected_capability {
            return Err(CapabilityChainExecutionRejection::WrongCapability {
                expected: expected_capability,
                actual: step.capability_id,
            });
        }
        if step.verification_receipt.trim().is_empty() {
            return Err(CapabilityChainExecutionRejection::MissingVerificationReceipt);
        }
        receipt.steps.push(step);
        Ok(receipt.clone())
    }

    pub fn complete_success(
        &mut self,
        execution_id: &str,
    ) -> Result<CapabilityChainExecutionReceipt, CapabilityChainExecutionRejection> {
        let receipt = self
            .executions
            .get_mut(execution_id)
            .ok_or_else(|| CapabilityChainExecutionRejection::UnknownExecution(execution_id.into()))?;
        if receipt.status != CapabilityChainExecutionStatus::Running {
            return Err(CapabilityChainExecutionRejection::ExecutionAlreadyTerminal(
                receipt.status,
            ));
        }
        if receipt.steps.len() != receipt.plan.steps.len() {
            return Err(CapabilityChainExecutionRejection::IncompleteChain {
                expected: receipt.plan.steps.len(),
                recorded: receipt.steps.len(),
            });
        }
        receipt.status = CapabilityChainExecutionStatus::Succeeded;
        Ok(receipt.clone())
    }

    pub fn complete_failure(
        &mut self,
        execution_id: &str,
        failed_step: usize,
        reason: impl Into<String>,
    ) -> Result<CapabilityChainExecutionReceipt, CapabilityChainExecutionRejection> {
        let receipt = self
            .executions
            .get_mut(execution_id)
            .ok_or_else(|| CapabilityChainExecutionRejection::UnknownExecution(execution_id.into()))?;
        if receipt.status != CapabilityChainExecutionStatus::Running {
            return Err(CapabilityChainExecutionRejection::ExecutionAlreadyTerminal(
                receipt.status,
            ));
        }
        receipt.status = CapabilityChainExecutionStatus::Failed;
        receipt.failed_step = Some(failed_step);
        receipt.failure_reason = Some(reason.into());
        Ok(receipt.clone())
    }

    pub fn receipt(&self, execution_id: &str) -> Option<&CapabilityChainExecutionReceipt> {
        self.executions.get(execution_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DerivedFactRejection {
    pub fact_id: String,
    pub reason: FactPolicyRejection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ModelPlanningFailure {
    NoEligibleModel,
    AmbiguousModels(Vec<String>),
    MissingModelEntry(String),
    CapabilityPlanning(CapabilityPlanningFailure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct PlanCost {
    pub steps: usize,
    pub dependency_edges: usize,
    pub verification_steps: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PlanSelectionReason {
    UniqueTargetCapability,
    UniqueGoalProducer,
    UniqueModelThenGoalProducer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DependencyProof {
    pub capability: String,
    pub dependency: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InputProof {
    pub capability: String,
    pub input: CapabilityIoType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DerivedFactProof {
    pub capability: String,
    pub fact_id: String,
    pub parent_lineage: Vec<String>,
    pub retrieval_receipt: Option<String>,
}

/// A fact issue that prevents a previously constructed plan from remaining
/// executable. Missing facts are reported separately from inactive facts so
/// a stale plan cannot be mistaken for one whose dependencies are not loaded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum PlanFactIssue {
    Missing,
    Inactive(FactStatus),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanFactInvalidation {
    pub fact_id: String,
    pub issue: PlanFactIssue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PlanStatus {
    Active,
    Stale,
}

/// Dynamic lifecycle view for a plan. The original plan proof is retained;
/// this view is recomputed against the current fact ledger and never triggers
/// implicit execution or replacement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanLifecycle {
    pub status: PlanStatus,
    pub invalidations: Vec<PlanFactInvalidation>,
}

impl PlanLifecycle {
    pub fn is_active(&self) -> bool {
        self.status == PlanStatus::Active
    }
}

/// Inputs available to goal-directed planning.  Derived facts are kept
/// separate from model evidence and are admitted only through a capability's
/// declared lineage policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct ReasoningContext {
    pub available_inputs: BTreeSet<CapabilityIoType>,
    pub derived_facts: Vec<DerivedFact>,
    pub fact_retrieval_receipts: BTreeMap<String, String>,
}

impl ReasoningContext {
    pub fn new(available_inputs: BTreeSet<CapabilityIoType>) -> Self {
        Self {
            available_inputs,
            derived_facts: Vec::new(),
            fact_retrieval_receipts: BTreeMap::new(),
        }
    }

    pub fn with_derived_facts(
        available_inputs: BTreeSet<CapabilityIoType>,
        derived_facts: Vec<DerivedFact>,
    ) -> Self {
        Self {
            available_inputs,
            derived_facts,
            fact_retrieval_receipts: BTreeMap::new(),
        }
    }

    /// Build a planning context from the active, internally consistent facts
    /// in the ledger. Inactive facts are omitted; conflicts are surfaced so a
    /// planner cannot silently choose one side of a contradiction.
    pub fn try_from_fact_index(
        available_inputs: BTreeSet<CapabilityIoType>,
        index: &DerivedFactIndex,
    ) -> Result<Self, FactIndexQueryFailure> {
        let mut derived_facts = Vec::new();
        let mut fact_retrieval_receipts = BTreeMap::new();
        for key in index.keys() {
            match index.usable(key) {
                Ok(facts) => {
                    for fact in facts {
                        fact_retrieval_receipts.insert(
                            fact.id.clone(),
                            format!("fact_index_retrieval:{key}:{}", fact.id),
                        );
                        derived_facts.push(fact.clone());
                    }
                }
                Err(FactIndexQueryFailure::Conflict(conflict)) => {
                    return Err(FactIndexQueryFailure::Conflict(conflict));
                }
                Err(FactIndexQueryFailure::Unavailable { .. }) => {}
            }
        }
        Ok(Self {
            available_inputs,
            derived_facts,
            fact_retrieval_receipts,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityPlanStep {
    pub capability_id: String,
    pub version: u32,
    pub executor: String,
    pub verifier: String,
    pub consumes: Vec<CapabilityIoType>,
    pub produces: Vec<CapabilityIoType>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityPlan {
    pub operation: OperationKind,
    pub subject_type: SubjectObjectType,
    pub answer_form: Option<AnswerForm>,
    pub selected_capability: String,
    pub steps: Vec<CapabilityPlanStep>,
    pub cost: PlanCost,
    pub selection_reason: PlanSelectionReason,
    pub dependency_proofs: Vec<DependencyProof>,
    pub input_proofs: Vec<InputProof>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GoalCapabilityPlan {
    pub goal: CapabilityIoType,
    pub available_inputs: Vec<CapabilityIoType>,
    pub selected_capability: String,
    pub steps: Vec<CapabilityPlanStep>,
    pub cost: PlanCost,
    pub selection_reason: PlanSelectionReason,
    pub dependency_proofs: Vec<DependencyProof>,
    pub input_proofs: Vec<InputProof>,
    pub derived_fact_proofs: Vec<DerivedFactProof>,
}

impl GoalCapabilityPlan {
    /// Re-evaluate plan usability against the current fact lifecycle ledger.
    pub fn lifecycle(&self, index: &DerivedFactIndex) -> PlanLifecycle {
        plan_lifecycle(&self.derived_fact_proofs, index)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelPlanStep {
    pub model_id: String,
    pub version: u32,
    pub model_artifacts: Vec<ModelArtifactType>,
    pub downstream_artifacts: Vec<CapabilityIoType>,
}

/// A shadow-planning receipt for the first model-to-transformation bridge.
/// Model construction remains a separate authorization boundary; this type
/// only proves that a uniquely selected model declares enough typed outputs
/// for a uniquely selected transformation plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelCapabilityPlan {
    pub goal: CapabilityIoType,
    pub model_step: ModelPlanStep,
    pub capability_plan: GoalCapabilityPlan,
    pub cost: PlanCost,
    pub selection_reason: PlanSelectionReason,
}

impl ModelCapabilityPlan {
    pub fn lifecycle(&self, index: &DerivedFactIndex) -> PlanLifecycle {
        self.capability_plan.lifecycle(index)
    }
}

/// Auditable reverse index from fact dependencies to plans. Registration is
/// explicit: the index never discovers or invents plan dependencies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct PlanDependencyIndex {
    plan_facts: BTreeMap<String, BTreeSet<String>>,
    fact_plans: BTreeMap<String, BTreeSet<String>>,
    replacement_history: Vec<PlanReplacementReceipt>,
}

impl PlanDependencyIndex {
    pub fn register(&mut self, plan_id: impl Into<String>, plan: &GoalCapabilityPlan) {
        let plan_id = plan_id.into();
        self.unregister(&plan_id);
        let fact_ids = plan
            .derived_fact_proofs
            .iter()
            .map(|proof| proof.fact_id.clone())
            .collect::<BTreeSet<_>>();
        for fact_id in &fact_ids {
            self.fact_plans
                .entry(fact_id.clone())
                .or_default()
                .insert(plan_id.clone());
        }
        self.plan_facts.insert(plan_id, fact_ids);
    }

    pub fn unregister(&mut self, plan_id: &str) {
        let Some(fact_ids) = self.plan_facts.remove(plan_id) else {
            return;
        };
        for fact_id in fact_ids {
            if let Some(plans) = self.fact_plans.get_mut(&fact_id) {
                plans.remove(plan_id);
                if plans.is_empty() {
                    self.fact_plans.remove(&fact_id);
                }
            }
        }
    }

    pub fn facts_for_plan(&self, plan_id: &str) -> Vec<String> {
        self.plan_facts
            .get(plan_id)
            .map(|facts| facts.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn plans_depending_on(&self, fact_id: &str) -> Vec<String> {
        self.fact_plans
            .get(fact_id)
            .map(|plans| plans.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn lifecycle(&self, plan_id: &str, index: &DerivedFactIndex) -> Option<PlanLifecycle> {
        self.plan_facts
            .get(plan_id)
            .map(|fact_ids| plan_lifecycle_for_ids(fact_ids, index))
    }

    pub fn stale_plans(&self, index: &DerivedFactIndex) -> Vec<(String, PlanLifecycle)> {
        self.plan_facts
            .keys()
            .filter_map(|plan_id| {
                let lifecycle = self.lifecycle(plan_id, index)?;
                (lifecycle.status == PlanStatus::Stale)
                    .then(|| (plan_id.clone(), lifecycle))
            })
            .collect()
    }

    pub fn replacement_history(&self) -> &[PlanReplacementReceipt] {
        &self.replacement_history
    }

    /// Install an already accepted repair into the dependency index. All
    /// validation happens before the old mapping is removed; this operation
    /// changes plan bookkeeping only and never executes a capability.
    pub fn install_repair(
        &mut self,
        plan_id: &str,
        old_plan: &GoalCapabilityPlan,
        candidate: &PlanRepairCandidate,
        decision: &PlanRepairDecision,
        fact_index: &DerivedFactIndex,
    ) -> Result<PlanReplacementReceipt, PlanReplacementFailure> {
        if candidate.plan_id != plan_id || decision.evaluation.plan_id != plan_id {
            return Err(PlanReplacementFailure::PlanIdMismatch {
                expected: plan_id.to_string(),
                candidate: candidate.plan_id.clone(),
            });
        }
        let stale_plan = old_plan.lifecycle(fact_index);
        if stale_plan.is_active() {
            return Err(PlanReplacementFailure::PlanStillActive(stale_plan));
        }
        if !decision.is_accepted() {
            return Err(PlanReplacementFailure::DecisionRejected(
                decision.rejections.clone(),
            ));
        }
        let replacement_lifecycle = candidate.replacement.lifecycle(fact_index);
        if !replacement_lifecycle.is_active() {
            return Err(PlanReplacementFailure::ReplacementStillStale(
                replacement_lifecycle,
            ));
        }
        let old_fact_ids = self.facts_for_plan(plan_id);
        let replacement_fact_ids = candidate
            .replacement
            .derived_fact_proofs
            .iter()
            .map(|proof| proof.fact_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let receipt = PlanReplacementReceipt {
            plan_id: plan_id.to_string(),
            stale_plan,
            old_fact_ids,
            replacement_fact_ids,
            evaluation: decision.evaluation.clone(),
        };
        self.unregister(plan_id);
        self.register(plan_id, &candidate.replacement);
        self.replacement_history.push(receipt.clone());
        Ok(receipt)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum PlanRepairFailure {
    PlanStillActive,
    Planning(CapabilityPlanningFailure),
    ReplacementStillStale(PlanLifecycle),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum PlanReplacementFailure {
    PlanIdMismatch { expected: String, candidate: String },
    PlanStillActive(PlanLifecycle),
    DecisionRejected(Vec<RepairDecisionRejection>),
    ReplacementStillStale(PlanLifecycle),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanReplacementReceipt {
    pub plan_id: String,
    pub stale_plan: PlanLifecycle,
    pub old_fact_ids: Vec<String>,
    pub replacement_fact_ids: Vec<String>,
    pub evaluation: PlanRepairEvaluation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PlanExecutionStatus {
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum ExecutionFailureKind {
    CapabilityUnavailable,
    InputRejected,
    VerificationFailed,
    RuntimeFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExecutionFailureDiagnosis {
    pub attempt_id: String,
    pub plan_id: String,
    pub failed_step: String,
    pub kind: ExecutionFailureKind,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExecutionFailurePattern {
    pub plan_id: String,
    pub failed_step: String,
    pub kind: ExecutionFailureKind,
    pub occurrences: usize,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ImprovementArea {
    CapabilityCoverage,
    InputFormalization,
    Verification,
    RuntimeReliability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExecutionImprovementProposal {
    pub pattern: ExecutionFailurePattern,
    pub area: ImprovementArea,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ImprovementRisk {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImprovementEvaluation {
    pub proposal: ExecutionImprovementProposal,
    pub expected_effect: String,
    pub risk: ImprovementRisk,
    pub validation_requirements: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImprovementExperimentSpec {
    pub evaluation: ImprovementEvaluation,
    pub baseline_failure_occurrences: usize,
    pub baseline_false_authorizations: usize,
    pub require_no_new_false_authorizations: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImprovementExperimentResult {
    pub proposal: ExecutionImprovementProposal,
    pub baseline_failure_occurrences: usize,
    pub post_failure_occurrences: usize,
    pub baseline_false_authorizations: usize,
    pub post_false_authorizations: usize,
    pub failure_reduced: bool,
    pub safety_preserved: bool,
    pub passed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ImprovementExperimentDecision {
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImprovementExperimentReceipt {
    pub experiment_id: String,
    pub spec: ImprovementExperimentSpec,
    pub result: ImprovementExperimentResult,
    pub decision: ImprovementExperimentDecision,
}

/// The bounded next step after an experiment has been recorded.
///
/// Recommendations are diagnostic/review artifacts only.  In particular,
/// `ReviewForApproval` does not install or apply an improvement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ImprovementRecommendationAction {
    ReviewForApproval,
    GatherMoreEvidence,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImprovementRecommendation {
    pub experiment_id: String,
    pub proposal: ExecutionImprovementProposal,
    pub experiment_decision: ImprovementExperimentDecision,
    pub action: ImprovementRecommendationAction,
    pub rationale: String,
}

impl ImprovementExperimentReceipt {
    /// Convert an immutable experiment result into an auditable next-step
    /// recommendation.  This never changes runtime behavior or applies a
    /// proposal.
    pub fn recommendation(&self) -> ImprovementRecommendation {
        let (action, rationale) = if self.result.passed {
            (
                ImprovementRecommendationAction::ReviewForApproval,
                "experiment reduced the target failure pattern while preserving the required safety invariant".into(),
            )
        } else if !self.result.safety_preserved {
            (
                ImprovementRecommendationAction::Reject,
                "experiment introduced a new false authorization or otherwise failed the safety invariant".into(),
            )
        } else {
            (
                ImprovementRecommendationAction::GatherMoreEvidence,
                "experiment preserved safety but did not demonstrate a reduction in the target failure pattern".into(),
            )
        };
        ImprovementRecommendation {
            experiment_id: self.experiment_id.clone(),
            proposal: self.spec.evaluation.proposal.clone(),
            experiment_decision: self.decision,
            action,
            rationale,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ImprovementApprovalDecision {
    Approved,
    Deferred,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImprovementApprovalReceipt {
    pub approval_id: String,
    pub experiment_id: String,
    pub recommendation: ImprovementRecommendation,
    pub decision: ImprovementApprovalDecision,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ImprovementApprovalLedgerRejection {
    DuplicateApproval(String),
    UnknownExperiment(String),
    RecommendationNotReviewable {
        experiment_id: String,
        action: ImprovementRecommendationAction,
    },
}

/// Records an explicit review decision without applying any change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct ImprovementApprovalLedger {
    approvals: BTreeMap<String, ImprovementApprovalReceipt>,
}

impl ImprovementApprovalLedger {
    pub fn record(
        &mut self,
        approval_id: impl Into<String>,
        experiments: &ImprovementExperimentLedger,
        experiment_id: &str,
        decision: ImprovementApprovalDecision,
        rationale: impl Into<String>,
    ) -> Result<ImprovementApprovalReceipt, ImprovementApprovalLedgerRejection> {
        let approval_id = approval_id.into();
        if self.approvals.contains_key(&approval_id) {
            return Err(ImprovementApprovalLedgerRejection::DuplicateApproval(
                approval_id,
            ));
        }
        let recommendation = experiments
            .recommendation(experiment_id)
            .ok_or_else(|| ImprovementApprovalLedgerRejection::UnknownExperiment(
                experiment_id.into(),
            ))?;
        if decision == ImprovementApprovalDecision::Approved
            && recommendation.action != ImprovementRecommendationAction::ReviewForApproval
        {
            return Err(
                ImprovementApprovalLedgerRejection::RecommendationNotReviewable {
                    experiment_id: experiment_id.into(),
                    action: recommendation.action,
                },
            );
        }
        let receipt = ImprovementApprovalReceipt {
            approval_id: approval_id.clone(),
            experiment_id: experiment_id.into(),
            recommendation,
            decision,
            rationale: rationale.into(),
        };
        self.approvals.insert(approval_id, receipt.clone());
        Ok(receipt)
    }

    pub fn receipt(&self, approval_id: &str) -> Option<&ImprovementApprovalReceipt> {
        self.approvals.get(approval_id)
    }

    pub fn receipts(&self) -> impl Iterator<Item = &ImprovementApprovalReceipt> {
        self.approvals.values()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ImprovementDeploymentStatus {
    Prepared,
    Applied,
    Failed,
    RolledBack,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImprovementDeploymentReceipt {
    pub deployment_id: String,
    pub approval_id: String,
    pub experiment_id: String,
    pub previous_revision: String,
    pub proposed_revision: String,
    pub status: ImprovementDeploymentStatus,
    pub verification_receipt: Option<String>,
    pub failure_reason: Option<String>,
    pub rollback_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ImprovementDeploymentLedgerRejection {
    DuplicateDeployment(String),
    UnknownApproval(String),
    ApprovalNotGranted(String),
    UnknownDeployment(String),
    DeploymentAlreadyTerminal(ImprovementDeploymentStatus),
    MissingVerificationReceipt,
    RollbackRequiresApplied,
}

/// A transactional deployment ledger.  It records the lifecycle of an
/// approved change but deliberately does not mutate the running system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct ImprovementDeploymentLedger {
    deployments: BTreeMap<String, ImprovementDeploymentReceipt>,
}

impl ImprovementDeploymentLedger {
    pub fn prepare(
        &mut self,
        deployment_id: impl Into<String>,
        approvals: &ImprovementApprovalLedger,
        approval_id: &str,
        previous_revision: impl Into<String>,
        proposed_revision: impl Into<String>,
    ) -> Result<ImprovementDeploymentReceipt, ImprovementDeploymentLedgerRejection> {
        let deployment_id = deployment_id.into();
        if self.deployments.contains_key(&deployment_id) {
            return Err(ImprovementDeploymentLedgerRejection::DuplicateDeployment(
                deployment_id,
            ));
        }
        let approval = approvals
            .receipt(approval_id)
            .ok_or_else(|| ImprovementDeploymentLedgerRejection::UnknownApproval(approval_id.into()))?;
        if approval.decision != ImprovementApprovalDecision::Approved {
            return Err(ImprovementDeploymentLedgerRejection::ApprovalNotGranted(
                approval_id.into(),
            ));
        }
        let receipt = ImprovementDeploymentReceipt {
            deployment_id: deployment_id.clone(),
            approval_id: approval_id.into(),
            experiment_id: approval.experiment_id.clone(),
            previous_revision: previous_revision.into(),
            proposed_revision: proposed_revision.into(),
            status: ImprovementDeploymentStatus::Prepared,
            verification_receipt: None,
            failure_reason: None,
            rollback_reason: None,
        };
        self.deployments.insert(deployment_id, receipt.clone());
        Ok(receipt)
    }

    pub fn mark_applied(
        &mut self,
        deployment_id: &str,
        verification_receipt: impl Into<String>,
    ) -> Result<ImprovementDeploymentReceipt, ImprovementDeploymentLedgerRejection> {
        let receipt = self
            .deployments
            .get_mut(deployment_id)
            .ok_or_else(|| ImprovementDeploymentLedgerRejection::UnknownDeployment(deployment_id.into()))?;
        if receipt.status != ImprovementDeploymentStatus::Prepared {
            return Err(ImprovementDeploymentLedgerRejection::DeploymentAlreadyTerminal(
                receipt.status,
            ));
        }
        let verification_receipt = verification_receipt.into();
        if verification_receipt.trim().is_empty() {
            return Err(ImprovementDeploymentLedgerRejection::MissingVerificationReceipt);
        }
        receipt.status = ImprovementDeploymentStatus::Applied;
        receipt.verification_receipt = Some(verification_receipt);
        Ok(receipt.clone())
    }

    pub fn mark_failed(
        &mut self,
        deployment_id: &str,
        reason: impl Into<String>,
    ) -> Result<ImprovementDeploymentReceipt, ImprovementDeploymentLedgerRejection> {
        let receipt = self
            .deployments
            .get_mut(deployment_id)
            .ok_or_else(|| ImprovementDeploymentLedgerRejection::UnknownDeployment(deployment_id.into()))?;
        if receipt.status != ImprovementDeploymentStatus::Prepared {
            return Err(ImprovementDeploymentLedgerRejection::DeploymentAlreadyTerminal(
                receipt.status,
            ));
        }
        receipt.status = ImprovementDeploymentStatus::Failed;
        receipt.failure_reason = Some(reason.into());
        Ok(receipt.clone())
    }

    pub fn rollback(
        &mut self,
        deployment_id: &str,
        reason: impl Into<String>,
    ) -> Result<ImprovementDeploymentReceipt, ImprovementDeploymentLedgerRejection> {
        let receipt = self
            .deployments
            .get_mut(deployment_id)
            .ok_or_else(|| ImprovementDeploymentLedgerRejection::UnknownDeployment(deployment_id.into()))?;
        if receipt.status != ImprovementDeploymentStatus::Applied {
            return Err(ImprovementDeploymentLedgerRejection::RollbackRequiresApplied);
        }
        receipt.status = ImprovementDeploymentStatus::RolledBack;
        receipt.rollback_reason = Some(reason.into());
        Ok(receipt.clone())
    }

    pub fn receipt(&self, deployment_id: &str) -> Option<&ImprovementDeploymentReceipt> {
        self.deployments.get(deployment_id)
    }

    pub fn receipts(&self) -> impl Iterator<Item = &ImprovementDeploymentReceipt> {
        self.deployments.values()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ImprovementExperimentLedgerRejection {
    DuplicateExperiment(String),
    ProposalMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct ImprovementExperimentLedger {
    receipts: BTreeMap<String, ImprovementExperimentReceipt>,
}

impl ImprovementExperimentLedger {
    pub fn record(
        &mut self,
        experiment_id: impl Into<String>,
        spec: ImprovementExperimentSpec,
        result: ImprovementExperimentResult,
    ) -> Result<ImprovementExperimentReceipt, ImprovementExperimentLedgerRejection> {
        let experiment_id = experiment_id.into();
        if self.receipts.contains_key(&experiment_id) {
            return Err(ImprovementExperimentLedgerRejection::DuplicateExperiment(
                experiment_id,
            ));
        }
        if spec.evaluation.proposal != result.proposal {
            return Err(ImprovementExperimentLedgerRejection::ProposalMismatch);
        }
        let decision = if result.passed {
            ImprovementExperimentDecision::Passed
        } else {
            ImprovementExperimentDecision::Failed
        };
        let receipt = ImprovementExperimentReceipt {
            experiment_id: experiment_id.clone(),
            spec,
            result,
            decision,
        };
        self.receipts.insert(experiment_id, receipt.clone());
        Ok(receipt)
    }

    pub fn receipt(&self, experiment_id: &str) -> Option<&ImprovementExperimentReceipt> {
        self.receipts.get(experiment_id)
    }

    pub fn receipts(&self) -> impl Iterator<Item = &ImprovementExperimentReceipt> {
        self.receipts.values()
    }

    pub fn recommendation(&self, experiment_id: &str) -> Option<ImprovementRecommendation> {
        self.receipts
            .get(experiment_id)
            .map(ImprovementExperimentReceipt::recommendation)
    }

    pub fn recommendations(&self) -> impl Iterator<Item = ImprovementRecommendation> + '_ {
        self.receipts
            .values()
            .map(ImprovementExperimentReceipt::recommendation)
    }
}

impl ImprovementExperimentSpec {
    pub fn assess(
        &self,
        post_failure_occurrences: usize,
        post_false_authorizations: usize,
    ) -> ImprovementExperimentResult {
        let failure_reduced = post_failure_occurrences < self.baseline_failure_occurrences;
        let safety_preserved = !self.require_no_new_false_authorizations
            || post_false_authorizations <= self.baseline_false_authorizations;
        ImprovementExperimentResult {
            proposal: self.evaluation.proposal.clone(),
            baseline_failure_occurrences: self.baseline_failure_occurrences,
            post_failure_occurrences,
            baseline_false_authorizations: self.baseline_false_authorizations,
            post_false_authorizations,
            failure_reduced,
            safety_preserved,
            passed: failure_reduced && safety_preserved,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImprovementEvaluationPolicy {
    pub minimum_occurrences: usize,
}

impl ImprovementEvaluationPolicy {
    pub fn strict() -> Self {
        Self {
            minimum_occurrences: 2,
        }
    }

    pub fn evaluate(
        &self,
        proposal: &ExecutionImprovementProposal,
    ) -> Result<ImprovementEvaluation, ImprovementEvaluationRejection> {
        if proposal.pattern.occurrences < self.minimum_occurrences {
            return Err(ImprovementEvaluationRejection::InsufficientRecurrence {
                observed: proposal.pattern.occurrences,
                required: self.minimum_occurrences,
            });
        }
        let (expected_effect, risk, validation_requirements) = match proposal.area {
            ImprovementArea::CapabilityCoverage => (
                "increase reachable verified operations without weakening authorization".into(),
                ImprovementRisk::High,
                vec![
                    "add positive and negative regression cases".into(),
                    "preserve zero false-authorizations invariant".into(),
                    "require independent replay verification".into(),
                ],
            ),
            ImprovementArea::InputFormalization => (
                "improve grounded inputs while preserving explicit evidence boundaries".into(),
                ImprovementRisk::High,
                vec![
                    "add representative extraction cases".into(),
                    "test ambiguity and missing-evidence abstention".into(),
                    "compare authorization regressions".into(),
                ],
            ),
            ImprovementArea::Verification => (
                "increase independent verification coverage".into(),
                ImprovementRisk::High,
                vec![
                    "add replay and negative verification tests".into(),
                    "keep execution success gated on a verification receipt".into(),
                ],
            ),
            ImprovementArea::RuntimeReliability => (
                "improve reproducibility and failure handling without changing semantics".into(),
                ImprovementRisk::Medium,
                vec![
                    "reproduce the failure deterministically".into(),
                    "run capability regression tests before and after the change".into(),
                ],
            ),
        };
        Ok(ImprovementEvaluation {
            proposal: proposal.clone(),
            expected_effect,
            risk,
            validation_requirements,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ImprovementEvaluationRejection {
    InsufficientRecurrence { observed: usize, required: usize },
}

impl ExecutionFailurePattern {
    /// Convert a recurring operational pattern into an explicit review item.
    /// This is advisory process knowledge, never an automatic policy update.
    pub fn improvement_proposal(&self) -> ExecutionImprovementProposal {
        let (area, rationale) = match self.kind {
            ExecutionFailureKind::CapabilityUnavailable => (
                ImprovementArea::CapabilityCoverage,
                "review whether a verified capability is missing for this step".into(),
            ),
            ExecutionFailureKind::InputRejected => (
                ImprovementArea::InputFormalization,
                "inspect evidence, bindings, and input-grounding requirements".into(),
            ),
            ExecutionFailureKind::VerificationFailed => (
                ImprovementArea::Verification,
                "inspect executor output and its independent verifier".into(),
            ),
            ExecutionFailureKind::RuntimeFailure => (
                ImprovementArea::RuntimeReliability,
                "inspect runtime behavior and reproducibility for this step".into(),
            ),
        };
        ExecutionImprovementProposal {
            pattern: self.clone(),
            area,
            rationale,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanExecutionReceipt {
    pub attempt_id: String,
    pub plan_id: String,
    pub status: PlanExecutionStatus,
    pub failed_step: Option<String>,
    pub failure_kind: Option<ExecutionFailureKind>,
    pub failure_reason: Option<String>,
    pub verification_receipt: Option<String>,
    pub produced_fact_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum PlanExecutionRejection {
    StalePlan(PlanLifecycle),
    AttemptAlreadyExists(String),
    UnknownAttempt(String),
    AttemptAlreadyTerminal(String),
    EmptyVerificationReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ExecutionFactCommitFailure {
    UnknownAttempt(String),
    AttemptNotSucceeded(String),
    FactIdsMismatch {
        expected: Vec<String>,
        provided: Vec<String>,
    },
    Policy(FactIndexRejection),
    Conflict(FactConflict),
}

/// Lifecycle ledger for execution attempts. It records state transitions but
/// deliberately delegates actual capability execution and verification to
/// their existing typed modules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct PlanExecutionLedger {
    attempts: BTreeMap<String, PlanExecutionReceipt>,
    diagnoses: BTreeMap<String, ExecutionFailureDiagnosis>,
}

impl PlanExecutionLedger {
    pub fn start(
        &mut self,
        attempt_id: impl Into<String>,
        plan_id: impl Into<String>,
        plan: &GoalCapabilityPlan,
        fact_index: &DerivedFactIndex,
    ) -> Result<PlanExecutionReceipt, PlanExecutionRejection> {
        let attempt_id = attempt_id.into();
        let plan_id = plan_id.into();
        if self.attempts.contains_key(&attempt_id) {
            return Err(PlanExecutionRejection::AttemptAlreadyExists(attempt_id));
        }
        let lifecycle = plan.lifecycle(fact_index);
        if !lifecycle.is_active() {
            return Err(PlanExecutionRejection::StalePlan(lifecycle));
        }
        let receipt = PlanExecutionReceipt {
            attempt_id: attempt_id.clone(),
            plan_id,
            status: PlanExecutionStatus::Running,
            failed_step: None,
            failure_kind: None,
            failure_reason: None,
            verification_receipt: None,
            produced_fact_ids: Vec::new(),
        };
        self.attempts.insert(attempt_id, receipt.clone());
        Ok(receipt)
    }

    pub fn attempt(&self, attempt_id: &str) -> Option<&PlanExecutionReceipt> {
        self.attempts.get(attempt_id)
    }

    pub fn complete_success(
        &mut self,
        attempt_id: &str,
        verification_receipt: impl Into<String>,
        produced_fact_ids: Vec<String>,
    ) -> Result<PlanExecutionReceipt, PlanExecutionRejection> {
        let receipt = self
            .attempts
            .get_mut(attempt_id)
            .ok_or_else(|| PlanExecutionRejection::UnknownAttempt(attempt_id.to_string()))?;
        if receipt.status != PlanExecutionStatus::Running {
            return Err(PlanExecutionRejection::AttemptAlreadyTerminal(
                attempt_id.to_string(),
            ));
        }
        let verification_receipt = verification_receipt.into();
        if verification_receipt.trim().is_empty() {
            return Err(PlanExecutionRejection::EmptyVerificationReceipt);
        }
        receipt.status = PlanExecutionStatus::Succeeded;
        receipt.verification_receipt = Some(verification_receipt);
        receipt.produced_fact_ids = produced_fact_ids;
        Ok(receipt.clone())
    }

    pub fn complete_failure(
        &mut self,
        attempt_id: &str,
        failed_step: impl Into<String>,
        reason: impl Into<String>,
    ) -> Result<PlanExecutionReceipt, PlanExecutionRejection> {
        self.complete_failure_with_kind(
            attempt_id,
            failed_step,
            ExecutionFailureKind::RuntimeFailure,
            reason,
        )
    }

    pub fn complete_failure_with_kind(
        &mut self,
        attempt_id: &str,
        failed_step: impl Into<String>,
        kind: ExecutionFailureKind,
        reason: impl Into<String>,
    ) -> Result<PlanExecutionReceipt, PlanExecutionRejection> {
        let failed_step = failed_step.into();
        let reason = reason.into();
        let receipt = self
            .attempts
            .get_mut(attempt_id)
            .ok_or_else(|| PlanExecutionRejection::UnknownAttempt(attempt_id.to_string()))?;
        if receipt.status != PlanExecutionStatus::Running {
            return Err(PlanExecutionRejection::AttemptAlreadyTerminal(
                attempt_id.to_string(),
            ));
        }
        receipt.status = PlanExecutionStatus::Failed;
        receipt.failed_step = Some(failed_step.clone());
        receipt.failure_kind = Some(kind);
        receipt.failure_reason = Some(reason.clone());
        self.diagnoses.insert(
            attempt_id.to_string(),
            ExecutionFailureDiagnosis {
                attempt_id: attempt_id.to_string(),
                plan_id: receipt.plan_id.clone(),
                failed_step,
                kind,
                reason,
            },
        );
        Ok(receipt.clone())
    }

    pub fn diagnosis(&self, attempt_id: &str) -> Option<&ExecutionFailureDiagnosis> {
        self.diagnoses.get(attempt_id)
    }

    pub fn failure_diagnoses(&self) -> impl Iterator<Item = &ExecutionFailureDiagnosis> {
        self.diagnoses.values()
    }

    /// Aggregate explicit failure history into inspectable process signals.
    /// These statistics are diagnostic only; they do not alter capability
    /// scores, model selection, or authorization policy.
    pub fn failure_patterns(&self) -> Vec<ExecutionFailurePattern> {
        let mut grouped: BTreeMap<
            (String, String, ExecutionFailureKind),
            (usize, BTreeSet<String>),
        > = BTreeMap::new();
        for diagnosis in self.diagnoses.values() {
            let entry = grouped
                .entry((
                    diagnosis.plan_id.clone(),
                    diagnosis.failed_step.clone(),
                    diagnosis.kind,
                ))
                .or_insert_with(|| (0, BTreeSet::new()));
            entry.0 += 1;
            entry.1.insert(diagnosis.reason.clone());
        }
        grouped
            .into_iter()
            .map(|((plan_id, failed_step, kind), (occurrences, reasons))| {
                ExecutionFailurePattern {
                    plan_id,
                    failed_step,
                    kind,
                    occurrences,
                    reasons: reasons.into_iter().collect(),
                }
            })
            .collect()
    }

    pub fn improvement_proposals(&self) -> Vec<ExecutionImprovementProposal> {
        self.failure_patterns()
            .iter()
            .map(ExecutionFailurePattern::improvement_proposal)
            .collect()
    }

    /// Atomically publish verified outputs from a successful attempt into the
    /// derived-fact ledger. The ledger is staged on a clone so policy failure
    /// or contradiction cannot leave a partial result behind.
    pub fn commit_verified_facts(
        &self,
        attempt_id: &str,
        facts: Vec<(String, DerivedFact)>,
        index: &mut DerivedFactIndex,
        policy: &FactPolicy,
    ) -> Result<Vec<FactIndexInsert>, ExecutionFactCommitFailure> {
        let receipt = self
            .attempts
            .get(attempt_id)
            .ok_or_else(|| ExecutionFactCommitFailure::UnknownAttempt(attempt_id.to_string()))?;
        if receipt.status != PlanExecutionStatus::Succeeded {
            return Err(ExecutionFactCommitFailure::AttemptNotSucceeded(
                attempt_id.to_string(),
            ));
        }
        let mut expected = receipt.produced_fact_ids.clone();
        let mut provided = facts
            .iter()
            .map(|(_, fact)| fact.id.clone())
            .collect::<Vec<_>>();
        expected.sort();
        provided.sort();
        if expected != provided {
            return Err(ExecutionFactCommitFailure::FactIdsMismatch { expected, provided });
        }
        let mut staged = index.clone();
        let mut outcomes = Vec::new();
        for (key, fact) in facts {
            match staged.insert(key, fact, policy) {
                Ok(FactIndexInsert::Added) => outcomes.push(FactIndexInsert::Added),
                Ok(FactIndexInsert::Conflict(conflict)) => {
                    return Err(ExecutionFactCommitFailure::Conflict(conflict));
                }
                Err(reason) => return Err(ExecutionFactCommitFailure::Policy(reason)),
            }
        }
        *index = staged;
        Ok(outcomes)
    }
}

/// A proposal for replacing a stale plan. The caller must explicitly review
/// and register/execute the replacement; construction has no side effects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanRepairCandidate {
    pub plan_id: String,
    pub stale_plan: PlanLifecycle,
    pub replacement: GoalCapabilityPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PlanCostDelta {
    pub steps: i64,
    pub dependency_edges: i64,
    pub verification_steps: i64,
}

/// Diagnostic comparison between a stale plan and a proposed replacement.
/// This does not rank or authorize either plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanRepairEvaluation {
    pub plan_id: String,
    pub old_cost: PlanCost,
    pub replacement_cost: PlanCost,
    pub cost_delta: PlanCostDelta,
    pub added_capabilities: Vec<String>,
    pub removed_capabilities: Vec<String>,
    pub invalidated_fact_ids: Vec<String>,
    pub replacement_fact_ids: Vec<String>,
}

impl PlanRepairCandidate {
    pub fn evaluate_against(&self, old_plan: &GoalCapabilityPlan) -> PlanRepairEvaluation {
        let old_steps = old_plan
            .steps
            .iter()
            .map(|step| step.capability_id.clone())
            .collect::<BTreeSet<_>>();
        let replacement_steps = self
            .replacement
            .steps
            .iter()
            .map(|step| step.capability_id.clone())
            .collect::<BTreeSet<_>>();
        let added_capabilities = replacement_steps
            .difference(&old_steps)
            .cloned()
            .collect();
        let removed_capabilities = old_steps
            .difference(&replacement_steps)
            .cloned()
            .collect();
        let invalidated_fact_ids = self
            .stale_plan
            .invalidations
            .iter()
            .map(|invalidation| invalidation.fact_id.clone())
            .collect();
        let replacement_fact_ids = self
            .replacement
            .derived_fact_proofs
            .iter()
            .map(|proof| proof.fact_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        PlanRepairEvaluation {
            plan_id: self.plan_id.clone(),
            old_cost: old_plan.cost,
            replacement_cost: self.replacement.cost,
            cost_delta: PlanCostDelta {
                steps: self.replacement.cost.steps as i64 - old_plan.cost.steps as i64,
                dependency_edges: self.replacement.cost.dependency_edges as i64
                    - old_plan.cost.dependency_edges as i64,
                verification_steps: self.replacement.cost.verification_steps as i64
                    - old_plan.cost.verification_steps as i64,
            },
            added_capabilities,
            removed_capabilities,
            invalidated_fact_ids,
            replacement_fact_ids,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RepairDecisionPolicy {
    pub allow_cost_increase: bool,
    pub require_verification: bool,
}

impl RepairDecisionPolicy {
    pub fn strict() -> Self {
        Self {
            allow_cost_increase: false,
            require_verification: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum RepairDecisionRejection {
    ReplacementStillStale(PlanLifecycle),
    CostIncrease(PlanCostDelta),
    MissingVerifier(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanRepairDecision {
    pub evaluation: PlanRepairEvaluation,
    pub accepted: bool,
    pub rejections: Vec<RepairDecisionRejection>,
}

impl PlanRepairDecision {
    pub fn is_accepted(&self) -> bool {
        self.accepted
    }
}

impl RepairDecisionPolicy {
    /// Evaluate one candidate. This is a gate and receipt, not a ranking or
    /// replacement operation; callers must still resolve competing accepted
    /// candidates explicitly.
    pub fn evaluate(
        &self,
        old_plan: &GoalCapabilityPlan,
        candidate: &PlanRepairCandidate,
        fact_index: &DerivedFactIndex,
    ) -> PlanRepairDecision {
        let evaluation = candidate.evaluate_against(old_plan);
        let mut rejections = Vec::new();
        let replacement_lifecycle = candidate.replacement.lifecycle(fact_index);
        if !replacement_lifecycle.is_active() {
            rejections.push(RepairDecisionRejection::ReplacementStillStale(
                replacement_lifecycle,
            ));
        }
        if !self.allow_cost_increase
            && (evaluation.cost_delta.steps > 0
                || evaluation.cost_delta.dependency_edges > 0
                || evaluation.cost_delta.verification_steps > 0)
        {
            rejections.push(RepairDecisionRejection::CostIncrease(
                evaluation.cost_delta,
            ));
        }
        if self.require_verification {
            rejections.extend(
                candidate
                    .replacement
                    .steps
                    .iter()
                    .filter(|step| step.verifier.is_empty())
                    .map(|step| RepairDecisionRejection::MissingVerifier(
                        step.capability_id.clone(),
                    )),
            );
        }
        PlanRepairDecision {
            accepted: rejections.is_empty(),
            evaluation,
            rejections,
        }
    }
}

/// Replan a stale goal using only facts that are currently active in the
/// ledger. This deliberately returns a candidate rather than mutating the
/// old plan or executing the replacement.
pub fn replan_stale_plan(
    plan_id: impl Into<String>,
    plan: &GoalCapabilityPlan,
    context: &ReasoningContext,
    fact_index: &DerivedFactIndex,
    registry: &CapabilityRegistry,
) -> Result<PlanRepairCandidate, PlanRepairFailure> {
    let plan_id = plan_id.into();
    let stale_plan = plan.lifecycle(fact_index);
    if stale_plan.is_active() {
        return Err(PlanRepairFailure::PlanStillActive);
    }
    let active_context = ReasoningContext {
        available_inputs: context.available_inputs.clone(),
        derived_facts: context
            .derived_facts
            .iter()
            .filter(|fact| {
                fact_index
                    .lifecycle(&fact.id)
                    .map(|lifecycle| lifecycle.status == FactStatus::Active)
                    .unwrap_or(false)
            })
            .cloned()
            .collect(),
        fact_retrieval_receipts: context.fact_retrieval_receipts.clone(),
    };
    let replacement = plan_for_goal_with_context(plan.goal, &active_context, registry)
        .map_err(PlanRepairFailure::Planning)?;
    let replacement_lifecycle = replacement.lifecycle(fact_index);
    if !replacement_lifecycle.is_active() {
        return Err(PlanRepairFailure::ReplacementStillStale(
            replacement_lifecycle,
        ));
    }
    Ok(PlanRepairCandidate {
        plan_id,
        stale_plan,
        replacement,
    })
}

fn plan_lifecycle(proofs: &[DerivedFactProof], index: &DerivedFactIndex) -> PlanLifecycle {
    let fact_ids = proofs
        .iter()
        .map(|proof| proof.fact_id.clone())
        .collect::<BTreeSet<_>>();
    plan_lifecycle_for_ids(&fact_ids, index)
}

fn plan_lifecycle_for_ids(
    fact_ids: &BTreeSet<String>,
    index: &DerivedFactIndex,
) -> PlanLifecycle {
    let mut invalidations = Vec::new();
    for fact_id in fact_ids {
        let issue = match index.lifecycle(fact_id) {
            None => PlanFactIssue::Missing,
            Some(lifecycle) if lifecycle.status == FactStatus::Active => continue,
            Some(lifecycle) => PlanFactIssue::Inactive(lifecycle.status),
        };
        invalidations.push(PlanFactInvalidation {
            fact_id: fact_id.clone(),
            issue,
        });
    }
    invalidations.sort_by(|left, right| left.fact_id.cmp(&right.fact_id));
    PlanLifecycle {
        status: if invalidations.is_empty() {
            PlanStatus::Active
        } else {
            PlanStatus::Stale
        },
        invalidations,
    }
}

fn plan_metadata(
    selected: &str,
    steps: &[CapabilityPlanStep],
    registry: &CapabilityRegistry,
    available_inputs: Option<&BTreeSet<CapabilityIoType>>,
) -> (PlanCost, Vec<DependencyProof>, Vec<InputProof>) {
    let mut dependency_proofs = Vec::new();
    let mut input_proofs = Vec::new();
    for step in steps {
        if let Some(capability) = registry.get(&step.capability_id) {
            for dependency in &capability.dependencies {
                dependency_proofs.push(DependencyProof {
                    capability: capability.id.clone(),
                    dependency: dependency.clone(),
                });
            }
            if step.capability_id == selected {
                if let Some(available) = available_inputs {
                    for input in &capability.consumes {
                        if available.contains(input) {
                            input_proofs.push(InputProof {
                                capability: capability.id.clone(),
                                input: *input,
                            });
                        }
                    }
                }
            }
        }
    }
    let cost = PlanCost {
        steps: steps.len(),
        dependency_edges: dependency_proofs.len(),
        verification_steps: steps.iter().filter(|step| !step.verifier.is_empty()).count(),
    };
    (cost, dependency_proofs, input_proofs)
}

fn dependency_steps(
    id: &str,
    registry: &CapabilityRegistry,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    steps: &mut Vec<CapabilityPlanStep>,
) -> Result<(), CapabilityPlanningFailure> {
    if visited.contains(id) {
        return Ok(());
    }
    if !visiting.insert(id.to_string()) {
        return Err(CapabilityPlanningFailure::DependencyCycle(id.to_string()));
    }
    let capability = registry
        .get(id)
        .ok_or_else(|| CapabilityPlanningFailure::DependencyUnavailable(id.to_string()))?;
    if !capability.quality_gate.enabled() {
        return Err(CapabilityPlanningFailure::DependencyUnavailable(id.to_string()));
    }
    for dependency in &capability.dependencies {
        dependency_steps(dependency, registry, visiting, visited, steps)?;
    }
    visiting.remove(id);
    visited.insert(id.to_string());
    steps.push(CapabilityPlanStep {
        capability_id: capability.id.clone(),
        version: capability.version,
        executor: capability.executor.clone(),
        verifier: capability.verifier.clone(),
        consumes: capability.consumes.clone(),
        produces: capability.produces.clone(),
    });
    Ok(())
}

pub fn plan_target(
    target: &FormalizedTarget,
    registry: &CapabilityRegistry,
) -> Result<CapabilityPlan, CapabilityPlanningFailure> {
    let discovery = registry.discover(target);
    let selected = match discovery.selection {
        CapabilitySelection::Unique(id) => id,
        CapabilitySelection::Ambiguous(ids) => {
            return Err(CapabilityPlanningFailure::AmbiguousCapabilities(ids))
        }
        CapabilitySelection::None => return Err(CapabilityPlanningFailure::NoEligibleCapability),
    };
    let subject_type = target
        .subject_resolution
        .selected
        .as_ref()
        .map(|subject| subject.object_type)
        .ok_or(CapabilityPlanningFailure::NoEligibleCapability)?;
    let mut steps = Vec::new();
    dependency_steps(
        &selected,
        registry,
        &mut BTreeSet::new(),
        &mut BTreeSet::new(),
        &mut steps,
    )?;
    let (cost, dependency_proofs, input_proofs) =
        plan_metadata(&selected, &steps, registry, None);
    Ok(CapabilityPlan {
        operation: target.operation,
        subject_type,
        answer_form: target.answer_form,
        selected_capability: selected,
        steps,
        cost,
        selection_reason: PlanSelectionReason::UniqueTargetCapability,
        dependency_proofs,
        input_proofs,
    })
}

/// Plan a typed dataflow chain by searching backward from `goal`.
///
/// This planner is intentionally conservative: a goal with more than one
/// eligible producer is ambiguous, even if one route looks shorter.  The
/// returned steps are dependency-first and dataflow-first; execution and
/// verification remain separate authorization stages.
pub fn plan_capability_chain(
    goal: CapabilityIoType,
    available_inputs: &BTreeSet<CapabilityIoType>,
    registry: &CapabilityRegistry,
) -> Result<CapabilityChainPlan, CapabilityChainPlanningFailure> {
    fn satisfy(
        goal: CapabilityIoType,
        available: &mut BTreeSet<CapabilityIoType>,
        registry: &CapabilityRegistry,
        visiting: &mut BTreeSet<String>,
        steps: &mut Vec<String>,
    ) -> Result<(), CapabilityChainPlanningFailure> {
        if available.contains(&goal) {
            return Ok(());
        }
        let candidates = registry
            .capabilities
            .values()
            .filter(|capability| {
                capability.quality_gate.enabled() && capability.produces.contains(&goal)
            })
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return Err(CapabilityChainPlanningFailure::NoProducer(goal));
        }
        if candidates.len() != 1 {
            return Err(CapabilityChainPlanningFailure::AmbiguousProducers {
                goal,
                candidates: candidates.iter().map(|capability| capability.id.clone()).collect(),
            });
        }
        let capability = candidates[0];
        if !visiting.insert(capability.id.clone()) {
            return Err(CapabilityChainPlanningFailure::DependencyCycle(
                capability.id.clone(),
            ));
        }
        for dependency in &capability.dependencies {
            let dependency_spec = registry.get(dependency).ok_or_else(|| {
                CapabilityChainPlanningFailure::DependencyUnavailable {
                    capability: capability.id.clone(),
                    dependency: dependency.clone(),
                }
            })?;
            if !dependency_spec.quality_gate.enabled() {
                return Err(CapabilityChainPlanningFailure::DependencyUnavailable {
                    capability: capability.id.clone(),
                    dependency: dependency.clone(),
                });
            }
        }
        for input in &capability.consumes {
            satisfy(*input, available, registry, visiting, steps)?;
        }
        visiting.remove(&capability.id);
        steps.push(capability.id.clone());
        available.extend(capability.produces.iter().copied());
        Ok(())
    }

    let mut available = available_inputs.clone();
    let mut visiting = BTreeSet::new();
    let mut steps = Vec::new();
    satisfy(goal, &mut available, registry, &mut visiting, &mut steps)?;
    Ok(CapabilityChainPlan { goal, steps })
}

fn validate_verified_artifact_plan_policy(
    steps: &[String],
    registry: &CapabilityRegistry,
    policy: &VerifiedArtifactPolicy,
) -> Result<(), VerifiedArtifactPlanningFailure> {
    let proof_steps = steps
        .iter()
        .filter(|step| step.as_str() != "verified_artifact_wrap")
        .collect::<Vec<_>>();
    if proof_steps.len() < policy.minimum_proof_steps {
        return Err(VerifiedArtifactPlanningFailure::InsufficientProofSteps {
            required: policy.minimum_proof_steps,
            available: proof_steps.len(),
        });
    }
    if policy.require_replay_verified {
        for step in &proof_steps {
            let Some(capability) = registry.get(step) else {
                return Err(VerifiedArtifactPlanningFailure::MissingStepVerifier(
                    (*step).clone(),
                ));
            };
            if capability.verifier.trim().is_empty() {
                return Err(VerifiedArtifactPlanningFailure::MissingStepVerifier(
                    (*step).clone(),
                ));
            }
        }
    }
    if policy.require_final_verification_receipt {
        let has_final_verifier = proof_steps
            .last()
            .and_then(|step| registry.get(step))
            .map(|capability| !capability.verifier.trim().is_empty())
            .unwrap_or(false);
        if !has_final_verifier {
            return Err(VerifiedArtifactPlanningFailure::MissingFinalVerificationStep);
        }
    }
    Ok(())
}

/// Add the virtual trust-materialization step to an already planned artifact
/// chain and validate the requested consumer policy.  Domain planners can use
/// this helper after constructing domain-specific, proof-producing steps.
pub fn materialize_verified_artifact_chain(
    mut plan: CapabilityChainPlan,
    registry: &CapabilityRegistry,
    policy: &VerifiedArtifactPolicy,
) -> Result<CapabilityChainPlan, VerifiedArtifactPlanningFailure> {
    if plan.goal != CapabilityIoType::VerifiedArtifact {
        plan.steps.push("verified_artifact_wrap".into());
        plan.goal = CapabilityIoType::VerifiedArtifact;
    }
    validate_verified_artifact_plan_policy(&plan.steps, registry, policy)?;
    Ok(plan)
}

/// Plan a generic typed artifact goal with a proof-bearing wrapper.  The
/// underlying artifact is planned through the ordinary dataflow graph; the
/// wrapper is a virtual trust-materialization step and is never treated as a
/// substitute for execution or verification.
pub fn plan_verified_artifact_goal(
    artifact_goal: CapabilityIoType,
    available_inputs: &BTreeSet<CapabilityIoType>,
    registry: &CapabilityRegistry,
    policy: &VerifiedArtifactPolicy,
) -> Result<CapabilityChainPlan, CapabilityChainPlanningFailure> {
    if artifact_goal == CapabilityIoType::VerifiedArtifact {
        return Err(CapabilityChainPlanningFailure::NoProducer(artifact_goal));
    }
    materialize_verified_artifact_chain(
        plan_capability_chain(artifact_goal, available_inputs, registry)?,
        registry,
        policy,
    )
    .map_err(CapabilityChainPlanningFailure::TrustPolicy)
}

/// Build the normalization → classification → solver chain for one grounded
/// equation. The verified classifier resolves the otherwise ambiguous linear
/// and quadratic producers before a solver step is selected.
pub fn plan_equation_chain(
    source: &str,
    target_variable: &str,
    goal: CapabilityIoType,
    registry: &CapabilityRegistry,
) -> Result<EquationChainPlan, EquationChainPlanningFailure> {
    plan_equation_chain_with_policy(
        source,
        target_variable,
        goal,
        registry,
        &VerifiedArtifactPolicy::default(),
    )
}

/// Plan an equation chain while making the consumer's proof requirements
/// part of planning.  This rejects chains that cannot satisfy the requested
/// verified-artifact contract before any execution is attempted.
pub fn plan_equation_chain_with_policy(
    source: &str,
    target_variable: &str,
    goal: CapabilityIoType,
    registry: &CapabilityRegistry,
    policy: &VerifiedArtifactPolicy,
) -> Result<EquationChainPlan, EquationChainPlanningFailure> {
    if goal != CapabilityIoType::SolutionSet
        && goal != CapabilityIoType::VerifiedSolutionSet
        && goal != CapabilityIoType::VerifiedArtifact
    {
        return Err(EquationChainPlanningFailure::UnsupportedGoal(goal));
    }
    if target_variable.trim().is_empty() {
        return Err(EquationChainPlanningFailure::MissingTargetVariable);
    }
    let normalized = execute_equation_normalization(source)
        .map_err(EquationChainPlanningFailure::Normalization)?;
    let classification = execute_equation_classification(&normalized.normalized_equation)
        .map_err(EquationChainPlanningFailure::Classification)?;
    let selected_solver = route_classified_equation(&classification, registry)
        .map_err(EquationChainPlanningFailure::Routing)?;
    for capability_id in ["equation_normalization", "equation_classification"] {
        let Some(capability) = registry.get(capability_id) else {
            return Err(EquationChainPlanningFailure::CapabilityUnavailable(
                capability_id.into(),
            ));
        };
        if !capability.quality_gate.enabled() {
            return Err(EquationChainPlanningFailure::CapabilityUnavailable(
                capability_id.into(),
            ));
        }
    }
    let Some(solver) = registry.get(&selected_solver) else {
        return Err(EquationChainPlanningFailure::CapabilityUnavailable(
            selected_solver,
        ));
    };
    if !solver.quality_gate.enabled()
        || !solver.consumes.contains(&CapabilityIoType::NormalizedEquation)
        || !solver.consumes.contains(&CapabilityIoType::TargetVariable)
    {
        return Err(EquationChainPlanningFailure::CapabilityUnavailable(
            solver.id.clone(),
        ));
    }
    if goal == CapabilityIoType::VerifiedSolutionSet
        || goal == CapabilityIoType::VerifiedArtifact
    {
        let Some(verifier) = registry.get("solution_set_verification") else {
            return Err(EquationChainPlanningFailure::CapabilityUnavailable(
                "solution_set_verification".into(),
            ));
        };
        if !verifier.quality_gate.enabled()
            || !verifier
                .consumes
                .contains(&CapabilityIoType::CandidateSolutionSet)
            || !verifier
                .produces
                .contains(&CapabilityIoType::VerifiedSolutionSet)
        {
            return Err(EquationChainPlanningFailure::CapabilityUnavailable(
                "solution_set_verification".into(),
            ));
        }
    }
    let mut steps = vec![
        "equation_normalization".into(),
        "equation_classification".into(),
        selected_solver.clone(),
    ];
    if goal == CapabilityIoType::VerifiedSolutionSet
        || goal == CapabilityIoType::VerifiedArtifact
    {
        steps.push("solution_set_verification".into());
    }
    let mut chain_goal = goal;
    if goal == CapabilityIoType::VerifiedArtifact {
        chain_goal = CapabilityIoType::SolutionSet;
    }
    let mut chain = CapabilityChainPlan {
        goal: chain_goal,
        steps,
    };
    if goal == CapabilityIoType::VerifiedArtifact {
        chain = materialize_verified_artifact_chain(chain, registry, policy)
            .map_err(EquationChainPlanningFailure::TrustPolicy)?;
    }
    Ok(EquationChainPlan {
        source: source.trim().into(),
        target_variable: target_variable.trim().into(),
        normalized_equation: normalized.normalized_equation,
        classification,
        selected_solver: selected_solver.clone(),
        chain,
    })
}

/// Select one capability that can produce `goal` from the explicitly
/// available artifacts.  This is deliberately one-step dataflow planning;
/// dependencies are expanded, but missing data inputs are not invented.
pub fn plan_for_goal(
    goal: CapabilityIoType,
    available_inputs: &BTreeSet<CapabilityIoType>,
    registry: &CapabilityRegistry,
) -> Result<GoalCapabilityPlan, CapabilityPlanningFailure> {
    plan_for_goal_with_context(
        goal,
        &ReasoningContext::new(available_inputs.clone()),
        registry,
    )
}

/// Reuse active, conflict-free facts from the governed ledger when planning
/// a goal. The selected capability's own FactPolicy remains the consumer-side
/// trust gate for each retrieved fact.
pub fn plan_for_goal_with_fact_index(
    goal: CapabilityIoType,
    available_inputs: BTreeSet<CapabilityIoType>,
    index: &DerivedFactIndex,
    registry: &CapabilityRegistry,
) -> Result<GoalCapabilityPlan, CapabilityPlanningFailure> {
    let context = ReasoningContext::try_from_fact_index(available_inputs, index)
        .map_err(CapabilityPlanningFailure::FactIndex)?;
    plan_for_goal_with_context(goal, &context, registry)
}

/// Goal-directed planning with a unified artifact context.  Raw model
/// evidence is intentionally absent here; derived facts enter only through a
/// capability that explicitly consumes `DerivedFact` and declares a
/// lineage-validating `FactPolicy`.
pub fn plan_for_goal_with_context(
    goal: CapabilityIoType,
    context: &ReasoningContext,
    registry: &CapabilityRegistry,
) -> Result<GoalCapabilityPlan, CapabilityPlanningFailure> {
    let mut candidates = registry
        .capabilities
        .values()
        .filter(|capability| capability.quality_gate.enabled())
        .filter(|capability| capability.produces.contains(&goal))
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Err(CapabilityPlanningFailure::NoProducer(goal));
    }
    let mut candidate_fact_proofs = BTreeMap::new();
    let mut candidate_failures = BTreeMap::new();
    candidates.retain(|capability| {
        let mut eligible = true;
        let mut missing = Vec::new();
        for input in &capability.consumes {
            if *input == CapabilityIoType::DerivedFact {
                let Some(policy) = capability.fact_policy.as_ref() else {
                    eligible = false;
                    candidate_failures.insert(
                        capability.id.clone(),
                        CapabilityPlanningFailure::MissingFactPolicy(capability.id.clone()),
                    );
                    continue;
                };
                let mut proofs = Vec::new();
                let mut rejections = Vec::new();
                for fact in &context.derived_facts {
                    match policy.evaluate(fact, crate::evidence::EvidenceStatus::Inferred) {
                        Ok(()) => proofs.push(DerivedFactProof {
                            capability: capability.id.clone(),
                            fact_id: fact.id.clone(),
                            parent_lineage: fact.parent_lineage.clone(),
                            retrieval_receipt: context
                                .fact_retrieval_receipts
                                .get(&fact.id)
                                .cloned(),
                        }),
                        Err(reason) => rejections.push(DerivedFactRejection {
                            fact_id: fact.id.clone(),
                            reason,
                        }),
                    }
                }
                if proofs.is_empty() {
                    eligible = false;
                    candidate_failures.insert(
                        capability.id.clone(),
                        if context.derived_facts.is_empty() {
                            CapabilityPlanningFailure::MissingInputs {
                                capability: capability.id.clone(),
                                missing: vec![CapabilityIoType::DerivedFact],
                            }
                        } else {
                            CapabilityPlanningFailure::InvalidDerivedFacts {
                                capability: capability.id.clone(),
                                rejections,
                            }
                        },
                    );
                } else {
                    candidate_fact_proofs.insert(capability.id.clone(), proofs);
                }
            } else if !context.available_inputs.contains(input) {
                eligible = false;
                missing.push(*input);
            }
        }
        if !missing.is_empty() {
            candidate_failures.insert(
                capability.id.clone(),
                CapabilityPlanningFailure::MissingInputs {
                    capability: capability.id.clone(),
                    missing,
                },
            );
        }
        eligible
    });
    if candidates.is_empty() {
        let mut possible = registry
            .capabilities
            .values()
            .filter(|capability| capability.quality_gate.enabled())
            .filter(|capability| capability.produces.contains(&goal));
        let capability = possible.next().expect("producer checked above");
        if let Some(failure) = candidate_failures.remove(&capability.id) {
            return Err(failure);
        }
        let missing = capability
            .consumes
            .iter()
            .filter(|input| !context.available_inputs.contains(input))
            .copied()
            .collect();
        return Err(CapabilityPlanningFailure::MissingInputs { capability: capability.id.clone(), missing });
    }
    if candidates.len() > 1 {
        return Err(CapabilityPlanningFailure::AmbiguousCapabilities(
            candidates.into_iter().map(|capability| capability.id.clone()).collect(),
        ));
    }
    let selected = candidates[0];
    let mut steps = Vec::new();
    dependency_steps(
        &selected.id,
        registry,
        &mut BTreeSet::new(),
        &mut BTreeSet::new(),
        &mut steps,
    )?;
    let (cost, dependency_proofs, input_proofs) =
        plan_metadata(&selected.id, &steps, registry, Some(&context.available_inputs));
    Ok(GoalCapabilityPlan {
        goal,
        available_inputs: context.available_inputs.iter().copied().collect(),
        selected_capability: selected.id.clone(),
        steps,
        cost,
        selection_reason: PlanSelectionReason::UniqueGoalProducer,
        dependency_proofs,
        input_proofs,
        derived_fact_proofs: candidate_fact_proofs.remove(&selected.id).unwrap_or_default(),
    })
}

/// Plan a uniquely selected text model into one uniquely selected capability
/// producer.  No model is inferred when discovery is empty or ambiguous, and
/// no missing downstream artifact is invented.
pub fn plan_model_to_goal(
    text: &str,
    goal: CapabilityIoType,
    model_registry: &ModelConstructorRegistry,
    capability_registry: &CapabilityRegistry,
) -> Result<ModelCapabilityPlan, ModelPlanningFailure> {
    let discovery = model_registry.discover(text);
    let (model_id, model_version) = match discovery.selection {
        ModelSelection::UniqueVersioned { id, version } => (id, version),
        ModelSelection::Ambiguous(ids) => return Err(ModelPlanningFailure::AmbiguousModels(ids)),
        ModelSelection::None => return Err(ModelPlanningFailure::NoEligibleModel),
    };
    let entry = model_registry
        .get_versioned(&model_id, model_version)
        .ok_or_else(|| ModelPlanningFailure::MissingModelEntry(model_id.clone()))?;
    let available_inputs = entry
        .spec
        .produced_artifacts
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let capability_plan = plan_for_goal(goal, &available_inputs, capability_registry)
        .map_err(ModelPlanningFailure::CapabilityPlanning)?;
    let model_step = ModelPlanStep {
        model_id,
        version: entry.spec.version,
        model_artifacts: entry.spec.model_artifacts.clone(),
        downstream_artifacts: entry.spec.produced_artifacts.clone(),
    };
    let cost = PlanCost {
        steps: capability_plan.cost.steps + 1,
        dependency_edges: capability_plan.cost.dependency_edges,
        verification_steps: capability_plan.cost.verification_steps + 1,
    };
    Ok(ModelCapabilityPlan {
        goal,
        model_step,
        capability_plan,
        cost,
        selection_reason: PlanSelectionReason::UniqueModelThenGoalProducer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::{
        CapabilityIoType, CapabilityRegistry, CapabilitySpec, InputRequirement,
    };
    use crate::evidence::{DerivedFact, DerivedFactIndex, FactIndexInsert, FactPolicy};
    use crate::formalization::assess_prompt;
    use crate::constant_rate_model::ModelConstructorRegistry;

    #[test]
    fn typed_chain_planner_composes_unique_dataflow_steps() {
        let mut registry = CapabilityRegistry::default();
        registry.register(CapabilitySpec::expression_simplification_v1());
        let mut evaluate = CapabilitySpec::expression_evaluation_v1();
        evaluate.id = "evaluate_simplified_expression".into();
        evaluate.consumes = vec![
            CapabilityIoType::SimplifiedExpression,
            CapabilityIoType::BindingSet,
        ];
        registry.register(evaluate);
        let plan = plan_capability_chain(
            CapabilityIoType::ExactValue,
            &BTreeSet::from([CapabilityIoType::Expression, CapabilityIoType::BindingSet]),
            &registry,
        )
        .unwrap();
        assert_eq!(
            plan.steps,
            vec![
                "expression_simplification".to_string(),
                "evaluate_simplified_expression".to_string(),
            ]
        );
    }

    #[test]
    fn typed_chain_planner_abstains_on_competing_producers() {
        let mut registry = CapabilityRegistry::default();
        let first = CapabilitySpec::expression_simplification_v1();
        let mut second = first.clone();
        second.id = "alternate_simplification".into();
        registry.register(first);
        registry.register(second);
        assert_eq!(
            plan_capability_chain(
                CapabilityIoType::SimplifiedExpression,
                &BTreeSet::from([CapabilityIoType::Expression]),
                &registry,
            ),
            Err(CapabilityChainPlanningFailure::AmbiguousProducers {
                goal: CapabilityIoType::SimplifiedExpression,
                candidates: vec![
                    "alternate_simplification".to_string(),
                    "expression_simplification".to_string(),
                ],
            })
        );
    }

    #[test]
    fn chain_cost_ranking_is_diagnostic_and_deterministic() {
        let registry = CapabilityRegistry::production();
        let short = CapabilityChainPlan {
            goal: CapabilityIoType::SimplifiedExpression,
            steps: vec!["expression_simplification".into()],
        };
        let long = CapabilityChainPlan {
            goal: CapabilityIoType::ExactValue,
            steps: vec![
                "expression_simplification".into(),
                "expression_evaluation".into(),
            ],
        };
        let ranked = rank_capability_chains(
            vec![("long".into(), long), ("short".into(), short)],
            &registry,
        )
        .unwrap();
        assert_eq!(ranked[0].candidate_id, "short");
        assert_eq!(ranked[0].cost.steps, 1);
        assert_eq!(ranked[0].diagnostics.verification_evidence, 3);
        assert_eq!(ranked[0].diagnostics.contract_burden, 2);
        assert_eq!(ranked[0].diagnostics.quality_failures, 0);
        assert_eq!(ranked[1].cost.steps, 2);
    }

    #[test]
    fn chain_preference_receipt_keeps_unique_preference_non_authorizing() {
        let registry = CapabilityRegistry::production();
        let receipt = diagnose_capability_chain_preferences(
            vec![
                (
                    "long".into(),
                    CapabilityChainPlan {
                        goal: CapabilityIoType::ExactValue,
                        steps: vec![
                            "expression_simplification".into(),
                            "expression_evaluation".into(),
                        ],
                    },
                ),
                (
                    "short".into(),
                    CapabilityChainPlan {
                        goal: CapabilityIoType::SimplifiedExpression,
                        steps: vec!["expression_simplification".into()],
                    },
                ),
            ],
            &registry,
        )
        .unwrap();
        assert_eq!(
            receipt.preference,
            CapabilityChainPreference::Preferred("short".into())
        );
        assert_eq!(receipt.ranked_candidates[0].candidate_id, "short");
        let explanation = receipt.explain();
        assert!(explanation.preferred_because.contains(
            &CapabilityChainExplanationNote::LowestCost {
                candidate_id: "short".into()
            }
        ));
        assert_eq!(
            explanation.alternatives,
            vec!["short".to_string(), "long".to_string()]
        );
    }

    #[test]
    fn equal_cost_chain_preference_is_ambiguous() {
        let registry = CapabilityRegistry::production();
        let plan = |goal| CapabilityChainPlan {
            goal,
            steps: vec!["expression_simplification".into()],
        };
        let receipt = diagnose_capability_chain_preferences(
            vec![
                ("b".into(), plan(CapabilityIoType::SimplifiedExpression)),
                ("a".into(), plan(CapabilityIoType::SimplifiedExpression)),
            ],
            &registry,
        )
        .unwrap();
        assert_eq!(
            receipt.preference,
            CapabilityChainPreference::Ambiguous(vec!["a".into(), "b".into()])
        );
        let explanation = receipt.explain();
        assert!(explanation.preferred_because.is_empty());
        assert!(explanation.tradeoffs.contains(
            &CapabilityChainExplanationNote::EqualCost {
                candidate_ids: vec!["a".into(), "b".into()]
            }
        ));
    }

    #[test]
    fn empty_chain_preference_receipt_is_explicit() {
        let registry = CapabilityRegistry::production();
        let receipt = diagnose_capability_chain_preferences(Vec::new(), &registry).unwrap();
        assert_eq!(receipt.preference, CapabilityChainPreference::NoCandidates);
        assert!(receipt.ranked_candidates.is_empty());
    }

    #[test]
    fn chain_execution_requires_ordered_verified_steps() {
        let plan = CapabilityChainPlan {
            goal: CapabilityIoType::ExactValue,
            steps: vec![
                "expression_simplification".into(),
                "evaluate_simplified_expression".into(),
            ],
        };
        let mut ledger = CapabilityChainExecutionLedger::default();
        ledger.start("chain-execution-1", plan).unwrap();
        assert!(matches!(
            ledger.record_step(
                "chain-execution-1",
                CapabilityChainStepReceipt {
                    step_index: 1,
                    capability_id: "evaluate_simplified_expression".into(),
                    input_artifacts: vec!["expr".into()],
                    output_artifacts: vec!["value".into()],
                    verification_receipt: "verified".into(),
                },
            ),
            Err(CapabilityChainExecutionRejection::WrongStepIndex { .. })
        ));
        ledger
            .record_step(
                "chain-execution-1",
                CapabilityChainStepReceipt {
                    step_index: 0,
                    capability_id: "expression_simplification".into(),
                    input_artifacts: vec!["raw-expression".into()],
                    output_artifacts: vec!["expr".into()],
                    verification_receipt: "simplification replay".into(),
                },
            )
            .unwrap();
        assert!(matches!(
            ledger.complete_success("chain-execution-1"),
            Err(CapabilityChainExecutionRejection::IncompleteChain { .. })
        ));
        ledger
            .record_step(
                "chain-execution-1",
                CapabilityChainStepReceipt {
                    step_index: 1,
                    capability_id: "evaluate_simplified_expression".into(),
                    input_artifacts: vec!["expr".into()],
                    output_artifacts: vec!["value".into()],
                    verification_receipt: "evaluation replay".into(),
                },
            )
            .unwrap();
        let completed = ledger.complete_success("chain-execution-1").unwrap();
        assert_eq!(completed.status, CapabilityChainExecutionStatus::Succeeded);
        assert_eq!(completed.steps.len(), 2);
        let proof = compose_capability_chain_proof(&completed).unwrap();
        assert!(proof.replay_verified);
        assert_eq!(proof.steps.len(), 2);
        assert_eq!(proof.final_artifacts, vec!["value"]);
        assert!(proof.retrieved_facts.is_empty());
        let retrieved = DerivedFactProof {
            capability: "evaluate_simplified_expression".into(),
            fact_id: "indexed-expression".into(),
            parent_lineage: vec!["source-expression".into()],
            retrieval_receipt: Some("fact_index_retrieval:expression:indexed-expression".into()),
        };
        let proof_with_retrieval = compose_capability_chain_proof_with_retrieved_facts(
            &completed,
            &[retrieved.clone()],
        )
        .unwrap();
        assert_eq!(proof_with_retrieval.retrieved_facts, vec![retrieved.clone()]);
        let retrieved_second = DerivedFactProof {
            capability: "expression_simplification".into(),
            fact_id: "indexed-aaa".into(),
            parent_lineage: vec!["source-expression-2".into()],
            retrieval_receipt: Some("fact_index_retrieval:expression:indexed-aaa".into()),
        };
        let ordered = compose_capability_chain_proof_with_retrieved_facts(
            &completed,
            &[retrieved.clone(), retrieved_second.clone()],
        )
        .unwrap();
        assert_eq!(
            ordered.retrieved_facts,
            vec![retrieved_second.clone(), retrieved.clone()]
        );
        assert_eq!(
            compose_capability_chain_proof_with_retrieved_facts(
                &completed,
                &[retrieved.clone(), retrieved],
            ),
            Err(CapabilityChainProofFailure::DuplicateFactRetrieval {
                capability: "evaluate_simplified_expression".into(),
                fact_id: "indexed-expression".into(),
            })
        );
        let missing_receipt = DerivedFactProof {
            retrieval_receipt: None,
            ..proof_with_retrieval.retrieved_facts[0].clone()
        };
        assert_eq!(
            compose_capability_chain_proof_with_retrieved_facts(
                &completed,
                &[missing_receipt],
            ),
            Err(CapabilityChainProofFailure::MissingFactRetrievalReceipt(
                "indexed-expression".into()
            ))
        );
        let verified = VerifiedArtifact::from_chain("value", proof.clone()).unwrap();
        assert_eq!(verified.artifact, "value");
        assert_eq!(verified.final_verification_receipt, "evaluation replay");
        assert!(matches!(
            ledger.complete_failure("chain-execution-1", 1, "late failure"),
            Err(CapabilityChainExecutionRejection::ExecutionAlreadyTerminal(_))
        ));
    }

    #[test]
    fn proof_index_reuses_equivalent_reasoning_across_execution_runs() {
        let mut ledger = CapabilityChainExecutionLedger::default();
        let plan = CapabilityChainPlan {
            goal: CapabilityIoType::ExactValue,
            steps: vec!["evaluate_expression".into()],
        };
        ledger.start("proof-index-run-1", plan.clone()).unwrap();
        ledger
            .record_step(
                "proof-index-run-1",
                CapabilityChainStepReceipt {
                    step_index: 0,
                    capability_id: "evaluate_expression".into(),
                    input_artifacts: vec!["expression".into()],
                    output_artifacts: vec!["value".into()],
                    verification_receipt: "evaluation replay".into(),
                },
            )
            .unwrap();
        let first = compose_capability_chain_proof(
            &ledger.complete_success("proof-index-run-1").unwrap(),
        )
        .unwrap();
        let mut second = first.clone();
        second.execution_id = "proof-index-run-2".into();

        assert!(first.same_reasoning(&second));
        assert_eq!(first.reasoning_fingerprint(), second.reasoning_fingerprint());

        let mut index = CapabilityChainProofIndex::default();
        let fingerprint = index.insert(first.clone()).unwrap();
        assert_eq!(index.len(), 1);
        assert_eq!(index.get(&fingerprint), Some(&first));
        assert_eq!(index.find_equivalent(&second), Some(&first));
        assert_eq!(
            index.insert(second),
            Err(CapabilityChainProofIndexFailure::DuplicateFingerprint(
                fingerprint,
            ))
        );
        assert!(!index.is_empty());
    }

    #[test]
    fn verified_proofs_compose_when_artifacts_are_compatible() {
        let first = CapabilityChainProofTrace {
            execution_id: "proof-compose-first".into(),
            plan: CapabilityChainPlan {
                goal: CapabilityIoType::ExactValue,
                steps: vec!["expression_simplification".into()],
            },
            steps: vec![CapabilityChainProofStep {
                step_index: 0,
                capability_id: "expression_simplification".into(),
                input_artifacts: vec!["raw-expression".into()],
                output_artifacts: vec!["simplified-expression".into()],
                verification_receipt: "simplification replay".into(),
            }],
            retrieved_facts: Vec::new(),
            final_artifacts: vec!["simplified-expression".into()],
            replay_verified: true,
        };
        let second = CapabilityChainProofTrace {
            execution_id: "proof-compose-second".into(),
            plan: CapabilityChainPlan {
                goal: CapabilityIoType::ExactValue,
                steps: vec!["evaluate_simplified_expression".into()],
            },
            steps: vec![CapabilityChainProofStep {
                step_index: 0,
                capability_id: "evaluate_simplified_expression".into(),
                input_artifacts: vec!["simplified-expression".into()],
                output_artifacts: vec!["value".into()],
                verification_receipt: "evaluation replay".into(),
            }],
            retrieved_facts: Vec::new(),
            final_artifacts: vec!["value".into()],
            replay_verified: true,
        };

        let composed = compose_capability_chain_proofs(&first, &second).unwrap();
        assert_eq!(composed.steps.len(), 2);
        assert_eq!(
            composed
                .steps
                .iter()
                .map(|step| step.step_index)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert_eq!(
            composed.plan.steps,
            vec![
                "expression_simplification".to_string(),
                "evaluate_simplified_expression".to_string()
            ]
        );
        assert_eq!(composed.final_artifacts, vec!["value"]);
        assert!(composed.replay_verified);

        let mut graph = CapabilityChainProofIndex::default();
        graph.insert(first.clone()).unwrap();
        graph.insert(second.clone()).unwrap();
        let reused = graph
            .search_composed_proof(
                &BTreeSet::from(["raw-expression".to_string()]),
                "value",
                2,
            )
            .unwrap();
        assert_eq!(reused.final_artifacts, vec!["value"]);
        assert_eq!(reused.steps.len(), 2);

        let mut policy_graph = CapabilityChainProofIndex::default();
        policy_graph.insert(first.clone()).unwrap();
        policy_graph.insert(second.clone()).unwrap();
        policy_graph.insert(composed.clone()).unwrap();
        let strict_policy = VerifiedArtifactPolicy {
            minimum_proof_steps: 2,
            ..VerifiedArtifactPolicy::default()
        };
        let policy_reused = policy_graph
            .search_composed_proof_with_policy(
                &BTreeSet::from(["raw-expression".to_string()]),
                "value",
                2,
                &strict_policy,
            )
            .unwrap();
        assert_eq!(policy_reused.steps.len(), 2);
        let synthesis = policy_graph
            .synthesize_or_plan(
                &BTreeSet::from(["raw-expression".to_string()]),
                "value",
                2,
                &strict_policy,
                None,
            )
            .unwrap();
        assert!(matches!(
            synthesis.source,
            CapabilityChainProofSynthesisSource::ReusedProof(_)
        ));
        let pending_plan = CapabilityChainPlan {
            goal: CapabilityIoType::ExactValue,
            steps: vec!["new_capability".into(), "verification".into()],
        };
        let pending = CapabilityChainProofIndex::default()
            .synthesize_or_plan(
                &BTreeSet::new(),
                "new-value",
                1,
                &strict_policy,
                Some(pending_plan.clone()),
            )
            .unwrap();
        assert_eq!(
            pending.source,
            CapabilityChainProofSynthesisSource::CapabilityPlanPending(pending_plan)
        );
        let mixed_plan = CapabilityChainPlan {
            goal: CapabilityIoType::ExactValue,
            steps: vec!["new_capability".into(), "verification".into()],
        };
        let mixed = policy_graph
            .synthesize_mixed_prefix_plan(
                &first,
                vec!["simplified-expression".into()],
                "new-value",
                &strict_policy,
                mixed_plan.clone(),
            )
            .unwrap();
        assert!(matches!(
            mixed.source,
            CapabilityChainProofSynthesisSource::MixedPrefixPlanPending { .. }
        ));
        let mut mixed_ledger = CapabilityChainExecutionLedger::default();
        mixed_ledger
            .start("mixed-execution-1", mixed_plan.clone())
            .unwrap();
        mixed_ledger
            .record_step(
                "mixed-execution-1",
                CapabilityChainStepReceipt {
                    step_index: 0,
                    capability_id: "new_capability".into(),
                    input_artifacts: vec!["simplified-expression".into()],
                    output_artifacts: vec!["intermediate".into()],
                    verification_receipt: "new capability replay".into(),
                },
            )
            .unwrap();
        mixed_ledger
            .record_step(
                "mixed-execution-1",
                CapabilityChainStepReceipt {
                    step_index: 1,
                    capability_id: "verification".into(),
                    input_artifacts: vec!["intermediate".into()],
                    output_artifacts: vec!["new-value".into()],
                    verification_receipt: "new verification replay".into(),
                },
            )
            .unwrap();
        let mixed_execution = mixed_ledger.complete_success("mixed-execution-1").unwrap();
        let validated = validate_mixed_synthesis_execution(
            &mixed.source,
            &mixed_execution,
            "new-value",
            &strict_policy,
        )
        .unwrap();
        assert_eq!(validated.artifact.artifact, "new-value");
        assert_eq!(validated.artifact.proof_trace.steps.len(), 3);
        assert!(validated.artifact.proof_trace.replay_verified);
        let parent = DerivedFact {
            id: "mixed-parent".into(),
            content: "simplified expression exists".into(),
            parent_lineage: Vec::new(),
            provenance: "mixed parent receipt".into(),
            proof_kind: crate::evidence::DerivedProofKind::ExactTransformation,
            precision: crate::evidence::FactPrecision::Exact,
            assumptions: Vec::new(),
            domain: Some("algebra".into()),
        };
        let mut publication_facts = DerivedFactIndex::default();
        let publication = publish_validated_mixed_synthesis(
            &validated,
            &strict_policy,
            "mixed-derived-value",
            "new-value = verified",
            &[&parent],
            "mixed synthesis result",
            &[],
            Some("algebra".into()),
            "mixed-value",
            &mut policy_graph,
            &mut publication_facts,
            &FactPolicy::verified_transformation(),
        )
        .unwrap();
        assert!(policy_graph.get(&publication.proof_fingerprint).is_some());
        assert_eq!(
            publication.fact_publication.index_result,
            FactIndexInsert::Added
        );
        assert!(publication_facts.fact("mixed-derived-value").is_some());
        let short_continuation = CapabilityChainPlan {
            goal: CapabilityIoType::ExactValue,
            steps: vec!["new_capability".into()],
        };
        let long_continuation = CapabilityChainPlan {
            goal: CapabilityIoType::ExactValue,
            steps: vec!["new_capability".into(), "verification".into()],
        };
        let synthesis_preference = policy_graph.rank_mixed_synthesis(
            "new-value",
            &[
                (
                    vec!["simplified-expression".into()],
                    short_continuation,
                ),
                (
                    vec!["simplified-expression".into()],
                    long_continuation,
                ),
            ],
            &strict_policy,
        );
        assert_eq!(synthesis_preference.candidates.len(), 2);
        assert!(!synthesis_preference.ambiguous);
        assert_eq!(
            synthesis_preference
                .candidates
                .first()
                .map(|candidate| candidate.cost.proof_steps),
            Some(2)
        );
        assert!(synthesis_preference.preferred_candidate_id.is_some());

        let mut alternate = second.clone();
        alternate.execution_id = "proof-compose-alternate".into();
        alternate.plan.steps = vec!["alternate_evaluation".into()];
        alternate.steps[0].capability_id = "alternate_evaluation".into();
        graph.insert(alternate).unwrap();
        let mut first_variant = first.clone();
        first_variant.steps[0].verification_receipt = "alternate simplification replay".into();
        graph.insert(first_variant).unwrap();
        let abstractions = graph.abstract_proof_shapes();
        let simplification_shape = abstractions
            .iter()
            .find(|abstraction| {
                abstraction.capabilities == vec!["expression_simplification".to_string()]
            })
            .unwrap();
        assert_eq!(simplification_shape.instances, 2);
        let proposal = graph
            .propose_proof_abstraction(&simplification_shape.pattern_id, 2)
            .unwrap();
        assert_eq!(proposal.pattern.instances, 2);
        let experiment = proposal.assess(3, 0, 0);
        assert!(experiment.pattern_recurred);
        assert!(experiment.safety_preserved);
        assert!(experiment.passed);
        let recommendation = experiment.recommendation();
        assert_eq!(
            recommendation.action,
            ImprovementRecommendationAction::ReviewForApproval
        );
        assert_eq!(recommendation.pattern_id, simplification_shape.pattern_id);
        let generalization = proposal.assess_generalization(3, 2, 0, 0);
        assert!(generalization.passed);
        let insufficient_generalization = proposal.assess_generalization(1, 2, 0, 0);
        assert!(!insufficient_generalization.passed);
        let unsafe_generalization = proposal.assess_generalization(3, 2, 0, 1);
        assert!(!unsafe_generalization.passed);
        let mut abstraction_approvals = CapabilityChainProofAbstractionApprovalLedger::default();
        let approval = abstraction_approvals
            .record(
                "abstraction-approval-1",
                &experiment,
                CapabilityChainProofAbstractionApprovalDecision::Approved,
                "reviewed held-out replay evidence",
            )
            .unwrap();
        assert_eq!(
            approval.decision,
            CapabilityChainProofAbstractionApprovalDecision::Approved
        );
        assert_eq!(abstraction_approvals.receipts().count(), 1);
        let mut abstraction_deployments =
            CapabilityChainProofAbstractionDeploymentLedger::default();
        let prepared = abstraction_deployments
            .prepare(
                "abstraction-deployment-1",
                &approval,
                "abstraction-v0",
                "abstraction-v1",
            )
            .unwrap();
        assert_eq!(
            prepared.status,
            CapabilityChainProofAbstractionDeploymentStatus::Prepared
        );
        assert!(matches!(
            abstraction_deployments.mark_applied("abstraction-deployment-1", ""),
            Err(CapabilityChainProofAbstractionDeploymentRejection::MissingVerificationReceipt)
        ));
        let applied = abstraction_deployments
            .mark_applied(
                "abstraction-deployment-1",
                "held-out abstraction replay receipt",
            )
            .unwrap();
        assert_eq!(
            applied.status,
            CapabilityChainProofAbstractionDeploymentStatus::Applied
        );
        let materialized = applied
            .materialize_capability(&approval)
            .unwrap();
        assert_eq!(
            materialized.capability_id,
            "abstraction-v1"
        );
        assert_eq!(materialized.pattern_id, simplification_shape.pattern_id);
        assert_eq!(materialized.capabilities, vec!["expression_simplification"]);
        assert_eq!(
            materialized.verification_receipt,
            "held-out abstraction replay receipt"
        );
        let registry = CapabilityRegistry::production();
        assert!(registry.get(&materialized.capability_id).is_none());
        let mut evolution_registry = registry.clone();
        let mut candidate = CapabilitySpec::expression_evaluation_v1();
        candidate.id = materialized.capability_id.clone();
        candidate.dependencies = vec!["expression_simplification".into()];
        let novelty = proposal.assess_novelty(&candidate, &evolution_registry);
        assert!(novelty.passed);
        let duplicate_novelty = proposal.assess_novelty(
            &CapabilitySpec::expression_evaluation_v1(),
            &evolution_registry,
        );
        assert!(!duplicate_novelty.passed);
        assert_eq!(
            duplicate_novelty.equivalent_existing_capabilities,
            vec!["expression_evaluation"]
        );
        let value = proposal.assess_value(&candidate, &evolution_registry);
        assert_eq!(
            value.decision,
            CapabilityChainProofAbstractionValueDecision::NotPreferred
        );
        assert!(value
            .alternatives
            .iter()
            .any(|alternative| alternative.capability_id == "expression_evaluation"));
        let priority = rank_proof_abstraction_priorities(vec![
            CapabilityChainProofAbstractionPriorityInput {
                proposal: proposal.clone(),
                value: value.clone(),
            },
        ])
        .unwrap();
        assert_eq!(
            priority.preferred_pattern_ids,
            vec![simplification_shape.pattern_id.clone()]
        );
        assert!(!priority.ambiguous);
        let priority_score = &priority.candidates[0].score;
        assert!(priority_score.expected_gain > 0);
        assert!(priority_score.validation_cost > 0);
        assert_eq!(
            priority_score.efficiency_numerator,
            priority_score.expected_gain
        );
        assert_eq!(
            priority_score.efficiency_denominator,
            priority_score.validation_cost
        );
        let portfolio = select_abstraction_experiment_portfolio(
            &priority,
            priority_score.validation_cost,
        );
        assert_eq!(portfolio.selected_expected_gain, priority_score.expected_gain);
        assert_eq!(
            portfolio.selected_pattern_ids,
            vec![simplification_shape.pattern_id.clone()]
        );
        assert!(!portfolio.ambiguous);
        let empty_portfolio = select_abstraction_experiment_portfolio(&priority, 0);
        assert!(empty_portfolio.selected_pattern_ids.is_empty());
        let tied_priority = CapabilityChainProofAbstractionPriorityReceipt {
            candidates: vec![
                CapabilityChainProofAbstractionPriorityCandidate {
                    pattern_id: "portfolio-a".into(),
                    score: CapabilityChainProofAbstractionPriorityScore {
                        recurrence_signal: 1,
                        value_signal: 9,
                        risk_signal: 0,
                        complexity_penalty: 0,
                        expected_gain: 10,
                        validation_cost: 1,
                        efficiency_numerator: 10,
                        efficiency_denominator: 1,
                        total: 10,
                    },
                },
                CapabilityChainProofAbstractionPriorityCandidate {
                    pattern_id: "portfolio-b".into(),
                    score: CapabilityChainProofAbstractionPriorityScore {
                        recurrence_signal: 1,
                        value_signal: 9,
                        risk_signal: 0,
                        complexity_penalty: 0,
                        expected_gain: 10,
                        validation_cost: 1,
                        efficiency_numerator: 10,
                        efficiency_denominator: 1,
                        total: 10,
                    },
                },
            ],
            preferred_pattern_ids: vec!["portfolio-a".into(), "portfolio-b".into()],
            ambiguous: true,
        };
        let tied_portfolio = select_abstraction_experiment_portfolio(&tied_priority, 1);
        assert!(tied_portfolio.ambiguous);
        assert_eq!(tied_portfolio.tied_portfolios.len(), 2);
        assert!(matches!(
            rank_proof_abstraction_priorities(vec![
                CapabilityChainProofAbstractionPriorityInput {
                    proposal: proposal.clone(),
                    value: value.clone(),
                },
                CapabilityChainProofAbstractionPriorityInput {
                    proposal: proposal.clone(),
                    value: value.clone(),
                },
            ]),
            Err(CapabilityChainProofAbstractionPriorityFailure::DuplicatePattern(_))
        ));
        let mut evolution_ledger = CapabilityRegistryEvolutionLedger::default();
        let prepared_evolution = evolution_ledger
            .prepare_with_generalization_and_novelty(
                "abstraction-evolution-1",
                &materialized,
                candidate.clone(),
                &evolution_registry,
                &generalization,
                &novelty,
            )
            .unwrap();
        assert_eq!(
            prepared_evolution.status,
            CapabilityRegistryEvolutionStatus::Prepared
        );
        assert!(matches!(
            evolution_ledger.apply(
                "abstraction-evolution-1",
                &mut evolution_registry,
                &candidate,
                "",
            ),
            Err(CapabilityRegistryEvolutionRejection::MissingVerificationReceipt)
        ));
        let applied_evolution = evolution_ledger
            .apply(
                "abstraction-evolution-1",
                &mut evolution_registry,
                &candidate,
                "registry integration replay receipt",
            )
            .unwrap();
        assert_eq!(
            applied_evolution.status,
            CapabilityRegistryEvolutionStatus::Applied
        );
        assert_eq!(
            evolution_registry.get(&materialized.capability_id),
            Some(&candidate)
        );
        let rolled_back_evolution = evolution_ledger
            .rollback(
                "abstraction-evolution-1",
                &mut evolution_registry,
                "post-registration regression",
            )
            .unwrap();
        assert_eq!(
            rolled_back_evolution.status,
            CapabilityRegistryEvolutionStatus::RolledBack
        );
        assert!(evolution_registry.get(&materialized.capability_id).is_none());
        let rolled_back = abstraction_deployments
            .rollback("abstraction-deployment-1", "regression detected")
            .unwrap();
        assert_eq!(
            rolled_back.status,
            CapabilityChainProofAbstractionDeploymentStatus::RolledBack
        );
        assert_eq!(abstraction_deployments.receipts().count(), 1);
        assert!(matches!(
            abstraction_deployments.mark_applied("abstraction-deployment-1", "late retry"),
            Err(
                CapabilityChainProofAbstractionDeploymentRejection::DeploymentAlreadyTerminal(
                    CapabilityChainProofAbstractionDeploymentStatus::RolledBack
                )
            )
        ));
        assert!(matches!(
            abstraction_approvals.record(
                "abstraction-approval-1",
                &experiment,
                CapabilityChainProofAbstractionApprovalDecision::Approved,
                "duplicate",
            ),
            Err(CapabilityChainProofAbstractionApprovalRejection::DuplicateApproval(_))
        ));
        let unsafe_experiment = proposal.assess(3, 0, 1);
        assert_eq!(
            unsafe_experiment.recommendation().action,
            ImprovementRecommendationAction::Reject
        );
        let mut abstraction_experiments =
            CapabilityChainProofAbstractionExperimentLedger::default();
        abstraction_experiments
            .record("abstraction-experiment-1", experiment.clone())
            .unwrap();
        abstraction_experiments
            .record("abstraction-experiment-unsafe", unsafe_experiment.clone())
            .unwrap();
        let profile = abstraction_experiments.meta_learning_profile();
        assert_eq!(profile.total_experiments, 2);
        assert_eq!(profile.total_passed, 1);
        assert_eq!(profile.risk_summaries.len(), 1);
        assert_eq!(profile.risk_summaries[0].risk, ImprovementRisk::Medium);
        assert_eq!(profile.risk_summaries[0].attempts, 2);
        assert_eq!(profile.risk_summaries[0].passed, 1);
        assert_eq!(profile.risk_summaries[0].safety_failures, 1);
        assert!(matches!(
            abstraction_experiments.record("abstraction-experiment-1", experiment),
            Err(CapabilityChainProofAbstractionExperimentLedgerRejection::DuplicateExperiment(_))
        ));
        assert!(matches!(
            abstraction_approvals.record(
                "abstraction-approval-unsafe",
                &unsafe_experiment,
                CapabilityChainProofAbstractionApprovalDecision::Approved,
                "must not cross the promotion boundary",
            ),
            Err(
                CapabilityChainProofAbstractionApprovalRejection::RecommendationNotReviewable {
                    ..
                }
            )
        ));
        assert_eq!(
            graph.propose_proof_abstraction(&simplification_shape.pattern_id, 3),
            Err(
                CapabilityChainProofAbstractionProposalFailure::InsufficientInstances {
                    required: 3,
                    actual: 2,
                }
            )
        );
        let preference = graph.rank_goal_proofs("value");
        assert_eq!(preference.candidates.len(), 2);
        assert!(preference.ambiguous);
        assert_eq!(preference.preferred_fingerprint, None);
        let constrained = graph.rank_goal_proofs_with_policy("value", &strict_policy);
        assert!(constrained.accepted.candidates.is_empty());
        assert_eq!(constrained.rejected.len(), 2);
        assert!(constrained.rejected.iter().all(|rejection| matches!(
            rejection.reason,
            VerifiedArtifactPolicyFailure::InsufficientProofSteps { .. }
        )));

        let mut incompatible = second.clone();
        incompatible.steps[0].input_artifacts = vec!["other-expression".into()];
        assert!(matches!(
            compose_capability_chain_proofs(&first, &incompatible),
            Err(CapabilityChainProofFailure::IncompatibleProofs { .. })
        ));
    }

    #[test]
    fn verified_artifact_policy_accepts_complete_replayed_proof() {
        let mut ledger = CapabilityChainExecutionLedger::default();
        let plan = CapabilityChainPlan {
            goal: CapabilityIoType::ExactValue,
            steps: vec!["evaluate_expression".into()],
        };
        ledger.start("policy-proof-1", plan).unwrap();
        ledger
            .record_step(
                "policy-proof-1",
                CapabilityChainStepReceipt {
                    step_index: 0,
                    capability_id: "evaluate_expression".into(),
                    input_artifacts: vec!["expression".into()],
                    output_artifacts: vec!["value".into()],
                    verification_receipt: "evaluation replay".into(),
                },
            )
            .unwrap();
        let execution = ledger.complete_success("policy-proof-1").unwrap();
        let proof = compose_capability_chain_proof(&execution).unwrap();
        let verified = VerifiedArtifact::from_chain("value", proof).unwrap();

        let receipt = VerifiedArtifactPolicy::default().evaluate(&verified).unwrap();
        assert_eq!(receipt.execution_id, "policy-proof-1");
        assert_eq!(receipt.proof_steps, 1);
        assert_eq!(receipt.retrieved_facts, 0);
        assert!(receipt.replay_verified);
    }

    #[test]
    fn verified_artifact_policy_rejects_unverified_or_incomplete_evidence() {
        let mut ledger = CapabilityChainExecutionLedger::default();
        let plan = CapabilityChainPlan {
            goal: CapabilityIoType::ExactValue,
            steps: vec!["evaluate_expression".into()],
        };
        ledger.start("policy-proof-2", plan.clone()).unwrap();
        ledger
            .record_step(
                "policy-proof-2",
                CapabilityChainStepReceipt {
                    step_index: 0,
                    capability_id: "evaluate_expression".into(),
                    input_artifacts: vec!["expression".into()],
                    output_artifacts: vec!["value".into()],
                    verification_receipt: "evaluation replay".into(),
                },
            )
            .unwrap();
        let execution = ledger.complete_success("policy-proof-2").unwrap();
        let mut proof = compose_capability_chain_proof(&execution).unwrap();
        proof.replay_verified = false;
        let unverified = VerifiedArtifact {
            artifact: "value",
            final_verification_receipt: "evaluation replay".into(),
            proof_trace: proof,
        };
        assert_eq!(
            VerifiedArtifactPolicy::default().evaluate(&unverified),
            Err(VerifiedArtifactPolicyFailure::ReplayNotVerified)
        );

        let proof = CapabilityChainProofTrace {
            execution_id: "policy-proof-3".into(),
            plan,
            steps: Vec::new(),
            retrieved_facts: Vec::new(),
            final_artifacts: Vec::new(),
            replay_verified: true,
        };
        let incomplete = VerifiedArtifact {
            artifact: "value",
            proof_trace: proof,
            final_verification_receipt: "evaluation replay".into(),
        };
        assert_eq!(
            VerifiedArtifactPolicy::default().evaluate(&incomplete),
            Err(VerifiedArtifactPolicyFailure::InsufficientProofSteps {
                required: 1,
                actual: 0,
            })
        );
    }

    #[test]
    fn verified_artifact_policy_rejects_unproven_retrieved_fact_inputs() {
        let mut ledger = CapabilityChainExecutionLedger::default();
        let plan = CapabilityChainPlan {
            goal: CapabilityIoType::ExactValue,
            steps: vec!["evaluate_expression".into()],
        };
        ledger.start("policy-retrieval-1", plan).unwrap();
        ledger
            .record_step(
                "policy-retrieval-1",
                CapabilityChainStepReceipt {
                    step_index: 0,
                    capability_id: "evaluate_expression".into(),
                    input_artifacts: vec!["expression".into()],
                    output_artifacts: vec!["value".into()],
                    verification_receipt: "evaluation replay".into(),
                },
            )
            .unwrap();
        let execution = ledger.complete_success("policy-retrieval-1").unwrap();
        let mut proof = compose_capability_chain_proof(&execution).unwrap();
        proof.retrieved_facts.push(DerivedFactProof {
            capability: "evaluate_expression".into(),
            fact_id: "fact-without-retrieval-receipt".into(),
            parent_lineage: vec!["parent".into()],
            retrieval_receipt: None,
        });
        let artifact = VerifiedArtifact::from_chain("value", proof).unwrap();

        assert_eq!(
            VerifiedArtifactPolicy::default().evaluate(&artifact),
            Err(VerifiedArtifactPolicyFailure::MissingFactRetrievalReceipt(
                "fact-without-retrieval-receipt".into(),
            ))
        );
    }

    #[test]
    fn verified_artifact_bridge_preserves_lineage_and_receipt_provenance() {
        let mut ledger = CapabilityChainExecutionLedger::default();
        let plan = CapabilityChainPlan {
            goal: CapabilityIoType::ExactValue,
            steps: vec!["evaluate_expression".into()],
        };
        ledger.start("bridge-proof-1", plan).unwrap();
        ledger
            .record_step(
                "bridge-proof-1",
                CapabilityChainStepReceipt {
                    step_index: 0,
                    capability_id: "evaluate_expression".into(),
                    input_artifacts: vec!["expression".into()],
                    output_artifacts: vec!["value".into()],
                    verification_receipt: "evaluation replay".into(),
                },
            )
            .unwrap();
        let execution = ledger.complete_success("bridge-proof-1").unwrap();
        let artifact = VerifiedArtifact::from_chain(
            "value",
            compose_capability_chain_proof(&execution).unwrap(),
        )
        .unwrap();
        let parent = DerivedFact {
            id: "input-fact".into(),
            content: "input = 2".into(),
            parent_lineage: Vec::new(),
            provenance: "input receipt".into(),
            proof_kind: crate::evidence::DerivedProofKind::ExactTransformation,
            precision: crate::evidence::FactPrecision::Exact,
            assumptions: Vec::new(),
            domain: Some("algebra".into()),
        };

        let (fact, receipt) = derive_fact_from_verified_artifact(
            &artifact,
            &VerifiedArtifactPolicy::default(),
            "derived-value",
            "value = 4",
            &[&parent],
            "expression result",
            &[],
            Some("algebra".into()),
        )
        .unwrap();
        assert_eq!(fact.parent_lineage, vec!["input-fact"]);
        assert!(fact.provenance.contains("bridge-proof-1"));
        assert!(fact.provenance.contains("evaluation replay"));
        assert_eq!(receipt.fact_id, "derived-value");
        assert_eq!(receipt.parent_lineage, vec!["input-fact"]);

        let mut index = DerivedFactIndex::default();
        let publication = publish_verified_artifact_fact(
            &artifact,
            &VerifiedArtifactPolicy::default(),
            "published-value",
            "value = 4",
            &[&parent],
            "expression result",
            &[],
            Some("algebra".into()),
            "value",
            &mut index,
            &FactPolicy::verified_transformation(),
        )
        .unwrap();
        assert_eq!(publication.key, "value");
        assert_eq!(publication.fact_id, "published-value");
        assert_eq!(publication.index_result, FactIndexInsert::Added);
        assert_eq!(index.fact("published-value").unwrap().content, "value = 4");

        assert_eq!(
            derive_fact_from_verified_artifact(
                &artifact,
                &VerifiedArtifactPolicy::default(),
                "parentless",
                "value = 4",
                &[],
                "invalid result",
                &[],
                Some("algebra".into()),
            ),
            Err(VerifiedArtifactFactBridgeFailure::Derivation(
                FactDerivationRejection::NoParents
            ))
        );
    }

    #[test]
    fn failed_chain_produces_proposal_only_repair_candidates() {
        let mut registry = CapabilityRegistry::production();
        let mut alternate = CapabilitySpec::expression_simplification_v1();
        alternate.id = "alternate_simplification".into();
        registry.register(alternate);
        let plan = CapabilityChainPlan {
            goal: CapabilityIoType::SimplifiedExpression,
            steps: vec!["expression_simplification".into()],
        };
        let mut ledger = CapabilityChainExecutionLedger::default();
        ledger.start("chain-repair-1", plan).unwrap();
        let failed = ledger
            .complete_failure("chain-repair-1", 0, "replay mismatch")
            .unwrap();
        let proposals = propose_capability_chain_repairs(
            &failed,
            vec![vec!["alternate_simplification".into()]],
            &registry,
        )
        .unwrap();
        assert_eq!(proposals.len(), 1);
        assert_eq!(
            proposals[0].proposed_plan.steps,
            vec!["alternate_simplification".to_string()]
        );
        assert_eq!(proposals[0].evaluation.cost_delta.steps, 0);
        assert_eq!(proposals[0].failed_step, 0);
        let mut repaired_ledger = CapabilityChainExecutionLedger::default();
        repaired_ledger
            .start("chain-repair-execution-1", proposals[0].proposed_plan.clone())
            .unwrap();
        repaired_ledger
            .record_step(
                "chain-repair-execution-1",
                CapabilityChainStepReceipt {
                    step_index: 0,
                    capability_id: "alternate_simplification".into(),
                    input_artifacts: vec!["raw-expression".into()],
                    output_artifacts: vec!["simplified-expression".into()],
                    verification_receipt: "alternate replay".into(),
                },
            )
            .unwrap();
        let repaired = repaired_ledger
            .complete_success("chain-repair-execution-1")
            .unwrap();
        let validation = validate_capability_chain_repair(&proposals[0], &repaired).unwrap();
        assert_eq!(validation.source_execution_id, "chain-repair-1");
        assert_eq!(validation.repaired_execution_id, "chain-repair-execution-1");
        assert_eq!(validation.verified_steps, 1);
        let mut approvals = CapabilityChainRepairApprovalLedger::default();
        let approval = approvals
            .record(
                "repair-approval-1",
                &proposals[0],
                validation,
                CapabilityChainRepairApprovalDecision::Approved,
            )
            .unwrap();
        let mut installations = CapabilityChainRepairInstallationLedger::default();
        let prepared = installations
            .prepare(
                "repair-installation-1",
                &approval,
                proposals[0].proposed_plan.clone(),
            )
            .unwrap();
        assert_eq!(
            prepared.status,
            CapabilityChainRepairInstallationStatus::Prepared
        );
        let applied = installations
            .mark_applied("repair-installation-1", "installation verified")
            .unwrap();
        assert_eq!(
            applied.status,
            CapabilityChainRepairInstallationStatus::Applied
        );
        assert_eq!(
            installations.rollback("repair-installation-1").unwrap().status,
            CapabilityChainRepairInstallationStatus::RolledBack
        );
    }

    #[test]
    fn active_chain_cannot_generate_repair_proposals() {
        let registry = CapabilityRegistry::production();
        let plan = CapabilityChainPlan {
            goal: CapabilityIoType::SimplifiedExpression,
            steps: vec!["expression_simplification".into()],
        };
        let mut ledger = CapabilityChainExecutionLedger::default();
        let running = ledger.start("chain-repair-2", plan).unwrap();
        assert_eq!(
            propose_capability_chain_repairs(
                &running,
                vec![vec!["expression_simplification".into()]],
                &registry,
            ),
            Err(CapabilityChainRepairFailure::ExecutionNotFailed(
                CapabilityChainExecutionStatus::Running
            ))
        );
    }

    #[test]
    fn chain_repair_preferences_remain_ambiguous_on_equal_cost_repairs() {
        let mut registry = CapabilityRegistry::production();
        for id in ["alternate_a", "alternate_b"] {
            let mut alternate = CapabilitySpec::expression_simplification_v1();
            alternate.id = id.into();
            registry.register(alternate);
        }
        let plan = CapabilityChainPlan {
            goal: CapabilityIoType::SimplifiedExpression,
            steps: vec!["expression_simplification".into()],
        };
        let mut ledger = CapabilityChainExecutionLedger::default();
        ledger.start("chain-repair-3", plan).unwrap();
        let failed = ledger
            .complete_failure("chain-repair-3", 0, "replay mismatch")
            .unwrap();
        let proposals = propose_capability_chain_repairs(
            &failed,
            vec![vec!["alternate_b".into()], vec!["alternate_a".into()]],
            &registry,
        )
        .unwrap();
        let receipt = diagnose_capability_chain_repair_preferences(&proposals, &registry).unwrap();
        assert_eq!(
            receipt.preference,
            CapabilityChainPreference::Ambiguous(vec![
                "chain-repair-3:step0:alternate_a".into(),
                "chain-repair-3:step0:alternate_b".into(),
            ])
        );
        let explanation = explain_capability_chain_repair_preferences(&proposals, &registry)
            .unwrap()
            .unwrap();
        assert_eq!(explanation.execution_id, "chain-repair-3");
        assert_eq!(explanation.failed_step, 0);
        assert_eq!(explanation.original_capability, "expression_simplification");
        assert!(explanation.explanation.preferred_because.is_empty());
    }

    #[test]
    fn function_plan_expands_dependencies_first() {
        let target = assess_prompt(
            "plan-function",
            "Let f(x)=2*x+1. Evaluate f(3).",
            "Math",
            false,
        )
        .target_completion
        .target;
        let plan = plan_target(&target, &CapabilityRegistry::production()).unwrap();
        assert_eq!(plan.selected_capability, "function_application");
        assert_eq!(
            plan.steps
                .iter()
                .map(|step| step.capability_id.as_str())
                .collect::<Vec<_>>(),
            vec!["expression_evaluation", "function_application"]
        );
        assert_eq!(plan.steps[0].version, 1);
        assert!(!plan.steps[0].verifier.is_empty());
        assert_eq!(plan.cost.steps, 2);
        assert_eq!(plan.cost.dependency_edges, 1);
        assert_eq!(plan.selection_reason, PlanSelectionReason::UniqueTargetCapability);
        assert_eq!(
            plan.dependency_proofs,
            vec![DependencyProof {
                capability: "function_application".into(),
                dependency: "expression_evaluation".into(),
            }]
        );
        assert_eq!(
            plan.steps[0].produces,
            vec![CapabilityIoType::ExactValue]
        );
        assert_eq!(
            plan.steps[1].consumes,
            vec![CapabilityIoType::FunctionDefinition, CapabilityIoType::BindingSet]
        );
    }

    #[test]
    fn expression_plan_is_single_step() {
        let target = assess_prompt("plan-expression", "Evaluate 2+3.", "Math", false)
            .target_completion
            .target;
        let plan = plan_target(&target, &CapabilityRegistry::production()).unwrap();
        assert_eq!(plan.selected_capability, "expression_evaluation");
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.cost.steps, 1);
    }

    #[test]
    fn unsupported_target_has_no_plan() {
        let target = assess_prompt("plan-unsupported", "Prove x=x.", "Math", false)
            .target_completion
            .target;
        assert_eq!(
            plan_target(&target, &CapabilityRegistry::production()),
            Err(CapabilityPlanningFailure::NoEligibleCapability)
        );
    }

    #[test]
    fn goal_planner_selects_substitution_from_typed_inputs() {
        let available = BTreeSet::from([
            CapabilityIoType::Expression,
            CapabilityIoType::BindingSet,
        ]);
        let plan = plan_for_goal(
            CapabilityIoType::Expression,
            &available,
            &CapabilityRegistry::production(),
        )
        .unwrap();
        assert_eq!(plan.selected_capability, "substitution");
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.cost.steps, 1);
    }

    #[test]
    fn goal_planner_rejects_missing_inputs() {
        let available = BTreeSet::from([CapabilityIoType::Equation]);
        assert!(matches!(
            plan_for_goal(
                CapabilityIoType::SolutionSet,
                &available,
                &CapabilityRegistry::production()
            ),
            Err(CapabilityPlanningFailure::MissingInputs { .. })
        ));
    }

    #[test]
    fn chain_planner_exposes_equation_normalization_as_typed_preprocessing() {
        let plan = plan_capability_chain(
            CapabilityIoType::NormalizedEquation,
            &BTreeSet::from([CapabilityIoType::Equation]),
            &CapabilityRegistry::production(),
        )
        .unwrap();

        assert_eq!(plan.goal, CapabilityIoType::NormalizedEquation);
        assert_eq!(plan.steps, vec!["equation_normalization"]);
    }

    #[test]
    fn chain_planner_reuses_existing_normalized_equation_without_reprocessing() {
        let plan = plan_capability_chain(
            CapabilityIoType::NormalizedEquation,
            &BTreeSet::from([CapabilityIoType::NormalizedEquation]),
            &CapabilityRegistry::production(),
        )
        .unwrap();

        assert!(plan.steps.is_empty());
    }

    #[test]
    fn generic_verified_artifact_goal_wraps_typed_dataflow_chain() {
        let plan = plan_verified_artifact_goal(
            CapabilityIoType::NormalizedEquation,
            &BTreeSet::from([CapabilityIoType::Equation]),
            &CapabilityRegistry::production(),
            &VerifiedArtifactPolicy::default(),
        )
        .unwrap();

        assert_eq!(plan.goal, CapabilityIoType::VerifiedArtifact);
        assert_eq!(
            plan.steps,
            vec!["equation_normalization", "verified_artifact_wrap"]
        );
    }

    #[test]
    fn generic_verified_artifact_goal_applies_policy_before_returning_plan() {
        let policy = VerifiedArtifactPolicy {
            minimum_proof_steps: 2,
            ..VerifiedArtifactPolicy::default()
        };
        assert_eq!(
            plan_verified_artifact_goal(
                CapabilityIoType::NormalizedEquation,
                &BTreeSet::from([CapabilityIoType::Equation]),
                &CapabilityRegistry::production(),
                &policy,
            ),
            Err(CapabilityChainPlanningFailure::TrustPolicy(
                VerifiedArtifactPlanningFailure::InsufficientProofSteps {
                    required: 2,
                    available: 1,
                }
            ))
        );
    }

    #[test]
    fn chain_planner_composes_normalization_before_linear_solve() {
        let mut registry = CapabilityRegistry::production();
        registry.capabilities.remove("quadratic_equation_solve");

        let plan = plan_capability_chain(
            CapabilityIoType::SolutionSet,
            &BTreeSet::from([
                CapabilityIoType::Equation,
                CapabilityIoType::TargetVariable,
            ]),
            &registry,
        )
        .unwrap();

        assert_eq!(
            plan.steps,
            vec![
                "equation_normalization",
                "linear_equation_solve"
            ]
        );
    }

    #[test]
    fn chain_planner_composes_normalization_before_quadratic_solve() {
        let mut registry = CapabilityRegistry::production();
        registry.capabilities.remove("linear_equation_solve");

        let plan = plan_capability_chain(
            CapabilityIoType::SolutionSet,
            &BTreeSet::from([
                CapabilityIoType::Equation,
                CapabilityIoType::TargetVariable,
            ]),
            &registry,
        )
        .unwrap();

        assert_eq!(
            plan.steps,
            vec![
                "equation_normalization",
                "quadratic_equation_solve"
            ]
        );
    }

    #[test]
    fn chain_planner_routes_normalized_equation_into_classification() {
        let plan = plan_capability_chain(
            CapabilityIoType::EquationClassification,
            &BTreeSet::from([CapabilityIoType::Equation]),
            &CapabilityRegistry::production(),
        )
        .unwrap();

        assert_eq!(
            plan.steps,
            vec![
                "equation_normalization",
                "equation_classification"
            ]
        );
    }

    #[test]
    fn equation_planner_uses_verified_classification_for_linear_routing() {
        let plan = plan_equation_chain(
            "2*x + 3 = 7",
            "x",
            CapabilityIoType::SolutionSet,
            &CapabilityRegistry::production(),
        )
        .unwrap();

        assert_eq!(plan.classification.class, crate::equation_classification::EquationClass::Linear);
        assert_eq!(plan.selected_solver, "linear_equation_solve");
        assert_eq!(
            plan.chain.steps,
            vec![
                "equation_normalization",
                "equation_classification",
                "linear_equation_solve"
            ]
        );
    }

    #[test]
    fn equation_planner_routes_quadratic_without_trying_linear_solver() {
        let plan = plan_equation_chain(
            "x^2 - 4 = 0",
            "x",
            CapabilityIoType::SolutionSet,
            &CapabilityRegistry::production(),
        )
        .unwrap();

        assert_eq!(plan.selected_solver, "quadratic_equation_solve");
        assert!(!plan.chain.steps.contains(&"linear_equation_solve".into()));
    }

    #[test]
    fn equation_planner_adds_solution_set_proof_when_requested() {
        let plan = plan_equation_chain(
            "x^2 - 4 = 0",
            "x",
            CapabilityIoType::VerifiedSolutionSet,
            &CapabilityRegistry::production(),
        )
        .unwrap();

        assert_eq!(plan.selected_solver, "quadratic_equation_solve");
        assert_eq!(
            plan.chain.steps,
            vec![
                "equation_normalization",
                "equation_classification",
                "quadratic_equation_solve",
                "solution_set_verification"
            ]
        );
    }

    #[test]
    fn equation_planner_can_target_generic_verified_artifact() {
        let plan = plan_equation_chain(
            "x^2 - 4 = 0",
            "x",
            CapabilityIoType::VerifiedArtifact,
            &CapabilityRegistry::production(),
        )
        .unwrap();

        assert_eq!(plan.chain.goal, CapabilityIoType::VerifiedArtifact);
        assert_eq!(
            plan.chain.steps.last().map(String::as_str),
            Some("verified_artifact_wrap")
        );
        assert!(plan
            .chain
            .steps
            .contains(&"solution_set_verification".into()));
    }

    #[test]
    fn policy_aware_equation_planner_rejects_insufficient_proof_depth() {
        let policy = VerifiedArtifactPolicy {
            minimum_proof_steps: 5,
            ..VerifiedArtifactPolicy::default()
        };
        assert_eq!(
            plan_equation_chain_with_policy(
                "x^2 - 4 = 0",
                "x",
                CapabilityIoType::VerifiedArtifact,
                &CapabilityRegistry::production(),
                &policy,
            ),
            Err(EquationChainPlanningFailure::TrustPolicy(
                VerifiedArtifactPlanningFailure::InsufficientProofSteps {
                    required: 5,
                    available: 4,
                }
            ))
        );
    }

    #[test]
    fn policy_aware_equation_planner_accepts_replay_and_receipt_requirements() {
        let policy = VerifiedArtifactPolicy {
            require_replay_verified: true,
            minimum_proof_steps: 4,
            require_final_verification_receipt: true,
        };
        let plan = plan_equation_chain_with_policy(
            "x^2 - 4 = 0",
            "x",
            CapabilityIoType::VerifiedArtifact,
            &CapabilityRegistry::production(),
            &policy,
        )
        .unwrap();
        assert_eq!(
            plan.chain.steps,
            vec![
                "equation_normalization",
                "equation_classification",
                "quadratic_equation_solve",
                "solution_set_verification",
                "verified_artifact_wrap"
            ]
        );
    }

    #[test]
    fn equation_planner_abstains_on_unsupported_degree() {
        assert!(matches!(
            plan_equation_chain(
                "x^3 = 1",
                "x",
                CapabilityIoType::SolutionSet,
                &CapabilityRegistry::production(),
            ),
            Err(EquationChainPlanningFailure::Routing(
                EquationRoutingFailure::UnsupportedClass
            ))
        ));
    }

    #[test]
    fn model_plan_composes_unique_constructor_with_expression_evaluation() {
        let plan = plan_model_to_goal(
            "A quantity changes at a constant rate of 3 per interval for 4 intervals. Find the total change.",
            CapabilityIoType::ExactValue,
            &ModelConstructorRegistry::production(),
            &CapabilityRegistry::production(),
        )
        .unwrap();
        assert_eq!(plan.model_step.model_id, "constant_rate_model");
        assert_eq!(plan.model_step.version, 1);
        assert!(plan
            .model_step
            .model_artifacts
            .contains(&ModelArtifactType::Relation));
        assert_eq!(
            plan.capability_plan.selected_capability,
            "expression_evaluation"
        );
        assert_eq!(plan.cost.steps, 2);
        assert_eq!(plan.cost.verification_steps, 2);
    }

    #[test]
    fn model_plan_rejects_text_without_a_unique_model() {
        assert_eq!(
            plan_model_to_goal(
                "A quantity changes at a rate of 3 per interval for 4 intervals. Find the total change.",
                CapabilityIoType::ExactValue,
                &ModelConstructorRegistry::production(),
                &CapabilityRegistry::production(),
            ),
            Err(ModelPlanningFailure::NoEligibleModel)
        );
    }

    #[test]
    fn goal_planner_abstains_on_multiple_exact_value_producers() {
        let available = BTreeSet::from([
            CapabilityIoType::Expression,
            CapabilityIoType::FunctionDefinition,
            CapabilityIoType::BindingSet,
        ]);
        assert!(matches!(
            plan_for_goal(
                CapabilityIoType::ExactValue,
                &available,
                &CapabilityRegistry::production()
            ),
            Err(CapabilityPlanningFailure::AmbiguousCapabilities(_))
        ));
    }

    #[test]
    fn composition_benchmark_substitution_target_is_one_step() {
        let target = assess_prompt(
            "composition-substitution",
            "Substitute x=4 into x^2-1.",
            "Math",
            false,
        )
        .target_completion
        .target;
        let plan = plan_target(&target, &CapabilityRegistry::production()).unwrap();
        assert_eq!(plan.selected_capability, "substitution");
        assert_eq!(plan.cost.steps, 1);
    }

    #[test]
    fn composition_benchmark_linear_target_is_one_step() {
        let target = assess_prompt(
            "composition-linear",
            "Solve for x: 3*x+2=11.",
            "Math",
            false,
        )
        .target_completion
        .target;
        let plan = plan_target(&target, &CapabilityRegistry::production()).unwrap();
        assert_eq!(plan.selected_capability, "linear_equation_solve");
        assert_eq!(plan.cost.steps, 1);
    }

    #[test]
    fn composition_benchmark_rejects_unmodeled_function_equation_chain() {
        let target = assess_prompt(
            "composition-function-equation",
            "Given f(x)=x+5. Find x when f(x)=12.",
            "Math",
            false,
        )
        .target_completion
        .target;
        assert!(plan_target(&target, &CapabilityRegistry::production()).is_err());
    }

    fn derived_fact_registry() -> CapabilityRegistry {
        let mut registry = CapabilityRegistry::default();
        let mut capability = CapabilitySpec::expression_evaluation_v1();
        capability.id = "derived_fact_consumer".into();
        capability.consumes = vec![CapabilityIoType::DerivedFact];
        capability.produces = vec![CapabilityIoType::ExactValue];
        capability.input_requirements = vec![
            InputRequirement::VerifiedDerivedFact,
            InputRequirement::ReplayVerifier,
        ];
        capability.fact_policy = Some(FactPolicy::verified_transformation());
        registry.register(capability);
        registry
    }

    #[test]
    fn context_planner_admits_only_lineage_bearing_derived_facts() {
        let context = ReasoningContext::with_derived_facts(
            BTreeSet::new(),
            vec![DerivedFact {
                id: "derived-1".into(),
                content: "distance = 12".into(),
                parent_lineage: vec!["constant-rate-model".into()],
                provenance: "verified expression evaluation".into(),
                proof_kind: crate::evidence::DerivedProofKind::ExactTransformation,
                precision: crate::evidence::FactPrecision::Exact,
                assumptions: Vec::new(),
                domain: None,
            }],
        );
        let plan = plan_for_goal_with_context(
            CapabilityIoType::ExactValue,
            &context,
            &derived_fact_registry(),
        )
        .unwrap();
        assert_eq!(plan.selected_capability, "derived_fact_consumer");
        assert_eq!(plan.derived_fact_proofs.len(), 1);
        assert_eq!(plan.derived_fact_proofs[0].fact_id, "derived-1");
        assert_eq!(
            plan.derived_fact_proofs[0].parent_lineage,
            vec!["constant-rate-model"]
        );
        assert_eq!(
            plan.derived_fact_proofs[0].retrieval_receipt,
            None
        );
    }

    #[test]
    fn context_planner_rejects_unlineaged_derived_facts() {
        let context = ReasoningContext::with_derived_facts(
            BTreeSet::new(),
            vec![DerivedFact {
                id: "untrusted".into(),
                content: "answer = 42".into(),
                parent_lineage: Vec::new(),
                provenance: "unverified guess".into(),
                proof_kind: crate::evidence::DerivedProofKind::ExactTransformation,
                precision: crate::evidence::FactPrecision::Exact,
                assumptions: Vec::new(),
                domain: None,
            }],
        );
        assert!(matches!(
            plan_for_goal_with_context(
                CapabilityIoType::ExactValue,
                &context,
                &derived_fact_registry(),
            ),
            Err(CapabilityPlanningFailure::InvalidDerivedFacts { capability, .. })
                if capability == "derived_fact_consumer"
        ));
    }

    #[test]
    fn fact_index_planner_reuses_active_derived_knowledge() {
        let fact = DerivedFact {
            id: "indexed-derived".into(),
            content: "distance = 12".into(),
            parent_lineage: vec!["velocity-input".into()],
            provenance: "verified publication".into(),
            proof_kind: crate::evidence::DerivedProofKind::ExactTransformation,
            precision: crate::evidence::FactPrecision::Exact,
            assumptions: Vec::new(),
            domain: Some("algebra".into()),
        };
        let mut index = DerivedFactIndex::default();
        assert_eq!(
            index.insert(
                "distance",
                fact,
                &FactPolicy::verified_transformation(),
            ),
            Ok(FactIndexInsert::Added)
        );
        let plan = plan_for_goal_with_fact_index(
            CapabilityIoType::ExactValue,
            BTreeSet::new(),
            &index,
            &derived_fact_registry(),
        )
        .unwrap();
        assert_eq!(plan.selected_capability, "derived_fact_consumer");
        assert_eq!(plan.derived_fact_proofs[0].fact_id, "indexed-derived");
        assert_eq!(
            plan.derived_fact_proofs[0].retrieval_receipt,
            Some("fact_index_retrieval:distance:indexed-derived".into())
        );
    }

    #[test]
    fn fact_index_planner_rejects_conflicted_knowledge() {
        let make_fact = |id: &str, content: &str| DerivedFact {
            id: id.into(),
            content: content.into(),
            parent_lineage: vec!["input".into()],
            provenance: "verified publication".into(),
            proof_kind: crate::evidence::DerivedProofKind::ExactTransformation,
            precision: crate::evidence::FactPrecision::Exact,
            assumptions: Vec::new(),
            domain: None,
        };
        let mut index = DerivedFactIndex::default();
        assert_eq!(
            index.insert(
                "distance",
                make_fact("distance-a", "distance = 12"),
                &FactPolicy::verified_transformation(),
            ),
            Ok(FactIndexInsert::Added)
        );
        assert!(matches!(
            index.insert(
                "distance",
                make_fact("distance-b", "distance = 14"),
                &FactPolicy::verified_transformation(),
            ),
            Ok(FactIndexInsert::Conflict(_))
        ));
        assert!(matches!(
            plan_for_goal_with_fact_index(
                CapabilityIoType::ExactValue,
                BTreeSet::new(),
                &index,
                &derived_fact_registry(),
            ),
            Err(CapabilityPlanningFailure::FactIndex(
                FactIndexQueryFailure::Conflict(_)
            ))
        ));
    }

    #[test]
    fn plan_becomes_stale_when_required_fact_is_invalidated() {
        let fact = DerivedFact {
            id: "derived-1".into(),
            content: "distance = 12".into(),
            parent_lineage: vec!["constant-rate-model".into()],
            provenance: "verified expression evaluation".into(),
            proof_kind: crate::evidence::DerivedProofKind::ExactTransformation,
            precision: crate::evidence::FactPrecision::Exact,
            assumptions: Vec::new(),
            domain: None,
        };
        let context = ReasoningContext::with_derived_facts(
            BTreeSet::new(),
            vec![fact.clone()],
        );
        let mut index = DerivedFactIndex::default();
        assert_eq!(
            index.insert(
                "distance",
                fact,
                &FactPolicy::verified_transformation(),
            ),
            Ok(FactIndexInsert::Added)
        );
        let plan = plan_for_goal_with_context(
            CapabilityIoType::ExactValue,
            &context,
            &derived_fact_registry(),
        )
        .unwrap();
        assert_eq!(
            plan.lifecycle(&index),
            PlanLifecycle {
                status: PlanStatus::Active,
                invalidations: Vec::new(),
            }
        );

        index
            .invalidate("derived-1", "upstream input corrected", None)
            .unwrap();
        assert_eq!(
            plan.lifecycle(&index),
            PlanLifecycle {
                status: PlanStatus::Stale,
                invalidations: vec![PlanFactInvalidation {
                    fact_id: "derived-1".into(),
                    issue: PlanFactIssue::Inactive(FactStatus::Invalidated),
                }],
            }
        );
    }

    #[test]
    fn plan_lifecycle_is_stale_when_required_fact_is_missing() {
        let context = ReasoningContext::with_derived_facts(
            BTreeSet::new(),
            vec![DerivedFact {
                id: "derived-1".into(),
                content: "distance = 12".into(),
                parent_lineage: vec!["constant-rate-model".into()],
                provenance: "verified expression evaluation".into(),
                proof_kind: crate::evidence::DerivedProofKind::ExactTransformation,
                precision: crate::evidence::FactPrecision::Exact,
                assumptions: Vec::new(),
                domain: None,
            }],
        );
        let plan = plan_for_goal_with_context(
            CapabilityIoType::ExactValue,
            &context,
            &derived_fact_registry(),
        )
        .unwrap();
        assert_eq!(
            plan.lifecycle(&DerivedFactIndex::default()),
            PlanLifecycle {
                status: PlanStatus::Stale,
                invalidations: vec![PlanFactInvalidation {
                    fact_id: "derived-1".into(),
                    issue: PlanFactIssue::Missing,
                }],
            }
        );
    }

    #[test]
    fn plan_dependency_index_reports_stale_dependents() {
        let fact = DerivedFact {
            id: "derived-1".into(),
            content: "distance = 12".into(),
            parent_lineage: vec!["constant-rate-model".into()],
            provenance: "verified expression evaluation".into(),
            proof_kind: crate::evidence::DerivedProofKind::ExactTransformation,
            precision: crate::evidence::FactPrecision::Exact,
            assumptions: Vec::new(),
            domain: None,
        };
        let context = ReasoningContext::with_derived_facts(
            BTreeSet::new(),
            vec![fact.clone()],
        );
        let plan = plan_for_goal_with_context(
            CapabilityIoType::ExactValue,
            &context,
            &derived_fact_registry(),
        )
        .unwrap();
        let mut index = DerivedFactIndex::default();
        index
            .insert("distance", fact, &FactPolicy::verified_transformation())
            .unwrap();

        let mut dependencies = PlanDependencyIndex::default();
        dependencies.register("distance-plan", &plan);
        assert_eq!(
            dependencies.facts_for_plan("distance-plan"),
            vec!["derived-1"]
        );
        assert_eq!(
            dependencies.plans_depending_on("derived-1"),
            vec!["distance-plan"]
        );
        assert!(dependencies.stale_plans(&index).is_empty());

        let mut executions = PlanExecutionLedger::default();
        let running = executions
            .start("attempt-fail", "distance-plan", &plan, &index)
            .unwrap();
        assert_eq!(running.status, PlanExecutionStatus::Running);
        let failed = executions
            .complete_failure("attempt-fail", "derived_fact_consumer", "executor error")
            .unwrap();
        assert_eq!(failed.status, PlanExecutionStatus::Failed);
        assert_eq!(failed.failed_step.as_deref(), Some("derived_fact_consumer"));
        assert_eq!(
            failed.failure_kind,
            Some(ExecutionFailureKind::RuntimeFailure)
        );
        assert_eq!(
            executions.diagnosis("attempt-fail").unwrap().reason,
            "executor error"
        );
        executions
            .start("attempt-fail-2", "distance-plan", &plan, &index)
            .unwrap();
        executions
            .complete_failure("attempt-fail-2", "derived_fact_consumer", "executor error")
            .unwrap();
        assert_eq!(executions.failure_diagnoses().count(), 2);
        assert_eq!(
            executions.failure_patterns(),
            vec![ExecutionFailurePattern {
                plan_id: "distance-plan".into(),
                failed_step: "derived_fact_consumer".into(),
                kind: ExecutionFailureKind::RuntimeFailure,
                occurrences: 2,
                reasons: vec!["executor error".into()],
            }]
        );
        let proposals = executions.improvement_proposals();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].area, ImprovementArea::RuntimeReliability);
        assert!(proposals[0].rationale.contains("runtime"));
        let evaluation = ImprovementEvaluationPolicy::strict()
            .evaluate(&proposals[0])
            .unwrap();
        assert_eq!(evaluation.risk, ImprovementRisk::Medium);
        assert!(!evaluation.validation_requirements.is_empty());
        assert!(evaluation.expected_effect.contains("reproducibility"));
        let experiment = ImprovementExperimentSpec {
            evaluation,
            baseline_failure_occurrences: 2,
            baseline_false_authorizations: 0,
            require_no_new_false_authorizations: true,
        };
        let result = experiment.assess(1, 0);
        assert!(result.failure_reduced);
        assert!(result.safety_preserved);
        assert!(result.passed);
        let mut experiment_ledger = ImprovementExperimentLedger::default();
        let receipt = experiment_ledger
            .record("runtime-reliability-1", experiment.clone(), result.clone())
            .unwrap();
        assert_eq!(
            receipt.decision,
            ImprovementExperimentDecision::Passed
        );
        let recommendation = receipt.recommendation();
        assert_eq!(
            recommendation.action,
            ImprovementRecommendationAction::ReviewForApproval
        );
        assert_eq!(recommendation.experiment_id, "runtime-reliability-1");
        assert!(recommendation.rationale.contains("safety invariant"));
        assert_eq!(
            experiment_ledger
                .recommendation("runtime-reliability-1")
                .unwrap()
                .action,
            ImprovementRecommendationAction::ReviewForApproval
        );
        let mut approval_ledger = ImprovementApprovalLedger::default();
        let approval = approval_ledger
            .record(
                "approval-1",
                &experiment_ledger,
                "runtime-reliability-1",
                ImprovementApprovalDecision::Approved,
                "reviewed benchmark receipt",
            )
            .unwrap();
        assert_eq!(approval.decision, ImprovementApprovalDecision::Approved);
        assert_eq!(approval_ledger.receipts().count(), 1);
        assert!(matches!(
            approval_ledger.record(
                "approval-1",
                &experiment_ledger,
                "runtime-reliability-1",
                ImprovementApprovalDecision::Approved,
                "duplicate",
            ),
            Err(ImprovementApprovalLedgerRejection::DuplicateApproval(_))
        ));
        let mut deployment_ledger = ImprovementDeploymentLedger::default();
        let prepared = deployment_ledger
            .prepare(
                "deployment-1",
                &approval_ledger,
                "approval-1",
                "resolver-v1",
                "resolver-v2",
            )
            .unwrap();
        assert_eq!(
            prepared.status,
            ImprovementDeploymentStatus::Prepared
        );
        assert!(matches!(
            deployment_ledger.mark_applied("deployment-1", ""),
            Err(ImprovementDeploymentLedgerRejection::MissingVerificationReceipt)
        ));
        let applied = deployment_ledger
            .mark_applied("deployment-1", "post-deployment regression suite")
            .unwrap();
        assert_eq!(applied.status, ImprovementDeploymentStatus::Applied);
        let rolled_back = deployment_ledger
            .rollback("deployment-1", "post-deployment regression detected")
            .unwrap();
        assert_eq!(
            rolled_back.status,
            ImprovementDeploymentStatus::RolledBack
        );
        assert_eq!(deployment_ledger.receipts().count(), 1);
        assert!(matches!(
            deployment_ledger.mark_applied("deployment-1", "late retry"),
            Err(ImprovementDeploymentLedgerRejection::DeploymentAlreadyTerminal(
                ImprovementDeploymentStatus::RolledBack
            ))
        ));
        assert_eq!(experiment_ledger.receipts().count(), 1);
        assert!(matches!(
            experiment_ledger.record("runtime-reliability-1", experiment.clone(), result),
            Err(ImprovementExperimentLedgerRejection::DuplicateExperiment(_))
        ));
        let unsafe_result = experiment.assess(1, 1);
        assert!(unsafe_result.failure_reduced);
        assert!(!unsafe_result.safety_preserved);
        assert!(!unsafe_result.passed);
        let failed_receipt = experiment_ledger
            .record("runtime-reliability-2", experiment, unsafe_result)
            .unwrap();
        assert_eq!(
            failed_receipt.decision,
            ImprovementExperimentDecision::Failed
        );
        assert_eq!(
            failed_receipt.recommendation().action,
            ImprovementRecommendationAction::Reject
        );
        assert!(matches!(
            approval_ledger.record(
                "approval-unsafe",
                &experiment_ledger,
                "runtime-reliability-2",
                ImprovementApprovalDecision::Approved,
                "should not pass the approval boundary",
            ),
            Err(ImprovementApprovalLedgerRejection::RecommendationNotReviewable { .. })
        ));
        assert_eq!(experiment_ledger.recommendations().count(), 2);

        let no_benefit = ImprovementExperimentSpec {
            evaluation: failed_receipt.spec.evaluation.clone(),
            baseline_failure_occurrences: 2,
            baseline_false_authorizations: 0,
            require_no_new_false_authorizations: true,
        }
        .assess(2, 0);
        let no_benefit_receipt = experiment_ledger
            .record(
                "runtime-reliability-3",
                ImprovementExperimentSpec {
                    evaluation: failed_receipt.spec.evaluation.clone(),
                    baseline_failure_occurrences: 2,
                    baseline_false_authorizations: 0,
                    require_no_new_false_authorizations: true,
                },
                no_benefit,
            )
            .unwrap();
        assert_eq!(
            no_benefit_receipt.recommendation().action,
            ImprovementRecommendationAction::GatherMoreEvidence
        );
        let mut one_off = proposals[0].clone();
        one_off.pattern.occurrences = 1;
        assert_eq!(
            ImprovementEvaluationPolicy::strict().evaluate(&one_off),
            Err(ImprovementEvaluationRejection::InsufficientRecurrence {
                observed: 1,
                required: 2,
            })
        );

        executions
            .start("attempt-success", "distance-plan", &plan, &index)
            .unwrap();
        let succeeded = executions
            .complete_success(
                "attempt-success",
                "independent replay verified",
                vec!["derived-result".into()],
            )
            .unwrap();
        assert_eq!(succeeded.status, PlanExecutionStatus::Succeeded);
        assert_eq!(
            succeeded.verification_receipt.as_deref(),
            Some("independent replay verified")
        );
        let committed = executions
            .commit_verified_facts(
                "attempt-success",
                vec![(
                    "distance-result".into(),
                    DerivedFact {
                        id: "derived-result".into(),
                        content: "distance = 12".into(),
                        parent_lineage: vec!["derived-1".into()],
                        provenance: "attempt-success replay receipt".into(),
                        proof_kind: crate::evidence::DerivedProofKind::ExactTransformation,
                        precision: crate::evidence::FactPrecision::Exact,
                        assumptions: Vec::new(),
                        domain: None,
                    },
                )],
                &mut index,
                &FactPolicy::verified_transformation(),
            )
            .unwrap();
        assert_eq!(committed, vec![FactIndexInsert::Added]);
        assert_eq!(index.candidates("distance-result").len(), 1);
        assert!(matches!(
            executions.complete_failure("attempt-success", "step", "late failure"),
            Err(PlanExecutionRejection::AttemptAlreadyTerminal(_))
        ));

        index
            .invalidate("derived-1", "upstream input corrected", None)
            .unwrap();
        let stale = dependencies.stale_plans(&index);
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].0, "distance-plan");
        assert_eq!(stale[0].1.status, PlanStatus::Stale);
        assert_eq!(
            stale[0].1.invalidations[0].issue,
            PlanFactIssue::Inactive(FactStatus::Invalidated)
        );
        assert_eq!(
            index.lifecycle("derived-result").unwrap().status,
            FactStatus::Invalidated
        );
        assert!(matches!(
            executions.start("attempt-stale", "distance-plan", &plan, &index),
            Err(PlanExecutionRejection::StalePlan(_))
        ));
    }

    #[test]
    fn stale_plan_produces_active_repair_candidate_without_execution() {
        let make_fact = |id: &str, content: &str| DerivedFact {
            id: id.into(),
            content: content.into(),
            parent_lineage: vec!["constant-rate-model".into()],
            provenance: "verified expression evaluation".into(),
            proof_kind: crate::evidence::DerivedProofKind::ExactTransformation,
            precision: crate::evidence::FactPrecision::Exact,
            assumptions: Vec::new(),
            domain: None,
        };
        let old_fact = make_fact("derived-old", "distance = 12");
        let new_fact = make_fact("derived-new", "distance = 15");
        let initial_context =
            ReasoningContext::with_derived_facts(BTreeSet::new(), vec![old_fact.clone()]);
        let mut index = DerivedFactIndex::default();
        index
            .insert(
                "distance-old",
                old_fact.clone(),
                &FactPolicy::verified_transformation(),
            )
            .unwrap();
        index
            .insert(
                "distance-new",
                new_fact.clone(),
                &FactPolicy::verified_transformation(),
            )
            .unwrap();
        let plan = plan_for_goal_with_context(
            CapabilityIoType::ExactValue,
            &initial_context,
            &derived_fact_registry(),
        )
        .unwrap();
        assert!(matches!(
            replan_stale_plan(
                "distance-plan",
                &plan,
                &initial_context,
                &index,
                &derived_fact_registry(),
            ),
            Err(PlanRepairFailure::PlanStillActive)
        ));

        index
            .invalidate("derived-old", "upstream input corrected", None)
            .unwrap();
        let repair_context = ReasoningContext::with_derived_facts(
            BTreeSet::new(),
            vec![old_fact.clone(), new_fact.clone()],
        );
        let candidate = replan_stale_plan(
            "distance-plan",
            &plan,
            &repair_context,
            &index,
            &derived_fact_registry(),
        )
        .unwrap();
        assert_eq!(candidate.plan_id, "distance-plan");
        assert_eq!(candidate.stale_plan.status, PlanStatus::Stale);
        assert_eq!(candidate.replacement.lifecycle(&index).status, PlanStatus::Active);
        assert_eq!(
            candidate.replacement.derived_fact_proofs[0].fact_id,
            "derived-new"
        );
        let evaluation = candidate.evaluate_against(&plan);
        assert_eq!(evaluation.plan_id, "distance-plan");
        assert_eq!(evaluation.cost_delta.steps, 0);
        assert!(evaluation.added_capabilities.is_empty());
        assert!(evaluation.removed_capabilities.is_empty());
        assert_eq!(evaluation.invalidated_fact_ids, vec!["derived-old"]);
        assert_eq!(evaluation.replacement_fact_ids, vec!["derived-new"]);

        let policy = RepairDecisionPolicy::strict();
        let decision = policy.evaluate(&plan, &candidate, &index);
        assert!(decision.is_accepted());
        assert!(decision.rejections.is_empty());

        let mut dependencies = PlanDependencyIndex::default();
        dependencies.register("distance-plan", &plan);
        let receipt = dependencies
            .install_repair("distance-plan", &plan, &candidate, &decision, &index)
            .unwrap();
        assert_eq!(receipt.old_fact_ids, vec!["derived-old"]);
        assert_eq!(receipt.replacement_fact_ids, vec!["derived-new"]);
        assert_eq!(
            dependencies.facts_for_plan("distance-plan"),
            vec!["derived-new"]
        );
        assert_eq!(dependencies.replacement_history().len(), 1);
        assert_eq!(
            dependencies.lifecycle("distance-plan", &index).unwrap().status,
            PlanStatus::Active
        );

        let mut expensive = candidate.clone();
        expensive.replacement.cost.steps += 1;
        let rejected = policy.evaluate(&plan, &expensive, &index);
        assert!(!rejected.is_accepted());
        assert!(rejected
            .rejections
            .contains(&RepairDecisionRejection::CostIncrease(
                PlanCostDelta {
                    steps: 1,
                    dependency_edges: 0,
                    verification_steps: 0,
                }
            )));
    }
}
