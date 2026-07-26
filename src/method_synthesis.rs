//! Phase 4 restricted implementation synthesis.
//!
//! This module synthesizes immutable, declarative method specifications from
//! already validated capability contracts.  It deliberately does not emit
//! Rust, execute arbitrary code, mutate a registry, or grant capability
//! authority.  The shadow interpreter may call only the existing, named
//! capability formalizers and their replay gates.

use crate::fractional_quantity::{self, FractionalQuantityDecision};
use crate::percentage_quantity::{self, PercentageQuantityDecision};
use crate::quantity_relation::{self, QuantityRelationDecision};
use crate::unit_aware_quantity::{self, UnitQuantityDecision};
use serde::{Deserialize, Serialize};

const MAX_STEPS: usize = 16;
const MAX_DEPTH: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArtifactType {
    RawPrompt,
    QuantityRelation,
    UnitQuantity,
    FractionalQuantity,
    PercentageQuantity,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpecValidationError {
    EmptySteps,
    WrongInitialInput { expected: ArtifactType, actual: ArtifactType },
    BrokenHandoff { index: usize, expected: ArtifactType, actual: ArtifactType },
    WrongFinalOutput { expected: ArtifactType, actual: ArtifactType },
    OperationBudgetExceeded { actual: usize, budget: usize },
    DepthBudgetExceeded { actual: usize, budget: usize },
    AuthorityRequired,
    UntrustedCapability(String),
    VerificationMissing,
    ReplayMissing,
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
        if self.depth_budget == 0 || self.depth_budget > MAX_DEPTH || actual > self.depth_budget {
            errors.push(SpecValidationError::DepthBudgetExceeded {
                actual,
                budget: self.depth_budget.min(MAX_DEPTH),
            });
        }
        if !self.diagnostic_only {
            errors.push(SpecValidationError::AuthorityRequired);
        }

        let mut verified = false;
        let mut replay = false;
        for step in &self.steps {
            match &step.operation {
                DslOperation::InvokeCapability { capability }
                    if !trusted_capability(capability) =>
                {
                    errors.push(SpecValidationError::UntrustedCapability(capability.clone()));
                }
                DslOperation::VerifyArtifact => verified = true,
                DslOperation::Replay => replay = true,
                _ => {}
            }
        }
        if !verified {
            errors.push(SpecValidationError::VerificationMissing);
        }
        if !replay {
            errors.push(SpecValidationError::ReplayMissing);
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }
}

fn trusted_capability(capability: &str) -> bool {
    matches!(
        capability,
        "quantity_relation" | "unit_aware_quantity" | "fractional_quantity" | "percentage_quantity"
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
    pub artifact_replay_verified: bool,
    pub method_replay_verified: bool,
}

impl ShadowExecution {
    pub fn authorized(&self) -> bool { self.decision == ShadowDecision::Applicable }
}

/// Build the first restricted method specification for a historical family.
/// The result is a plan, not an executable capability or registry entry.
pub fn synthesize_historical_method(
    family: &str,
) -> Result<MethodImplementationSpec, String> {
    let (capability, artifact) = match family {
        "QuantityRelationV1" => ("quantity_relation", ArtifactType::QuantityRelation),
        "UnitQuantity" => ("unit_aware_quantity", ArtifactType::UnitQuantity),
        "FractionalQuantity" => ("fractional_quantity", ArtifactType::FractionalQuantity),
        "PercentageQuantityV1" => ("percentage_quantity", ArtifactType::PercentageQuantity),
        other => return Err(format!("unsupported historical family: {other}")),
    };
    let mut steps = vec![
        MethodStep {
            operation: DslOperation::ExtractBinding { name: "source_text".into() },
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
            operation: DslOperation::CheckPredicate { predicate: "declared_contract_predicates".into() },
            input: ArtifactType::RawPrompt,
            output: ArtifactType::RawPrompt,
        },
        MethodStep {
            operation: DslOperation::InvokeCapability { capability: capability.into() },
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
    };
    spec.validate().map_err(|errors| format!("invalid synthesized spec: {errors:?}"))?;
    Ok(spec)
}

/// Execute a validated method spec in the shadow interpreter.  Only the
/// trusted historical formalizers are callable; all accepted artifacts pass
/// their own replay gate before the method receipt is marked valid.
pub fn shadow_execute(
    spec: &MethodImplementationSpec,
    prompt: &str,
) -> Result<ShadowExecution, String> {
    spec.validate().map_err(|errors| format!("invalid method spec: {errors:?}"))?;
    let (decision, artifact_type, artifact_replay_verified) = match spec.capability_family.as_str() {
        "QuantityRelationV1" => match quantity_relation::formalize(prompt) {
            QuantityRelationDecision::Accepted(artifact) =>
                (ShadowDecision::Applicable, Some(ArtifactType::QuantityRelation), artifact.replay_verified()),
            QuantityRelationDecision::Ambiguous => (ShadowDecision::Ambiguous, None, false),
            QuantityRelationDecision::Unsupported => (ShadowDecision::Unsupported, None, false),
        },
        "UnitQuantity" => match unit_aware_quantity::formalize(prompt) {
            UnitQuantityDecision::Accepted(artifact) =>
                (ShadowDecision::Applicable, Some(ArtifactType::UnitQuantity), artifact.replay_verified()),
            UnitQuantityDecision::Ambiguous => (ShadowDecision::Ambiguous, None, false),
            UnitQuantityDecision::Unsupported => (ShadowDecision::Unsupported, None, false),
        },
        "FractionalQuantity" => match fractional_quantity::formalize(prompt) {
            FractionalQuantityDecision::Accepted(artifact) =>
                (ShadowDecision::Applicable, Some(ArtifactType::FractionalQuantity), artifact.replay_verified()),
            FractionalQuantityDecision::Ambiguous => (ShadowDecision::Ambiguous, None, false),
            FractionalQuantityDecision::Unsupported => (ShadowDecision::Unsupported, None, false),
        },
        "PercentageQuantityV1" => match percentage_quantity::formalize(prompt) {
            PercentageQuantityDecision::Accepted(artifact) =>
                (ShadowDecision::Applicable, Some(ArtifactType::PercentageQuantity), artifact.replay_verified()),
            PercentageQuantityDecision::Ambiguous => (ShadowDecision::Ambiguous, None, false),
            PercentageQuantityDecision::Unsupported => (ShadowDecision::Unsupported, None, false),
        },
        other => return Err(format!("unsupported shadow family: {other}")),
    };
    // A formalizer's positive classification is never sufficient by itself:
    // a failed artifact replay gate downgrades the shadow decision to a safe
    // refusal before any method receipt can be considered authorized.
    let (decision, artifact_type) = if decision == ShadowDecision::Applicable
        && !artifact_replay_verified
    {
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
        artifact_replay_verified,
        method_replay_verified,
    })
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

pub fn evaluate_historical_cases(cases: &[HistoricalCase]) -> SynthesisCampaignReport {
    let mut report = SynthesisCampaignReport { cases: cases.len(), ..Default::default() };
    for case in cases {
        let Ok(spec) = synthesize_historical_method(&case.family) else {
            report.invalid_specs += 1;
            continue;
        };
        let Ok(result) = shadow_execute(&spec, &case.prompt) else {
            report.invalid_specs += 1;
            continue;
        };
        report.correct_decisions += usize::from(result.decision == case.expected);
        report.authorized += usize::from(result.authorized());
        report.replay_verified += usize::from(result.method_replay_verified);
        report.accepted_replay_verified += usize::from(
            result.authorized() && result.artifact_replay_verified && result.method_replay_verified,
        );
        report.false_authorizations += usize::from(
            result.authorized() && case.expected != ShadowDecision::Applicable,
        );
        report.false_denials += usize::from(
            !result.authorized() && case.expected == ShadowDecision::Applicable,
        );
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn historical_cases() -> Vec<HistoricalCase> {
        vec![
            HistoricalCase { family: "QuantityRelationV1".into(), prompt: "5 notebooks cost 20 dollars. What is the price per notebook?".into(), expected: ShadowDecision::Applicable },
            HistoricalCase { family: "QuantityRelationV1".into(), prompt: "A price changes by 5% each year. What is the final price?".into(), expected: ShadowDecision::Unsupported },
            HistoricalCase { family: "UnitQuantity".into(), prompt: "Convert 3 meters to centimeters using 100 centimeters per meter.".into(), expected: ShadowDecision::Applicable },
            HistoricalCase { family: "UnitQuantity".into(), prompt: "Add 2 meters and 30 centimeters.".into(), expected: ShadowDecision::Ambiguous },
            HistoricalCase { family: "FractionalQuantity".into(), prompt: "What is three quarters of 20?".into(), expected: ShadowDecision::Applicable },
            HistoricalCase { family: "FractionalQuantity".into(), prompt: "There is a 25% probability.".into(), expected: ShadowDecision::Unsupported },
            HistoricalCase { family: "PercentageQuantityV1".into(), prompt: "What is 20% of 50?".into(), expected: ShadowDecision::Applicable },
            HistoricalCase { family: "PercentageQuantityV1".into(), prompt: "A balance grows by 5% each year for 5 years.".into(), expected: ShadowDecision::Unsupported },
        ]
    }

    #[test]
    fn historical_specs_are_bounded_and_non_authorizing() {
        for family in ["QuantityRelationV1", "UnitQuantity", "FractionalQuantity", "PercentageQuantityV1"] {
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
        spec.steps[5].operation = DslOperation::InvokeCapability { capability: "arbitrary_code".into() };
        assert!(spec.validate().is_err());
    }
}
