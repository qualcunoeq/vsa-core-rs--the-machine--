//! Phase 4 restricted implementation synthesis.
//!
//! This module synthesizes immutable, declarative method specifications from
//! already validated capability contracts.  It deliberately does not emit
//! Rust, execute arbitrary code, mutate a registry, or grant capability
//! authority.  The shadow interpreter may call only the existing, named
//! capability formalizers and their replay gates.

use crate::clock_time_contract::{self, ClockBehaviorDefect, ClockDecision};
use crate::finite_state_contract::{self, StateDecision};
use crate::fractional_quantity::{self, FractionalQuantityDecision};
use crate::percentage_quantity::{self, PercentageQuantityDecision};
use crate::quantity_relation::{self, QuantityRelationDecision};
use crate::unit_aware_quantity::{self, UnitQuantityDecision};
use serde::{Deserialize, Serialize};

const MAX_STEPS: usize = 16;
const MAX_DEPTH: usize = 8;
pub const SYNTHESIS_VERSION: &str = "phase4-generic-contract-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArtifactType {
    RawPrompt,
    QuantityRelation,
    UnitQuantity,
    FractionalQuantity,
    PercentageQuantity,
    ClockTimeDuration,
    StateTransitionTrace,
    VerifiedArtifact,
    ReplayReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DslOperation {
    ExtractBinding { name: String },
    RequireBinding { name: String },
    NormalizeNumeric,
    MatchSupportedForm,
    CheckPredicate { predicate: String },
    ConstructTypedRelation,
    InvokeCapability { capability: String },
    VerifyArtifact,
    RejectAmbiguous,
    RejectUnsupported,
    Replay,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodStep {
    pub operation: DslOperation,
    pub input: ArtifactType,
    pub output: ArtifactType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodImplementationSpec {
    pub spec_id: String,
    pub capability_family: String,
    pub input_artifact: ArtifactType,
    pub output_artifact: ArtifactType,
    pub steps: Vec<MethodStep>,
    pub operation_budget: usize,
    pub depth_budget: usize,
    pub diagnostic_only: bool,
    pub trusted_capability: String,
}

pub fn operation_trace(spec: &MethodImplementationSpec) -> Vec<String> {
    spec.steps
        .iter()
        .map(|step| format!("{:?}", step.operation))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpecValidationError {
    EmptySteps,
    WrongInitialInput {
        expected: ArtifactType,
        actual: ArtifactType,
    },
    BrokenHandoff {
        index: usize,
        expected: ArtifactType,
        actual: ArtifactType,
    },
    WrongFinalOutput {
        expected: ArtifactType,
        actual: ArtifactType,
    },
    OperationBudgetExceeded {
        actual: usize,
        budget: usize,
    },
    DepthBudgetExceeded {
        actual: usize,
        budget: usize,
    },
    AuthorityRequired,
    UntrustedCapability(String),
    VerificationMissing,
    ReplayMissing,
    MatchSupportedFormMissing,
    SafetyPredicateMissing,
    UnsafeOperationOrder,
    CapabilityFamilyMismatch {
        expected: String,
        actual: String,
    },
    BindingExtractionMissing,
}

impl MethodImplementationSpec {
    pub fn validate(&self) -> Result<(), Vec<SpecValidationError>> {
        let mut errors = Vec::new();
        if self.steps.is_empty() {
            errors.push(SpecValidationError::EmptySteps);
            return Err(errors);
        }
        if self.steps[0].input != self.input_artifact {
            errors.push(SpecValidationError::WrongInitialInput {
                expected: self.input_artifact,
                actual: self.steps[0].input,
            });
        }
        for (index, pair) in self.steps.windows(2).enumerate() {
            if pair[0].output != pair[1].input {
                errors.push(SpecValidationError::BrokenHandoff {
                    index,
                    expected: pair[0].output,
                    actual: pair[1].input,
                });
            }
        }
        let final_output = self.steps.last().map(|step| step.output);
        if final_output != Some(self.output_artifact) {
            errors.push(SpecValidationError::WrongFinalOutput {
                expected: self.output_artifact,
                actual: final_output.unwrap_or(self.output_artifact),
            });
        }
        let actual = self.steps.len();
        if actual > self.operation_budget || actual > MAX_STEPS {
            errors.push(SpecValidationError::OperationBudgetExceeded {
                actual,
                budget: self.operation_budget.min(MAX_STEPS),
            });
        }
        let logical_depth = self
            .steps
            .iter()
            .filter(|step| {
                matches!(
                    step.operation,
                    DslOperation::InvokeCapability { .. } | DslOperation::ConstructTypedRelation
                )
            })
            .count()
            .max(1);
        if self.depth_budget == 0
            || self.depth_budget > MAX_DEPTH
            || logical_depth > self.depth_budget
        {
            errors.push(SpecValidationError::DepthBudgetExceeded {
                actual: logical_depth,
                budget: self.depth_budget.min(MAX_DEPTH),
            });
        }
        if !self.diagnostic_only {
            errors.push(SpecValidationError::AuthorityRequired);
        }

        let mut verified = false;
        let mut replay = false;
        let mut matched = false;
        let mut predicate = false;
        let mut invoke_index = None;
        let mut verify_index = None;
        let mut replay_index = None;
        for (index, step) in self.steps.iter().enumerate() {
            match &step.operation {
                DslOperation::InvokeCapability { capability }
                    if !trusted_capability(capability) =>
                {
                    errors.push(SpecValidationError::UntrustedCapability(capability.clone()));
                }
                DslOperation::InvokeCapability { capability } => {
                    invoke_index = Some(index);
                    let expected = self.trusted_capability.clone();
                    if expected != *capability {
                        errors.push(SpecValidationError::CapabilityFamilyMismatch {
                            expected,
                            actual: capability.clone(),
                        });
                    }
                }
                DslOperation::MatchSupportedForm => matched = true,
                DslOperation::CheckPredicate { .. } => predicate = true,
                DslOperation::VerifyArtifact => {
                    verified = true;
                    verify_index = Some(index);
                }
                DslOperation::Replay => {
                    replay = true;
                    replay_index = Some(index);
                }
                DslOperation::ExtractBinding { name } if name.trim().is_empty() => {
                    errors.push(SpecValidationError::BindingExtractionMissing);
                }
                _ => {}
            }
        }
        if !matched {
            errors.push(SpecValidationError::MatchSupportedFormMissing);
        }
        if !predicate {
            errors.push(SpecValidationError::SafetyPredicateMissing);
        }
        if let (Some(invoke), Some(match_index), Some(predicate_index)) = (
            invoke_index,
            self.steps
                .iter()
                .position(|step| matches!(step.operation, DslOperation::MatchSupportedForm)),
            self.steps
                .iter()
                .position(|step| matches!(step.operation, DslOperation::CheckPredicate { .. })),
        ) {
            if invoke < match_index || invoke < predicate_index {
                errors.push(SpecValidationError::UnsafeOperationOrder);
            }
        }
        if let Some(invoke) = invoke_index {
            let verification_before_invoke = verify_index.is_some_and(|verify| verify < invoke);
            let replay_before_verification = match (replay_index, verify_index) {
                (Some(replay), Some(verify)) => replay < verify,
                _ => false,
            };
            if verification_before_invoke || replay_before_verification {
                errors.push(SpecValidationError::UnsafeOperationOrder);
            }
        }
        if !verified {
            errors.push(SpecValidationError::VerificationMissing);
        }
        if !replay {
            errors.push(SpecValidationError::ReplayMissing);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

fn trusted_capability(capability: &str) -> bool {
    matches!(
        capability,
        "quantity_relation"
            | "unit_aware_quantity"
            | "fractional_quantity"
            | "percentage_quantity"
            | "clock_time_difference"
            | "finite_state_transition"
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShadowDecision {
    Applicable,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShadowExecution {
    pub family: String,
    pub prompt: String,
    pub decision: ShadowDecision,
    pub artifact_type: Option<ArtifactType>,
    pub observed_duration_minutes: Option<u16>,
    pub observed_final_state: Option<String>,
    pub artifact_replay_verified: bool,
    pub method_replay_verified: bool,
}

impl ShadowExecution {
    pub fn authorized(&self) -> bool {
        self.decision == ShadowDecision::Applicable
    }
}

/// Build the first restricted method specification for a historical family.
/// The result is a plan, not an executable capability or registry entry.
pub fn synthesize_historical_method(family: &str) -> Result<MethodImplementationSpec, String> {
    let (capability, artifact) = match family {
        "QuantityRelationV1" => ("quantity_relation", ArtifactType::QuantityRelation),
        "UnitQuantity" => ("unit_aware_quantity", ArtifactType::UnitQuantity),
        "FractionalQuantity" => ("fractional_quantity", ArtifactType::FractionalQuantity),
        "PercentageQuantityV1" => ("percentage_quantity", ArtifactType::PercentageQuantity),
        other => return Err(format!("unsupported historical family: {other}")),
    };
    let mut steps = vec![
        MethodStep {
            operation: DslOperation::ExtractBinding {
                name: "source_text".into(),
            },
            input: ArtifactType::RawPrompt,
            output: ArtifactType::RawPrompt,
        },
        MethodStep {
            operation: DslOperation::NormalizeNumeric,
            input: ArtifactType::RawPrompt,
            output: ArtifactType::RawPrompt,
        },
        MethodStep {
            operation: DslOperation::MatchSupportedForm,
            input: ArtifactType::RawPrompt,
            output: ArtifactType::RawPrompt,
        },
        MethodStep {
            operation: DslOperation::CheckPredicate {
                predicate: "declared_contract_predicates".into(),
            },
            input: ArtifactType::RawPrompt,
            output: ArtifactType::RawPrompt,
        },
        MethodStep {
            operation: DslOperation::InvokeCapability {
                capability: capability.into(),
            },
            input: ArtifactType::RawPrompt,
            output: artifact,
        },
        MethodStep {
            operation: DslOperation::VerifyArtifact,
            input: artifact,
            output: ArtifactType::VerifiedArtifact,
        },
        MethodStep {
            operation: DslOperation::Replay,
            input: ArtifactType::VerifiedArtifact,
            output: ArtifactType::ReplayReceipt,
        },
    ];
    // Keep a named rejection operation in the specification so the shadow
    // interpreter has an explicit fail-closed outcome for non-applicable
    // cases, without adding an executable fallback.
    steps.insert(
        4,
        MethodStep {
            operation: DslOperation::RejectAmbiguous,
            input: ArtifactType::RawPrompt,
            output: ArtifactType::RawPrompt,
        },
    );
    let spec = MethodImplementationSpec {
        spec_id: format!("phase4-shadow-{}", family.to_ascii_lowercase()),
        capability_family: family.into(),
        input_artifact: ArtifactType::RawPrompt,
        output_artifact: ArtifactType::ReplayReceipt,
        steps,
        operation_budget: 9,
        depth_budget: 8,
        diagnostic_only: true,
        trusted_capability: capability.into(),
    };
    spec.validate()
        .map_err(|errors| format!("invalid synthesized spec: {errors:?}"))?;
    Ok(spec)
}

/// Contract data supplied to generic synthesis.  It contains no
/// implementation branch or executor-specific template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedMethodContract {
    pub contract_id: String,
    pub input_artifact: ArtifactType,
    pub output_artifact: ArtifactType,
    pub required_bindings: Vec<String>,
    pub predicates: Vec<String>,
    pub trusted_capability: String,
    pub operation_budget: usize,
    pub depth_budget: usize,
}

/// Synthesize a method from only typed contract data and the trusted
/// capability graph.  This is intentionally generic: no capability family
/// names or implementation structures are inspected here.
pub fn synthesize_from_contract(
    contract: &ValidatedMethodContract,
) -> Result<MethodImplementationSpec, String> {
    if !trusted_capability(&contract.trusted_capability) {
        return Err(format!(
            "untrusted capability: {}",
            contract.trusted_capability
        ));
    }
    if contract.required_bindings.is_empty() || contract.predicates.is_empty() {
        return Err("contract requires bindings and predicates".into());
    }
    let mut steps = Vec::new();
    for binding in &contract.required_bindings {
        steps.push(MethodStep {
            operation: DslOperation::ExtractBinding {
                name: binding.clone(),
            },
            input: contract.input_artifact,
            output: contract.input_artifact,
        });
        steps.push(MethodStep {
            operation: DslOperation::RequireBinding {
                name: binding.clone(),
            },
            input: contract.input_artifact,
            output: contract.input_artifact,
        });
    }
    steps.push(MethodStep {
        operation: DslOperation::NormalizeNumeric,
        input: contract.input_artifact,
        output: contract.input_artifact,
    });
    steps.push(MethodStep {
        operation: DslOperation::MatchSupportedForm,
        input: contract.input_artifact,
        output: contract.input_artifact,
    });
    for predicate in &contract.predicates {
        steps.push(MethodStep {
            operation: DslOperation::CheckPredicate {
                predicate: predicate.clone(),
            },
            input: contract.input_artifact,
            output: contract.input_artifact,
        });
    }
    steps.push(MethodStep {
        operation: DslOperation::RejectAmbiguous,
        input: contract.input_artifact,
        output: contract.input_artifact,
    });
    steps.push(MethodStep {
        operation: DslOperation::RejectUnsupported,
        input: contract.input_artifact,
        output: contract.input_artifact,
    });
    steps.push(MethodStep {
        operation: DslOperation::InvokeCapability {
            capability: contract.trusted_capability.clone(),
        },
        input: contract.input_artifact,
        output: contract.output_artifact,
    });
    steps.push(MethodStep {
        operation: DslOperation::VerifyArtifact,
        input: contract.output_artifact,
        output: ArtifactType::VerifiedArtifact,
    });
    steps.push(MethodStep {
        operation: DslOperation::Replay,
        input: ArtifactType::VerifiedArtifact,
        output: ArtifactType::ReplayReceipt,
    });
    let spec = MethodImplementationSpec {
        spec_id: format!("synthesized-{}", contract.contract_id),
        capability_family: contract.contract_id.clone(),
        input_artifact: contract.input_artifact,
        output_artifact: ArtifactType::ReplayReceipt,
        steps,
        operation_budget: contract.operation_budget,
        depth_budget: contract.depth_budget,
        diagnostic_only: true,
        trusted_capability: contract.trusted_capability.clone(),
    };
    spec.validate()
        .map_err(|errors| format!("invalid synthesized contract method: {errors:?}"))?;
    Ok(spec)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MethodSpecDefectKind {
    OmitSafetyCheck,
    RemoveSupportedFormBranch,
    WrongBindingExtraction,
    WrongTrustedBridge,
    OmitReplay,
    ExceedBudget,
    ReorderChecksUnsafely,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InjectedMethodDefect {
    pub kind: MethodSpecDefectKind,
    pub expected_failure: SpecValidationError,
    pub spec: MethodImplementationSpec,
}

/// Make a deliberately invalid method specification for the Phase 4
/// reconstruction campaign.  This only clones data; it never mutates the
/// historical specification or a live registry.
pub fn inject_method_defect(
    parent: &MethodImplementationSpec,
    kind: MethodSpecDefectKind,
) -> InjectedMethodDefect {
    let mut spec = parent.clone();
    let expected_failure = match kind {
        MethodSpecDefectKind::OmitSafetyCheck => {
            spec.steps
                .retain(|step| !matches!(step.operation, DslOperation::CheckPredicate { .. }));
            SpecValidationError::SafetyPredicateMissing
        }
        MethodSpecDefectKind::RemoveSupportedFormBranch => {
            spec.steps
                .retain(|step| !matches!(step.operation, DslOperation::MatchSupportedForm));
            SpecValidationError::MatchSupportedFormMissing
        }
        MethodSpecDefectKind::WrongBindingExtraction => {
            if let Some(step) = spec
                .steps
                .iter_mut()
                .find(|step| matches!(step.operation, DslOperation::ExtractBinding { .. }))
            {
                step.operation = DslOperation::ExtractBinding {
                    name: String::new(),
                };
            }
            SpecValidationError::BindingExtractionMissing
        }
        MethodSpecDefectKind::WrongTrustedBridge => {
            if let Some(step) = spec
                .steps
                .iter_mut()
                .find(|step| matches!(step.operation, DslOperation::InvokeCapability { .. }))
            {
                step.operation = DslOperation::InvokeCapability {
                    capability: "unit_aware_quantity".into(),
                };
            }
            SpecValidationError::CapabilityFamilyMismatch {
                expected: spec.trusted_capability.clone(),
                actual: "unit_aware_quantity".into(),
            }
        }
        MethodSpecDefectKind::OmitReplay => {
            spec.steps
                .retain(|step| !matches!(step.operation, DslOperation::Replay));
            SpecValidationError::ReplayMissing
        }
        MethodSpecDefectKind::ExceedBudget => {
            spec.operation_budget = 0;
            SpecValidationError::OperationBudgetExceeded {
                actual: spec.steps.len(),
                budget: 0,
            }
        }
        MethodSpecDefectKind::ReorderChecksUnsafely => {
            let invoke = spec
                .steps
                .iter()
                .position(|step| matches!(step.operation, DslOperation::InvokeCapability { .. }))
                .unwrap();
            let predicate = spec
                .steps
                .iter()
                .position(|step| matches!(step.operation, DslOperation::CheckPredicate { .. }))
                .unwrap();
            spec.steps.swap(invoke, predicate);
            SpecValidationError::UnsafeOperationOrder
        }
    };
    InjectedMethodDefect {
        kind,
        expected_failure,
        spec,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MethodRevisionEdit {
    AddStep { index: usize, step: MethodStep },
    RemoveStep { index: usize },
    ReplaceStep { index: usize, step: MethodStep },
    SetOperationBudget { budget: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodSpecRevision {
    pub parent_spec_id: String,
    pub revision_id: String,
    pub triggering_defect: MethodSpecDefectKind,
    pub edits: Vec<MethodRevisionEdit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MethodRevisionError {
    ParentMismatch,
    InvalidIndex,
    Validation(Vec<SpecValidationError>),
}

pub fn apply_method_revision_sandboxed(
    parent: &MethodImplementationSpec,
    revision: &MethodSpecRevision,
) -> Result<MethodImplementationSpec, MethodRevisionError> {
    if revision.parent_spec_id != parent.spec_id {
        return Err(MethodRevisionError::ParentMismatch);
    }
    let mut revised = parent.clone();
    for edit in &revision.edits {
        match edit {
            MethodRevisionEdit::AddStep { index, step } => {
                if *index > revised.steps.len() {
                    return Err(MethodRevisionError::InvalidIndex);
                }
                revised.steps.insert(*index, step.clone());
            }
            MethodRevisionEdit::RemoveStep { index } => {
                if *index >= revised.steps.len() {
                    return Err(MethodRevisionError::InvalidIndex);
                }
                revised.steps.remove(*index);
            }
            MethodRevisionEdit::ReplaceStep { index, step } => {
                if *index >= revised.steps.len() {
                    return Err(MethodRevisionError::InvalidIndex);
                }
                revised.steps[*index] = step.clone();
            }
            MethodRevisionEdit::SetOperationBudget { budget } => revised.operation_budget = *budget,
        }
    }
    revised.spec_id = revision.revision_id.clone();
    revised
        .validate()
        .map_err(MethodRevisionError::Validation)?;
    Ok(revised)
}

/// Execute a validated method spec in the shadow interpreter.  Only the
/// trusted historical formalizers are callable; all accepted artifacts pass
/// their own replay gate before the method receipt is marked valid.
pub fn shadow_execute(
    spec: &MethodImplementationSpec,
    prompt: &str,
) -> Result<ShadowExecution, String> {
    spec.validate()
        .map_err(|errors| format!("invalid method spec: {errors:?}"))?;
    let mut observed_duration_minutes = None;
    let mut observed_final_state = None;
    let (decision, artifact_type, artifact_replay_verified) = match spec.capability_family.as_str()
    {
        "QuantityRelationV1" => match quantity_relation::formalize(prompt) {
            QuantityRelationDecision::Accepted(artifact) => (
                ShadowDecision::Applicable,
                Some(ArtifactType::QuantityRelation),
                artifact.replay_verified(),
            ),
            QuantityRelationDecision::Ambiguous => (ShadowDecision::Ambiguous, None, false),
            QuantityRelationDecision::Unsupported => (ShadowDecision::Unsupported, None, false),
        },
        "UnitQuantity" => match unit_aware_quantity::formalize(prompt) {
            UnitQuantityDecision::Accepted(artifact) => (
                ShadowDecision::Applicable,
                Some(ArtifactType::UnitQuantity),
                artifact.replay_verified(),
            ),
            UnitQuantityDecision::Ambiguous => (ShadowDecision::Ambiguous, None, false),
            UnitQuantityDecision::Unsupported => (ShadowDecision::Unsupported, None, false),
        },
        "FractionalQuantity" => match fractional_quantity::formalize(prompt) {
            FractionalQuantityDecision::Accepted(artifact) => (
                ShadowDecision::Applicable,
                Some(ArtifactType::FractionalQuantity),
                artifact.replay_verified(),
            ),
            FractionalQuantityDecision::Ambiguous => (ShadowDecision::Ambiguous, None, false),
            FractionalQuantityDecision::Unsupported => (ShadowDecision::Unsupported, None, false),
        },
        "PercentageQuantityV1" => match percentage_quantity::formalize(prompt) {
            PercentageQuantityDecision::Accepted(artifact) => (
                ShadowDecision::Applicable,
                Some(ArtifactType::PercentageQuantity),
                artifact.replay_verified(),
            ),
            PercentageQuantityDecision::Ambiguous => (ShadowDecision::Ambiguous, None, false),
            PercentageQuantityDecision::Unsupported => (ShadowDecision::Unsupported, None, false),
        },
        "ClockTimeDifferenceV1" => match clock_time_contract::formalize(prompt) {
            (ClockDecision::Supported, Some(artifact)) => {
                observed_duration_minutes = Some(artifact.duration_minutes);
                (
                    ShadowDecision::Applicable,
                    Some(ArtifactType::ClockTimeDuration),
                    artifact.replay_verified(),
                )
            }
            (ClockDecision::Ambiguous, _) => (ShadowDecision::Ambiguous, None, false),
            (ClockDecision::Unsupported, _) => (ShadowDecision::Unsupported, None, false),
            (ClockDecision::Supported, None) => (ShadowDecision::Unsupported, None, false),
        },
        "FiniteStateTransitionV1" => match finite_state_contract::formalize(prompt) {
            (StateDecision::Supported, Some(artifact)) => {
                observed_final_state = Some(artifact.final_state.clone());
                (
                    ShadowDecision::Applicable,
                    Some(ArtifactType::StateTransitionTrace),
                    artifact.replay_verified(),
                )
            }
            (StateDecision::Ambiguous, _) => (ShadowDecision::Ambiguous, None, false),
            (StateDecision::Unsupported, _) => (ShadowDecision::Unsupported, None, false),
            (StateDecision::Supported, None) => (ShadowDecision::Unsupported, None, false),
        },
        other => return Err(format!("unsupported shadow family: {other}")),
    };
    // A formalizer's positive classification is never sufficient by itself:
    // a failed artifact replay gate downgrades the shadow decision to a safe
    // refusal before any method receipt can be considered authorized.
    let (decision, artifact_type) =
        if decision == ShadowDecision::Applicable && !artifact_replay_verified {
            (ShadowDecision::Unsupported, None)
        } else {
            (decision, artifact_type)
        };
    let method_replay_verified = decision != ShadowDecision::Applicable || artifact_replay_verified;
    Ok(ShadowExecution {
        family: spec.capability_family.clone(),
        prompt: prompt.into(),
        decision,
        artifact_type,
        observed_duration_minutes,
        observed_final_state,
        artifact_replay_verified,
        method_replay_verified,
    })
}

/// A synthesized method with one semantic fault injected at its trusted
/// invocation boundary.  This is a sandbox-only test object: the underlying
/// immutable `MethodImplementationSpec` is unchanged and the fault cannot be
/// published to the registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaultedClockMethodSpec {
    pub parent: MethodImplementationSpec,
    pub defect: ClockBehaviorDefect,
}

pub fn inject_clock_method_defect(
    parent: &MethodImplementationSpec,
    defect: ClockBehaviorDefect,
) -> Result<FaultedClockMethodSpec, String> {
    parent
        .validate()
        .map_err(|errors| format!("invalid parent method: {errors:?}"))?;
    if parent.capability_family != "ClockTimeDifferenceV1" {
        return Err("clock defects require the unseen clock family".into());
    }
    Ok(FaultedClockMethodSpec {
        parent: parent.clone(),
        defect,
    })
}

/// Execute a faulted clock method without applying the normal fail-closed
/// replay downgrade.  This makes the defect observable to the counterexample
/// evaluator (for example, an omitted replay gate can be reported rather than
/// hidden by the production safety gate).
pub fn shadow_execute_faulted(
    faulted: &FaultedClockMethodSpec,
    prompt: &str,
) -> Result<ShadowExecution, String> {
    faulted
        .parent
        .validate()
        .map_err(|errors| format!("invalid method spec: {errors:?}"))?;
    let (decision, artifact, replay) =
        clock_time_contract::formalize_with_defect(prompt, faulted.defect);
    let decision = match decision {
        ClockDecision::Supported => ShadowDecision::Applicable,
        ClockDecision::Ambiguous => ShadowDecision::Ambiguous,
        ClockDecision::Unsupported => ShadowDecision::Unsupported,
    };
    let observed_duration_minutes = artifact.as_ref().map(|item| item.duration_minutes);
    Ok(ShadowExecution {
        family: faulted.parent.capability_family.clone(),
        prompt: prompt.into(),
        decision,
        artifact_type: artifact.as_ref().map(|_| ArtifactType::ClockTimeDuration),
        observed_duration_minutes,
        observed_final_state: None,
        artifact_replay_verified: replay,
        method_replay_verified: replay,
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockDefectCampaignReport {
    pub cases: usize,
    pub decision_mismatches: usize,
    pub duration_mismatches: usize,
    pub replay_failures: usize,
    pub false_authorizations: usize,
    pub false_denials: usize,
}

impl ClockDefectCampaignReport {
    pub fn detected(&self) -> bool {
        self.decision_mismatches > 0
            || self.duration_mismatches > 0
            || self.replay_failures > 0
            || self.false_authorizations > 0
            || self.false_denials > 0
    }
}

pub fn evaluate_clock_defect(
    faulted: &FaultedClockMethodSpec,
    cases: &[clock_time_contract::ClockCase],
) -> ClockDefectCampaignReport {
    let mut report = ClockDefectCampaignReport {
        cases: cases.len(),
        ..Default::default()
    };
    for case in cases {
        let Ok(observed) = shadow_execute_faulted(faulted, &case.prompt) else {
            report.decision_mismatches += 1;
            continue;
        };
        let expected = match case.expected {
            ClockDecision::Supported => ShadowDecision::Applicable,
            ClockDecision::Ambiguous => ShadowDecision::Ambiguous,
            ClockDecision::Unsupported => ShadowDecision::Unsupported,
        };
        if observed.decision != expected {
            report.decision_mismatches += 1;
        }
        if expected == ShadowDecision::Applicable
            && observed.observed_duration_minutes != case.expected_duration
        {
            report.duration_mismatches += 1;
        }
        if expected == ShadowDecision::Applicable && !observed.method_replay_verified {
            report.replay_failures += 1;
        }
        if observed.authorized() && expected != ShadowDecision::Applicable {
            report.false_authorizations += 1;
        }
        if !observed.authorized() && expected == ShadowDecision::Applicable {
            report.false_denials += 1;
        }
    }
    report
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockMethodRevision {
    pub parent_spec_id: String,
    pub revision_id: String,
    pub defect: ClockBehaviorDefect,
}

pub fn apply_clock_revision_sandboxed(
    faulted: &FaultedClockMethodSpec,
    revision: &ClockMethodRevision,
) -> Result<MethodImplementationSpec, MethodRevisionError> {
    if revision.parent_spec_id != faulted.parent.spec_id {
        return Err(MethodRevisionError::ParentMismatch);
    }
    let mut repaired = faulted.parent.clone();
    repaired.spec_id = revision.revision_id.clone();
    repaired
        .validate()
        .map_err(MethodRevisionError::Validation)?;
    Ok(repaired)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaultedStateMethodSpec {
    pub parent: MethodImplementationSpec,
    pub defect: finite_state_contract::StateBehaviorDefect,
}

pub fn inject_state_method_defect(
    parent: &MethodImplementationSpec,
    defect: finite_state_contract::StateBehaviorDefect,
) -> Result<FaultedStateMethodSpec, String> {
    parent
        .validate()
        .map_err(|errors| format!("invalid parent method: {errors:?}"))?;
    if parent.capability_family != "FiniteStateTransitionV1" {
        return Err("state defects require the finite-state family".into());
    }
    Ok(FaultedStateMethodSpec {
        parent: parent.clone(),
        defect,
    })
}

pub fn shadow_execute_state_faulted(
    faulted: &FaultedStateMethodSpec,
    prompt: &str,
) -> Result<ShadowExecution, String> {
    faulted
        .parent
        .validate()
        .map_err(|errors| format!("invalid method spec: {errors:?}"))?;
    let (decision, artifact, replay) =
        finite_state_contract::formalize_with_defect(prompt, faulted.defect);
    let decision = match decision {
        StateDecision::Supported => ShadowDecision::Applicable,
        StateDecision::Ambiguous => ShadowDecision::Ambiguous,
        StateDecision::Unsupported => ShadowDecision::Unsupported,
    };
    Ok(ShadowExecution {
        family: faulted.parent.capability_family.clone(),
        prompt: prompt.into(),
        decision,
        artifact_type: artifact
            .as_ref()
            .map(|_| ArtifactType::StateTransitionTrace),
        observed_duration_minutes: None,
        observed_final_state: artifact.as_ref().map(|item| item.final_state.clone()),
        artifact_replay_verified: replay,
        method_replay_verified: replay,
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateDefectCampaignReport {
    pub cases: usize,
    pub decision_mismatches: usize,
    pub final_state_mismatches: usize,
    pub replay_failures: usize,
    pub false_authorizations: usize,
    pub false_denials: usize,
}

impl StateDefectCampaignReport {
    pub fn detected(&self) -> bool {
        self.decision_mismatches > 0
            || self.final_state_mismatches > 0
            || self.replay_failures > 0
            || self.false_authorizations > 0
            || self.false_denials > 0
    }
}

pub fn evaluate_state_defect(
    faulted: &FaultedStateMethodSpec,
    cases: &[finite_state_contract::StateCase],
) -> StateDefectCampaignReport {
    let mut report = StateDefectCampaignReport {
        cases: cases.len(),
        ..Default::default()
    };
    for case in cases {
        let Ok(observed) = shadow_execute_state_faulted(faulted, &case.prompt) else {
            report.decision_mismatches += 1;
            continue;
        };
        let expected = match case.expected {
            StateDecision::Supported => ShadowDecision::Applicable,
            StateDecision::Ambiguous => ShadowDecision::Ambiguous,
            StateDecision::Unsupported => ShadowDecision::Unsupported,
        };
        if observed.decision != expected {
            report.decision_mismatches += 1;
        }
        if expected == ShadowDecision::Applicable
            && observed.observed_final_state != case.expected_state
        {
            report.final_state_mismatches += 1;
        }
        if expected == ShadowDecision::Applicable && !observed.method_replay_verified {
            report.replay_failures += 1;
        }
        if observed.authorized() && expected != ShadowDecision::Applicable {
            report.false_authorizations += 1;
        }
        if !observed.authorized() && expected == ShadowDecision::Applicable {
            report.false_denials += 1;
        }
    }
    report
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateMethodRevision {
    pub parent_spec_id: String,
    pub revision_id: String,
    pub defect: finite_state_contract::StateBehaviorDefect,
}

pub fn apply_state_revision_sandboxed(
    faulted: &FaultedStateMethodSpec,
    revision: &StateMethodRevision,
) -> Result<MethodImplementationSpec, MethodRevisionError> {
    if revision.parent_spec_id != faulted.parent.spec_id {
        return Err(MethodRevisionError::ParentMismatch);
    }
    let mut repaired = faulted.parent.clone();
    repaired.spec_id = revision.revision_id.clone();
    repaired
        .validate()
        .map_err(MethodRevisionError::Validation)?;
    Ok(repaired)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoricalCase {
    pub family: String,
    pub prompt: String,
    pub expected: ShadowDecision,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SynthesisCampaignReport {
    pub cases: usize,
    pub correct_decisions: usize,
    pub authorized: usize,
    pub replay_verified: usize,
    pub accepted_replay_verified: usize,
    pub false_authorizations: usize,
    pub false_denials: usize,
    pub invalid_specs: usize,
}

pub fn evaluate_method_spec(
    spec: &MethodImplementationSpec,
    cases: &[HistoricalCase],
) -> SynthesisCampaignReport {
    let mut report = SynthesisCampaignReport {
        cases: cases.len(),
        ..Default::default()
    };
    for case in cases {
        let Ok(result) = shadow_execute(spec, &case.prompt) else {
            report.invalid_specs += 1;
            continue;
        };
        report.correct_decisions += usize::from(result.decision == case.expected);
        report.authorized += usize::from(result.authorized());
        report.replay_verified += usize::from(result.method_replay_verified);
        report.accepted_replay_verified += usize::from(
            result.authorized() && result.artifact_replay_verified && result.method_replay_verified,
        );
        report.false_authorizations +=
            usize::from(result.authorized() && case.expected != ShadowDecision::Applicable);
        report.false_denials +=
            usize::from(!result.authorized() && case.expected == ShadowDecision::Applicable);
    }
    report
}

pub fn evaluate_historical_cases(cases: &[HistoricalCase]) -> SynthesisCampaignReport {
    let mut report = SynthesisCampaignReport {
        cases: cases.len(),
        ..Default::default()
    };
    for family in [
        "QuantityRelationV1",
        "UnitQuantity",
        "FractionalQuantity",
        "PercentageQuantityV1",
    ] {
        let family_cases: Vec<HistoricalCase> = cases
            .iter()
            .filter(|case| case.family == family)
            .cloned()
            .collect();
        if family_cases.is_empty() {
            continue;
        }
        let Ok(spec) = synthesize_historical_method(family) else {
            report.invalid_specs += family_cases.len();
            continue;
        };
        let family_report = evaluate_method_spec(&spec, &family_cases);
        report.correct_decisions += family_report.correct_decisions;
        report.authorized += family_report.authorized;
        report.replay_verified += family_report.replay_verified;
        report.accepted_replay_verified += family_report.accepted_replay_verified;
        report.false_authorizations += family_report.false_authorizations;
        report.false_denials += family_report.false_denials;
        report.invalid_specs += family_report.invalid_specs;
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn historical_cases() -> Vec<HistoricalCase> {
        vec![
            HistoricalCase {
                family: "QuantityRelationV1".into(),
                prompt: "5 notebooks cost 20 dollars. What is the price per notebook?".into(),
                expected: ShadowDecision::Applicable,
            },
            HistoricalCase {
                family: "QuantityRelationV1".into(),
                prompt: "A price changes by 5% each year. What is the final price?".into(),
                expected: ShadowDecision::Unsupported,
            },
            HistoricalCase {
                family: "UnitQuantity".into(),
                prompt: "Convert 3 meters to centimeters using 100 centimeters per meter.".into(),
                expected: ShadowDecision::Applicable,
            },
            HistoricalCase {
                family: "UnitQuantity".into(),
                prompt: "Add 2 meters and 30 centimeters.".into(),
                expected: ShadowDecision::Ambiguous,
            },
            HistoricalCase {
                family: "FractionalQuantity".into(),
                prompt: "What is three quarters of 20?".into(),
                expected: ShadowDecision::Applicable,
            },
            HistoricalCase {
                family: "FractionalQuantity".into(),
                prompt: "There is a 25% probability.".into(),
                expected: ShadowDecision::Unsupported,
            },
            HistoricalCase {
                family: "PercentageQuantityV1".into(),
                prompt: "What is 20% of 50?".into(),
                expected: ShadowDecision::Applicable,
            },
            HistoricalCase {
                family: "PercentageQuantityV1".into(),
                prompt: "A balance grows by 5% each year for 5 years.".into(),
                expected: ShadowDecision::Unsupported,
            },
        ]
    }

    #[test]
    fn historical_specs_are_bounded_and_non_authorizing() {
        for family in [
            "QuantityRelationV1",
            "UnitQuantity",
            "FractionalQuantity",
            "PercentageQuantityV1",
        ] {
            let spec = synthesize_historical_method(family).expect("historical spec");
            assert!(spec.diagnostic_only);
            assert!(spec.validate().is_ok());
            assert!(spec.steps.len() <= MAX_STEPS);
            assert!(spec.steps.iter().all(|step| !matches!(
                step.operation,
                DslOperation::InvokeCapability { ref capability } if !trusted_capability(capability)
            )));
        }
    }

    #[test]
    fn historical_reconstruction_shadow_has_zero_authorization_errors() {
        let report = evaluate_historical_cases(&historical_cases());
        assert_eq!(report.cases, 8);
        assert_eq!(report.correct_decisions, 8);
        assert_eq!(report.false_authorizations, 0);
        assert_eq!(report.false_denials, 0);
        assert_eq!(report.accepted_replay_verified, report.authorized);
        assert_eq!(report.replay_verified, report.cases);
        assert_eq!(report.invalid_specs, 0);
    }

    #[test]
    fn malformed_or_unauthorized_specs_are_rejected() {
        let mut spec = synthesize_historical_method("QuantityRelationV1").unwrap();
        spec.diagnostic_only = false;
        assert!(spec.validate().is_err());
        spec.diagnostic_only = true;
        spec.steps[5].operation = DslOperation::InvokeCapability {
            capability: "arbitrary_code".into(),
        };
        assert!(spec.validate().is_err());
    }

    #[test]
    fn full_frozen_historical_corpora_are_fail_closed() {
        #[derive(serde::Deserialize)]
        struct Cases<T> {
            cases: Vec<T>,
        }
        #[derive(serde::Deserialize)]
        struct BasicCase {
            prompt: String,
            outcome: String,
        }

        fn basic(path: &str, family: &str) -> Vec<HistoricalCase> {
            let corpus: Cases<BasicCase> = serde_json::from_str(path).expect("historical corpus");
            corpus
                .cases
                .into_iter()
                .map(|case| HistoricalCase {
                    family: family.into(),
                    prompt: case.prompt,
                    expected: match case.outcome.as_str() {
                        "supported" => ShadowDecision::Applicable,
                        "ambiguous" => ShadowDecision::Ambiguous,
                        "unsupported" => ShadowDecision::Unsupported,
                        other => panic!("unknown outcome: {other}"),
                    },
                })
                .collect()
        }

        let mut cases = basic(
            include_str!("../data/quantity_relation_v1_expanded.json"),
            "QuantityRelationV1",
        );
        cases.extend(basic(
            include_str!("../data/unit_aware_quantity_v1.json"),
            "UnitQuantity",
        ));
        cases.extend(basic(
            include_str!("../data/fractional_quantity_v1.json"),
            "FractionalQuantity",
        ));
        cases.extend(
            crate::percentage_quantity_proposal::corpus()
                .cases
                .into_iter()
                .map(|case| HistoricalCase {
                    family: "PercentageQuantityV1".into(),
                    prompt: case.prompt,
                    expected: match case.scope {
                        crate::percentage_quantity_proposal::PercentageScope::Supported => {
                            ShadowDecision::Applicable
                        }
                        crate::percentage_quantity_proposal::PercentageScope::Ambiguous => {
                            ShadowDecision::Ambiguous
                        }
                        crate::percentage_quantity_proposal::PercentageScope::Unsupported => {
                            ShadowDecision::Unsupported
                        }
                    },
                }),
        );

        let report = evaluate_historical_cases(&cases);
        for family in [
            "QuantityRelationV1",
            "UnitQuantity",
            "FractionalQuantity",
            "PercentageQuantityV1",
        ] {
            let family_cases: Vec<HistoricalCase> = cases
                .iter()
                .filter(|case| case.family == family)
                .cloned()
                .collect();
            let spec = synthesize_historical_method(family).expect("family spec");
            let family_report = evaluate_method_spec(&spec, &family_cases);
            eprintln!(
                "phase4 family: family={} cases={} correct={} authorized={} accepted_replay={} method_replay={} false_auth={} false_denials={} steps={} depth_budget={} operation_budget={}",
                family,
                family_report.cases,
                family_report.correct_decisions,
                family_report.authorized,
                family_report.accepted_replay_verified,
                family_report.replay_verified,
                family_report.false_authorizations,
                family_report.false_denials,
                spec.steps.len(),
                spec.depth_budget,
                spec.operation_budget,
            );
        }
        eprintln!(
            "phase4 full corpus: cases={} correct={} authorized={} replay={} accepted_replay={} false_auth={} false_denials={} invalid_specs={}",
            report.cases,
            report.correct_decisions,
            report.authorized,
            report.replay_verified,
            report.accepted_replay_verified,
            report.false_authorizations,
            report.false_denials,
            report.invalid_specs,
        );
        assert_eq!(report.invalid_specs, 0);
        assert_eq!(report.false_authorizations, 0);
        assert_eq!(report.accepted_replay_verified, report.authorized);
        assert!(report.replay_verified >= report.cases.saturating_sub(report.invalid_specs));
    }

    #[test]
    fn method_defect_campaign_is_static_and_fail_closed() {
        let spec = synthesize_historical_method("PercentageQuantityV1").unwrap();
        let kinds = [
            MethodSpecDefectKind::OmitSafetyCheck,
            MethodSpecDefectKind::RemoveSupportedFormBranch,
            MethodSpecDefectKind::WrongBindingExtraction,
            MethodSpecDefectKind::WrongTrustedBridge,
            MethodSpecDefectKind::OmitReplay,
            MethodSpecDefectKind::ExceedBudget,
            MethodSpecDefectKind::ReorderChecksUnsafely,
        ];
        for kind in kinds {
            let defect = inject_method_defect(&spec, kind);
            let errors = defect
                .spec
                .validate()
                .expect_err("injected defect must be rejected");
            assert!(
                errors.iter().any(|error| error == &defect.expected_failure),
                "{kind:?}: {errors:?}"
            );
        }
    }

    #[test]
    fn method_revision_is_immutable_and_can_repair_omitted_replay() {
        let parent = synthesize_historical_method("QuantityRelationV1").unwrap();
        let defect = inject_method_defect(&parent, MethodSpecDefectKind::OmitReplay);
        assert!(defect.spec.validate().is_err());
        let revision = MethodSpecRevision {
            parent_spec_id: defect.spec.spec_id.clone(),
            revision_id: "phase4-repaired-replay".into(),
            triggering_defect: MethodSpecDefectKind::OmitReplay,
            edits: vec![MethodRevisionEdit::AddStep {
                index: defect.spec.steps.len(),
                step: MethodStep {
                    operation: DslOperation::Replay,
                    input: ArtifactType::VerifiedArtifact,
                    output: ArtifactType::ReplayReceipt,
                },
            }],
        };
        let repaired =
            apply_method_revision_sandboxed(&defect.spec, &revision).expect("sandbox repair");
        assert!(repaired.validate().is_ok());
        assert_eq!(
            parent.steps.last().map(|step| &step.operation),
            Some(&DslOperation::Replay)
        );
        assert_ne!(parent.spec_id, repaired.spec_id);
    }

    #[test]
    fn unseen_clock_contract_synthesizes_and_preserves_holdout_boundary() {
        let contract = clock_time_contract::contract();
        assert!(contract.validation_errors().is_empty());
        let method_contract = ValidatedMethodContract {
            contract_id: contract.contract_id.clone(),
            input_artifact: ArtifactType::RawPrompt,
            output_artifact: ArtifactType::ClockTimeDuration,
            required_bindings: contract.required_bindings.clone(),
            predicates: contract.predicates.clone(),
            trusted_capability: "clock_time_difference".into(),
            operation_budget: 16,
            depth_budget: 8,
        };
        let spec = synthesize_from_contract(&method_contract).expect("generic unseen synthesis");
        assert!(spec.validate().is_ok());
        assert_eq!(spec.trusted_capability, "clock_time_difference");
        let development: Vec<HistoricalCase> = contract
            .cases
            .iter()
            .filter(|case| case.split == clock_time_contract::ClockSplit::Development)
            .map(|case| HistoricalCase {
                family: contract.contract_id.clone(),
                prompt: case.prompt.clone(),
                expected: match case.expected {
                    clock_time_contract::ClockDecision::Supported => ShadowDecision::Applicable,
                    clock_time_contract::ClockDecision::Ambiguous => ShadowDecision::Ambiguous,
                    clock_time_contract::ClockDecision::Unsupported => ShadowDecision::Unsupported,
                },
            })
            .collect();
        let holdout: Vec<HistoricalCase> = contract
            .cases
            .iter()
            .filter(|case| case.split == clock_time_contract::ClockSplit::Holdout)
            .map(|case| HistoricalCase {
                family: contract.contract_id.clone(),
                prompt: case.prompt.clone(),
                expected: match case.expected {
                    clock_time_contract::ClockDecision::Supported => ShadowDecision::Applicable,
                    clock_time_contract::ClockDecision::Ambiguous => ShadowDecision::Ambiguous,
                    clock_time_contract::ClockDecision::Unsupported => ShadowDecision::Unsupported,
                },
            })
            .collect();
        let development_report = evaluate_method_spec(&spec, &development);
        let holdout_report = evaluate_method_spec(&spec, &holdout);
        for case in development.iter().chain(holdout.iter()) {
            let observed = shadow_execute(&spec, &case.prompt)
                .expect("shadow clock")
                .decision;
            if observed != case.expected {
                eprintln!(
                    "phase4 clock mismatch: prompt={:?} expected={:?} observed={:?}",
                    case.prompt, case.expected, observed
                );
            }
        }
        eprintln!(
            "phase4 unseen clock: synthesis_version={} contract_hash={} dev_hash={} holdout_hash={} dev={}/{} holdout={}/{} holdout_authorized={} holdout_replay={} false_auth={} false_denials={}",
            SYNTHESIS_VERSION,
            contract.release_hash(),
            contract.split_hash(clock_time_contract::ClockSplit::Development),
            contract.split_hash(clock_time_contract::ClockSplit::Holdout),
            development_report.correct_decisions,
            development_report.cases,
            holdout_report.correct_decisions,
            holdout_report.cases,
            holdout_report.authorized,
            holdout_report.accepted_replay_verified,
            holdout_report.false_authorizations,
            holdout_report.false_denials,
        );
        eprintln!(
            "phase4 unseen clock operation trace: {:?}",
            operation_trace(&spec)
        );
        assert_eq!(
            development_report.correct_decisions,
            development_report.cases
        );
        assert_eq!(holdout_report.correct_decisions, holdout_report.cases);
        assert_eq!(holdout_report.false_authorizations, 0);
        assert_eq!(holdout_report.false_denials, 0);
        assert_eq!(
            holdout_report.accepted_replay_verified,
            holdout_report.authorized
        );
    }

    #[test]
    fn unseen_clock_pressure_campaign_finds_and_repairs_all_behavioral_defects() {
        let contract = clock_time_contract::contract();
        let method_contract = ValidatedMethodContract {
            contract_id: contract.contract_id.clone(),
            input_artifact: ArtifactType::RawPrompt,
            output_artifact: ArtifactType::ClockTimeDuration,
            required_bindings: contract.required_bindings.clone(),
            predicates: contract.predicates.clone(),
            trusted_capability: "clock_time_difference".into(),
            operation_budget: 16,
            depth_budget: 8,
        };
        let parent = synthesize_from_contract(&method_contract).expect("generic clock synthesis");
        let pressure = clock_time_contract::pressure_corpus();
        assert_eq!(pressure.len(), 240);
        let mut baseline_correct = 0;
        let mut baseline_false_auth = 0;
        let mut baseline_false_denials = 0;
        for case in &pressure {
            let observed = shadow_execute(&parent, &case.prompt).expect("baseline clock shadow");
            let expected = match case.expected {
                clock_time_contract::ClockDecision::Supported => ShadowDecision::Applicable,
                clock_time_contract::ClockDecision::Ambiguous => ShadowDecision::Ambiguous,
                clock_time_contract::ClockDecision::Unsupported => ShadowDecision::Unsupported,
            };
            let duration_ok = expected != ShadowDecision::Applicable
                || observed.observed_duration_minutes == case.expected_duration;
            if observed.decision == expected && duration_ok && observed.method_replay_verified {
                baseline_correct += 1;
            }
            baseline_false_auth +=
                usize::from(observed.authorized() && expected != ShadowDecision::Applicable);
            baseline_false_denials +=
                usize::from(!observed.authorized() && expected == ShadowDecision::Applicable);
        }
        assert_eq!(baseline_correct, pressure.len());
        assert_eq!(baseline_false_auth, 0);
        assert_eq!(baseline_false_denials, 0);
        eprintln!(
            "phase4 clock pressure: hash={} cases={} supported={} ambiguous={} unsupported={}",
            clock_time_contract::pressure_hash(),
            pressure.len(),
            pressure
                .iter()
                .filter(|case| case.expected == clock_time_contract::ClockDecision::Supported)
                .count(),
            pressure
                .iter()
                .filter(|case| case.expected == clock_time_contract::ClockDecision::Ambiguous)
                .count(),
            pressure
                .iter()
                .filter(|case| case.expected == clock_time_contract::ClockDecision::Unsupported)
                .count(),
        );
        let defects = [
            ClockBehaviorDefect::ReversedSubtraction,
            ClockBehaviorDefect::BrokenMeridiemNormalization,
            ClockBehaviorDefect::MissingRolloverGuard,
            ClockBehaviorDefect::MissingReplayGate,
            ClockBehaviorDefect::AcceptMissingMeridiem,
            ClockBehaviorDefect::AllowMultipleDayRollover,
        ];
        let mut detected = 0;
        for defect in defects {
            let faulted =
                inject_clock_method_defect(&parent, defect).expect("clock defect injection");
            let report = evaluate_clock_defect(&faulted, &pressure);
            eprintln!(
                "phase4 clock defect: defect={defect:?} cases={} decision_mismatches={} duration_mismatches={} replay_failures={} false_auth={} false_denials={}",
                report.cases,
                report.decision_mismatches,
                report.duration_mismatches,
                report.replay_failures,
                report.false_authorizations,
                report.false_denials,
            );
            assert!(report.detected(), "defect {defect:?} was not observable");
            detected += 1;
            let revision = ClockMethodRevision {
                parent_spec_id: parent.spec_id.clone(),
                revision_id: format!("phase4-clock-repair-{defect:?}"),
                defect,
            };
            let repaired = apply_clock_revision_sandboxed(&faulted, &revision)
                .expect("sandboxed clock repair");
            let repaired_report = evaluate_method_spec(
                &repaired,
                &pressure
                    .iter()
                    .map(|case| HistoricalCase {
                        family: "ClockTimeDifferenceV1".into(),
                        prompt: case.prompt.clone(),
                        expected: match case.expected {
                            clock_time_contract::ClockDecision::Supported => {
                                ShadowDecision::Applicable
                            }
                            clock_time_contract::ClockDecision::Ambiguous => {
                                ShadowDecision::Ambiguous
                            }
                            clock_time_contract::ClockDecision::Unsupported => {
                                ShadowDecision::Unsupported
                            }
                        },
                    })
                    .collect::<Vec<_>>(),
            );
            assert_eq!(repaired_report.correct_decisions, pressure.len());
            assert_eq!(repaired_report.false_authorizations, 0);
            assert_eq!(repaired_report.false_denials, 0);
            assert_eq!(
                repaired_report.accepted_replay_verified,
                repaired_report.authorized
            );
            assert_eq!(parent.spec_id, faulted.parent.spec_id);
            assert!(parent.validate().is_ok());
        }
        assert_eq!(detected, defects.len());
    }

    #[test]
    fn unseen_state_contract_synthesizes_and_replays_dev_holdout() {
        let contract = finite_state_contract::contract();
        assert!(contract.validation_errors().is_empty());
        let method_contract = ValidatedMethodContract {
            contract_id: contract.contract_id.clone(),
            input_artifact: ArtifactType::RawPrompt,
            output_artifact: ArtifactType::StateTransitionTrace,
            required_bindings: contract.required_bindings.clone(),
            predicates: contract.predicates.clone(),
            trusted_capability: "finite_state_transition".into(),
            operation_budget: 16,
            depth_budget: 8,
        };
        let spec = synthesize_from_contract(&method_contract).expect("generic state synthesis");
        assert!(spec.validate().is_ok());
        let to_historical = |case: &finite_state_contract::StateCase| HistoricalCase {
            family: contract.contract_id.clone(),
            prompt: case.prompt.clone(),
            expected: match case.expected {
                StateDecision::Supported => ShadowDecision::Applicable,
                StateDecision::Ambiguous => ShadowDecision::Ambiguous,
                StateDecision::Unsupported => ShadowDecision::Unsupported,
            },
        };
        let development: Vec<HistoricalCase> = contract
            .cases
            .iter()
            .filter(|case| case.split == finite_state_contract::StateSplit::Development)
            .map(to_historical)
            .collect();
        let holdout: Vec<HistoricalCase> = contract
            .cases
            .iter()
            .filter(|case| case.split == finite_state_contract::StateSplit::Holdout)
            .map(to_historical)
            .collect();
        let development_report = evaluate_method_spec(&spec, &development);
        let holdout_report = evaluate_method_spec(&spec, &holdout);
        let mut state_matches = 0;
        for case in &contract.cases {
            let observed = shadow_execute(&spec, &case.prompt).expect("shadow state");
            let expected = match case.expected {
                StateDecision::Supported => ShadowDecision::Applicable,
                StateDecision::Ambiguous => ShadowDecision::Ambiguous,
                StateDecision::Unsupported => ShadowDecision::Unsupported,
            };
            if (observed.decision == expected && expected != ShadowDecision::Applicable)
                || (observed.decision == expected
                    && observed.observed_final_state == case.expected_state)
            {
                state_matches += 1;
            }
        }
        eprintln!(
            "phase4 unseen state: synthesis_version={} contract_hash={} dev_hash={} holdout_hash={} dev={}/{} holdout={}/{} holdout_authorized={} holdout_replay={} state_matches={} trace={:?}",
            SYNTHESIS_VERSION,
            contract.release_hash(),
            contract.split_hash(finite_state_contract::StateSplit::Development),
            contract.split_hash(finite_state_contract::StateSplit::Holdout),
            development_report.correct_decisions,
            development_report.cases,
            holdout_report.correct_decisions,
            holdout_report.cases,
            holdout_report.authorized,
            holdout_report.accepted_replay_verified,
            state_matches,
            operation_trace(&spec),
        );
        assert_eq!(
            development_report.correct_decisions,
            development_report.cases
        );
        assert_eq!(holdout_report.correct_decisions, holdout_report.cases);
        assert_eq!(state_matches, contract.cases.len());
        assert_eq!(holdout_report.false_authorizations, 0);
        assert_eq!(holdout_report.false_denials, 0);
        assert_eq!(
            holdout_report.accepted_replay_verified,
            holdout_report.authorized
        );
    }

    #[test]
    fn unseen_state_pressure_finds_and_repairs_all_behavioral_defects() {
        let contract = finite_state_contract::contract();
        let method_contract = ValidatedMethodContract {
            contract_id: contract.contract_id.clone(),
            input_artifact: ArtifactType::RawPrompt,
            output_artifact: ArtifactType::StateTransitionTrace,
            required_bindings: contract.required_bindings.clone(),
            predicates: contract.predicates.clone(),
            trusted_capability: "finite_state_transition".into(),
            operation_budget: 16,
            depth_budget: 8,
        };
        let parent = synthesize_from_contract(&method_contract).expect("generic state synthesis");
        let pressure = finite_state_contract::pressure_corpus();
        assert_eq!(pressure.len(), 240);
        let mut baseline_correct = 0;
        for case in &pressure {
            let observed = shadow_execute(&parent, &case.prompt).expect("baseline state shadow");
            let expected = match case.expected {
                StateDecision::Supported => ShadowDecision::Applicable,
                StateDecision::Ambiguous => ShadowDecision::Ambiguous,
                StateDecision::Unsupported => ShadowDecision::Unsupported,
            };
            if observed.decision == expected
                && (expected != ShadowDecision::Applicable
                    || observed.observed_final_state == case.expected_state)
                && observed.method_replay_verified
            {
                baseline_correct += 1;
            }
        }
        assert_eq!(baseline_correct, pressure.len());
        eprintln!(
            "phase4 state pressure: hash={} cases={} supported={} ambiguous={} unsupported={}",
            finite_state_contract::pressure_hash(),
            pressure.len(),
            pressure
                .iter()
                .filter(|case| case.expected == finite_state_contract::StateDecision::Supported)
                .count(),
            pressure
                .iter()
                .filter(|case| case.expected == finite_state_contract::StateDecision::Ambiguous)
                .count(),
            pressure
                .iter()
                .filter(|case| case.expected == finite_state_contract::StateDecision::Unsupported)
                .count(),
        );
        let defects = [
            finite_state_contract::StateBehaviorDefect::IgnoreGuards,
            finite_state_contract::StateBehaviorDefect::FirstMatchingTransition,
            finite_state_contract::StateBehaviorDefect::SkipInvalidIntermediate,
            finite_state_contract::StateBehaviorDefect::ReorderEvents,
            finite_state_contract::StateBehaviorDefect::ContinueAfterTerminal,
            finite_state_contract::StateBehaviorDefect::OmitTraceReplay,
            finite_state_contract::StateBehaviorDefect::AcceptUnknownStates,
            finite_state_contract::StateBehaviorDefect::BypassSequenceBudget,
        ];
        for defect in defects {
            let faulted =
                inject_state_method_defect(&parent, defect).expect("state defect injection");
            let report = evaluate_state_defect(&faulted, &pressure);
            eprintln!(
                "phase4 state defect: defect={defect:?} cases={} decision_mismatches={} final_state_mismatches={} replay_failures={} false_auth={} false_denials={}",
                report.cases,
                report.decision_mismatches,
                report.final_state_mismatches,
                report.replay_failures,
                report.false_authorizations,
                report.false_denials,
            );
            assert!(
                report.detected(),
                "state defect {defect:?} was not observable"
            );
            let revision = StateMethodRevision {
                parent_spec_id: parent.spec_id.clone(),
                revision_id: format!("phase4-state-repair-{defect:?}"),
                defect,
            };
            let repaired =
                apply_state_revision_sandboxed(&faulted, &revision).expect("state sandbox repair");
            let repaired_cases: Vec<HistoricalCase> = pressure
                .iter()
                .map(|case| HistoricalCase {
                    family: contract.contract_id.clone(),
                    prompt: case.prompt.clone(),
                    expected: match case.expected {
                        StateDecision::Supported => ShadowDecision::Applicable,
                        StateDecision::Ambiguous => ShadowDecision::Ambiguous,
                        StateDecision::Unsupported => ShadowDecision::Unsupported,
                    },
                })
                .collect();
            let repaired_report = evaluate_method_spec(&repaired, &repaired_cases);
            assert_eq!(repaired_report.correct_decisions, pressure.len());
            assert_eq!(repaired_report.false_authorizations, 0);
            assert_eq!(repaired_report.false_denials, 0);
            assert_eq!(
                repaired_report.accepted_replay_verified,
                repaired_report.authorized
            );
            assert!(parent.validate().is_ok());
            assert_eq!(parent.spec_id, faulted.parent.spec_id);
        }
    }
}
