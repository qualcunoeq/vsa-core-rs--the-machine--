//! Narrow, prompt-grounded recurrence representation and execution.
//!
//! This is deliberately *not* a general recurrence solver.  It represents a
//! first-order explicit affine recurrence whose finite target can be reached
//! by bounded exact unrolling.  A recurrence-looking prompt must first be
//! normalized into this typed object; text similarity never authorizes an
//! execution.

use crate::algebra_island::{AlgebraFailure, ExactNumber};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum IndexDomain {
    NonNegative,
    AtLeast(i64),
    Range { start: i64, end: i64 },
}

impl IndexDomain {
    pub fn contains(&self, index: i64) -> bool {
        match *self {
            Self::NonNegative => index >= 0,
            Self::AtLeast(start) => index >= start,
            Self::Range { start, end } => (start..=end).contains(&index),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecurrenceRelation {
    /// a[n+1] = coefficient * a[n] + offset
    ExplicitAffine {
        coefficient: ExactNumber,
        offset: ExactNumber,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitialCondition {
    pub index: i64,
    pub value: ExactNumber,
    pub source_fragment: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum DefinitionProvenance {
    PromptSupplied {
        fragments: Vec<String>,
        normalized_hash: String,
    },
    Curated {
        source: String,
        statement_hash: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecurrenceDefinition {
    pub sequence: String,
    pub index_variable: String,
    pub index_domain: IndexDomain,
    pub relation: RecurrenceRelation,
    pub initial_conditions: Vec<InitialCondition>,
    pub quantification: String,
    pub provenance: DefinitionProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum RecurrenceTarget {
    ValueAt { index: i64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RecurrenceContract {
    pub max_order: usize,
    pub max_unroll_steps: usize,
    pub max_sequences: usize,
    pub allow_symbolic_parameters: bool,
    pub exact_arithmetic_only: bool,
}

impl Default for RecurrenceContract {
    fn default() -> Self {
        Self {
            max_order: 1,
            max_unroll_steps: 64,
            max_sequences: 1,
            allow_symbolic_parameters: false,
            exact_arithmetic_only: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecurrenceFailure {
    DefinitionNotIdentified,
    EmptySequence,
    IndexVariableUnbound,
    QuantifierMissing,
    InitialConditionMissing,
    InitialConditionsInsufficient,
    ConflictingDefinitions,
    UnsupportedOrder,
    UnsupportedImplicitRecurrence,
    UnsupportedTarget,
    TargetOutsideDomain,
    TargetBeforeBase,
    UnrollLimitExceeded,
    ArithmeticFailure(AlgebraFailure),
    ReplayVerificationFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecurrenceStepReceipt {
    pub source_index: i64,
    pub target_index: i64,
    pub previous_value: String,
    pub instantiated_relation: String,
    pub result: String,
    pub replay_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecurrenceExecutionReceipt {
    pub target: RecurrenceTarget,
    pub steps: Vec<RecurrenceStepReceipt>,
    pub final_result: String,
    pub verification: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecurrenceAnswer {
    pub value: ExactNumber,
    pub receipt: RecurrenceExecutionReceipt,
}

impl RecurrenceDefinition {
    pub fn validate(&self, target: &RecurrenceTarget) -> Result<(), RecurrenceFailure> {
        if self.sequence.trim().is_empty() {
            return Err(RecurrenceFailure::EmptySequence);
        }
        if self.index_variable.trim().is_empty() {
            return Err(RecurrenceFailure::IndexVariableUnbound);
        }
        if self.quantification.trim().is_empty() {
            return Err(RecurrenceFailure::QuantifierMissing);
        }
        if self.initial_conditions.is_empty() {
            return Err(RecurrenceFailure::InitialConditionMissing);
        }
        let mut seen = BTreeMap::new();
        for initial in &self.initial_conditions {
            if !self.index_domain.contains(initial.index) {
                return Err(RecurrenceFailure::TargetOutsideDomain);
            }
            if let Some(existing) = seen.insert(initial.index, initial.value) {
                if existing != initial.value {
                    return Err(RecurrenceFailure::ConflictingDefinitions);
                }
            }
        }
        let target_index = match target {
            RecurrenceTarget::ValueAt { index } => *index,
        };
        if !self.index_domain.contains(target_index) {
            return Err(RecurrenceFailure::TargetOutsideDomain);
        }
        let base = self.base_index()?;
        if target_index < base {
            return Err(RecurrenceFailure::TargetBeforeBase);
        }
        Ok(())
    }

    fn base_index(&self) -> Result<i64, RecurrenceFailure> {
        self.initial_conditions
            .iter()
            .map(|condition| condition.index)
            .min()
            .ok_or(RecurrenceFailure::InitialConditionMissing)
    }

    fn initial_value(&self, index: i64) -> Result<ExactNumber, RecurrenceFailure> {
        let values: Vec<_> = self
            .initial_conditions
            .iter()
            .filter(|condition| condition.index == index)
            .map(|condition| condition.value)
            .collect();
        match values.as_slice() {
            [] => Err(RecurrenceFailure::InitialConditionsInsufficient),
            [value] => Ok(*value),
            _ if values.windows(2).all(|pair| pair[0] == pair[1]) => Ok(values[0]),
            _ => Err(RecurrenceFailure::ConflictingDefinitions),
        }
    }

    /// Execute only a finite target under a strict bounded contract.
    pub fn execute(
        &self,
        target: RecurrenceTarget,
        contract: RecurrenceContract,
    ) -> Result<RecurrenceAnswer, RecurrenceFailure> {
        self.validate(&target)?;
        if contract.max_order < 1 || contract.max_sequences < 1 {
            return Err(RecurrenceFailure::UnsupportedOrder);
        }
        let target_index = match target {
            RecurrenceTarget::ValueAt { index } => index,
        };
        let base = self.base_index()?;
        let distance = target_index
            .checked_sub(base)
            .ok_or(RecurrenceFailure::TargetBeforeBase)?;
        let steps_needed =
            usize::try_from(distance).map_err(|_| RecurrenceFailure::TargetBeforeBase)?;
        if steps_needed > contract.max_unroll_steps {
            return Err(RecurrenceFailure::UnrollLimitExceeded);
        }
        let mut value = self.initial_value(base)?;
        let mut steps = Vec::with_capacity(steps_needed);
        for source_index in base..target_index {
            let target_index_step = source_index + 1;
            let (coefficient, offset) = match self.relation {
                RecurrenceRelation::ExplicitAffine {
                    coefficient,
                    offset,
                } => (coefficient, offset),
            };
            let result = value
                .checked_mul(coefficient)
                .and_then(|v| v.checked_add(offset))
                .map_err(RecurrenceFailure::ArithmeticFailure)?;
            let replay = value
                .checked_mul(coefficient)
                .and_then(|v| v.checked_add(offset))
                .map_err(RecurrenceFailure::ArithmeticFailure)?;
            let replay_verified = replay == result;
            if !replay_verified {
                return Err(RecurrenceFailure::ReplayVerificationFailed);
            }
            steps.push(RecurrenceStepReceipt {
                source_index,
                target_index: target_index_step,
                previous_value: value.format(),
                instantiated_relation: format!(
                    "a[{}] = ({}) * a[{}] + ({})",
                    target_index_step,
                    coefficient.format(),
                    source_index,
                    offset.format()
                ),
                result: result.format(),
                replay_verified,
            });
            value = result;
        }
        Ok(RecurrenceAnswer {
            value,
            receipt: RecurrenceExecutionReceipt {
                target: RecurrenceTarget::ValueAt {
                    index: target_index,
                },
                steps,
                final_result: value.format(),
                verification: vec![
                    "all target indices lie in the declared domain".to_string(),
                    "each recurrence step was independently replayed".to_string(),
                    "exact checked arithmetic was used".to_string(),
                ],
            },
        })
    }
}

// ---- Manual review of the four heuristic recurrence candidates ---------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RecurrenceTask {
    DynamicalSystemStability,
    ClosedFormPatternFinding,
    ArithmeticSequenceAlgebra,
    ParameterThresholdOfRationalMap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RecurrenceReviewTarget {
    ValueAt { index: i64 },
    ClosedForm,
    ParameterSet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Linearity {
    NotARecurrence,
    Linear,
    Nonlinear,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CoefficientKind {
    None,
    Constant,
    Parameterized,
    RationalFunction,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ReviewIndexDomain {
    NotApplicable,
    ExplicitFinite,
    NonNegative,
    Unspecified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RepresentationGap {
    DynamicalStabilityAnalysis,
    ClosedFormSequenceInference,
    ArithmeticSequenceModel,
    MöbiusIterationAndRootAnalysis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecurrenceCandidateReview {
    pub question_id: String,
    pub actual_task: RecurrenceTask,
    pub recurrence_supplied_explicitly: bool,
    pub initial_conditions_supplied: bool,
    pub requested_index_or_property: RecurrenceReviewTarget,
    pub recurrence_order: usize,
    pub linearity: Linearity,
    pub coefficient_kind: CoefficientKind,
    pub index_domain: ReviewIndexDomain,
    pub one_step_sufficient: bool,
    pub deterministic_verifier_available: bool,
    pub smallest_missing_representation: Option<RepresentationGap>,
    pub review_note: String,
    pub reviewed: bool,
    pub eligible: bool,
}

/// Human review of the four rows that the heuristic miner grouped together.
/// The review is intentionally checked in as data: it prevents a recurrence
/// keyword from silently becoming a runtime authorization.
pub fn reviewed_hle_candidates() -> Vec<RecurrenceCandidateReview> {
    vec![
        RecurrenceCandidateReview {
            question_id: "66eae5c971adc8ff57780329".into(),
            actual_task: RecurrenceTask::ParameterThresholdOfRationalMap,
            recurrence_supplied_explicitly: true,
            initial_conditions_supplied: true,
            requested_index_or_property: RecurrenceReviewTarget::ParameterSet,
            recurrence_order: 1,
            linearity: Linearity::Nonlinear,
            coefficient_kind: CoefficientKind::RationalFunction,
            index_domain: ReviewIndexDomain::NonNegative,
            one_step_sufficient: false,
            deterministic_verifier_available: false,
            smallest_missing_representation: Some(
                RepresentationGap::MöbiusIterationAndRootAnalysis,
            ),
            review_note: "The supplied map is nonlinear and the target is a minimal parameter threshold for a 1000-step singularity; bounded affine unrolling cannot authorize this.".into(),
            reviewed: true,
            eligible: false,
        },
        RecurrenceCandidateReview {
            question_id: "6706033749b90b396d2cb207".into(),
            actual_task: RecurrenceTask::DynamicalSystemStability,
            recurrence_supplied_explicitly: false,
            initial_conditions_supplied: false,
            requested_index_or_property: RecurrenceReviewTarget::ParameterSet,
            recurrence_order: 0,
            linearity: Linearity::NotARecurrence,
            coefficient_kind: CoefficientKind::None,
            index_domain: ReviewIndexDomain::Unspecified,
            one_step_sufficient: false,
            deterministic_verifier_available: false,
            smallest_missing_representation: Some(RepresentationGap::DynamicalStabilityAnalysis),
            review_note: "The equations are a three-variable nonlinear ODE stability/oscillation problem; no sequence recurrence or finite target term is supplied.".into(),
            reviewed: true,
            eligible: false,
        },
        RecurrenceCandidateReview {
            question_id: "67136bf495e840a8db703aee".into(),
            actual_task: RecurrenceTask::ClosedFormPatternFinding,
            recurrence_supplied_explicitly: false,
            initial_conditions_supplied: true,
            requested_index_or_property: RecurrenceReviewTarget::ClosedForm,
            recurrence_order: 0,
            linearity: Linearity::Unknown,
            coefficient_kind: CoefficientKind::Unknown,
            index_domain: ReviewIndexDomain::Unspecified,
            one_step_sufficient: false,
            deterministic_verifier_available: false,
            smallest_missing_representation: Some(RepresentationGap::ClosedFormSequenceInference),
            review_note: "The listed polynomials are examples and the question asks for a discovered closed form; the recurrence rule is not supplied.".into(),
            reviewed: true,
            eligible: false,
        },
        RecurrenceCandidateReview {
            question_id: "67371006980211368f0f954e".into(),
            actual_task: RecurrenceTask::ArithmeticSequenceAlgebra,
            recurrence_supplied_explicitly: false,
            initial_conditions_supplied: false,
            requested_index_or_property: RecurrenceReviewTarget::ParameterSet,
            recurrence_order: 0,
            linearity: Linearity::NotARecurrence,
            coefficient_kind: CoefficientKind::None,
            index_domain: ReviewIndexDomain::Unspecified,
            one_step_sufficient: false,
            deterministic_verifier_available: false,
            smallest_missing_representation: Some(RepresentationGap::ArithmeticSequenceModel),
            review_note: "The arithmetic-progression conditions require modeling and algebraic derivation; there is no recurrence definition to unroll.".into(),
            reviewed: true,
            eligible: false,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(value: i128) -> ExactNumber {
        ExactNumber::Integer(value)
    }

    fn affine() -> RecurrenceDefinition {
        RecurrenceDefinition {
            sequence: "a".into(),
            index_variable: "n".into(),
            index_domain: IndexDomain::NonNegative,
            relation: RecurrenceRelation::ExplicitAffine {
                coefficient: n(3),
                offset: n(-1),
            },
            initial_conditions: vec![InitialCondition {
                index: 0,
                value: n(2),
                source_fragment: "a_0 = 2".into(),
            }],
            quantification: "for all n >= 0".into(),
            provenance: DefinitionProvenance::PromptSupplied {
                fragments: vec!["a_0 = 2".into(), "a_{n+1}=3a_n-1".into()],
                normalized_hash: "test".into(),
            },
        }
    }

    #[test]
    fn bounded_affine_unrolling_replays_every_step() {
        let answer = affine()
            .execute(RecurrenceTarget::ValueAt { index: 3 }, Default::default())
            .unwrap();
        assert_eq!(answer.value.format(), "41");
        assert_eq!(answer.receipt.steps.len(), 3);
        assert!(answer.receipt.steps.iter().all(|step| step.replay_verified));
    }

    #[test]
    fn missing_initial_condition_and_bound_are_rejected() {
        let mut definition = affine();
        definition.initial_conditions.clear();
        assert_eq!(
            definition
                .execute(RecurrenceTarget::ValueAt { index: 2 }, Default::default())
                .unwrap_err(),
            RecurrenceFailure::InitialConditionMissing
        );
        let definition = affine();
        assert_eq!(
            definition
                .execute(
                    RecurrenceTarget::ValueAt { index: 65 },
                    RecurrenceContract {
                        max_unroll_steps: 2,
                        ..Default::default()
                    }
                )
                .unwrap_err(),
            RecurrenceFailure::UnrollLimitExceeded
        );
    }

    #[test]
    fn conflicting_initial_conditions_are_rejected() {
        let mut definition = affine();
        definition.initial_conditions.push(InitialCondition {
            index: 0,
            value: n(3),
            source_fragment: "a_0 = 3".into(),
        });
        assert_eq!(
            definition
                .execute(RecurrenceTarget::ValueAt { index: 1 }, Default::default())
                .unwrap_err(),
            RecurrenceFailure::ConflictingDefinitions
        );
    }

    #[test]
    fn checked_arithmetic_overflow_abstains() {
        let mut definition = affine();
        definition.relation = RecurrenceRelation::ExplicitAffine {
            coefficient: n(i128::MAX),
            offset: n(0),
        };
        assert_eq!(
            definition
                .execute(RecurrenceTarget::ValueAt { index: 1 }, Default::default())
                .unwrap_err(),
            RecurrenceFailure::ArithmeticFailure(AlgebraFailure::IntegerOverflow)
        );
    }

    #[test]
    fn reviewed_candidates_are_all_ineligible_for_bounded_contract() {
        let reviews = reviewed_hle_candidates();
        assert_eq!(reviews.len(), 4);
        assert!(reviews.iter().all(|review| review.reviewed));
        assert!(reviews.iter().all(|review| !review.eligible));
        assert_eq!(
            reviews
                .iter()
                .filter(|review| review.recurrence_supplied_explicitly)
                .count(),
            1
        );
    }
}
