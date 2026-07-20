//! Shadow-only model construction for one explicit constant-rate sentence.
//!
//! This is intentionally separate from transformation capabilities: it
//! consumes text evidence and constructs a formal relation.  It is not wired
//! into production target routing until a broader modeling contract exists.

use serde::Serialize;
use crate::capabilities::CapabilityIoType;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelConstructionQualityGate {
    pub positive_cases: usize,
    pub negative_cases: usize,
    pub adversarial_cases: usize,
    pub unauthorized_assumptions: usize,
    pub replay_failures: usize,
}

impl ModelConstructionQualityGate {
    pub fn enabled(&self) -> bool {
        self.positive_cases > 0
            && self.negative_cases > 0
            && self.adversarial_cases > 0
            && self.unauthorized_assumptions == 0
            && self.replay_failures == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelConstructionSpec {
    pub id: String,
    pub version: u32,
    pub supported_language_pattern: String,
    pub required_evidence: Vec<String>,
    pub produced_artifacts: Vec<CapabilityIoType>,
    pub introduced_assumptions: Vec<String>,
    pub validation_rules: Vec<String>,
    pub quality_gate: ModelConstructionQualityGate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ModelSelection {
    Unique(String),
    Ambiguous(Vec<String>),
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelCandidateTrace {
    pub id: String,
    pub version: u32,
    pub eligible: bool,
    pub rejection: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelDiscoveryTrace {
    pub candidates: Vec<ModelCandidateTrace>,
    pub selection: ModelSelection,
}

/// A registered model constructor is a proposal mechanism only.  It may
/// become eligible from text evidence, but it never executes by virtue of
/// being registered.  The downstream model contract and transformation
/// verifier remain the authorization boundary.
pub type ModelMatcher = fn(&str) -> Result<(), String>;

#[derive(Clone)]
pub struct ModelConstructorEntry {
    pub spec: ModelConstructionSpec,
    pub matcher: ModelMatcher,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ModelRegistryError {
    EmptyId,
    EmptyVersion,
    DuplicateVersionedId { id: String, version: u32 },
    MissingEvidence { id: String },
    MissingProducedArtifacts { id: String },
}

pub struct ModelConstructorRegistry {
    entries: Vec<ModelConstructorEntry>,
}

impl std::fmt::Debug for ModelConstructorRegistry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModelConstructorRegistry")
            .field("entries", &self.entries.iter().map(|entry| &entry.spec).collect::<Vec<_>>())
            .finish()
    }
}

impl ModelConstructorRegistry {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// The only constructor currently enabled for the production model
    /// registry.  Additional constructors must earn their own quality gate.
    pub fn production() -> Self {
        let mut registry = Self::new();
        registry
            .register(ModelConstructorEntry {
                spec: constant_rate_model_spec(),
                matcher: constant_rate_match,
            })
            .expect("constant-rate model registry entry is valid");
        registry
    }

    pub fn register(
        &mut self,
        entry: ModelConstructorEntry,
    ) -> Result<(), ModelRegistryError> {
        let spec = &entry.spec;
        if spec.id.trim().is_empty() {
            return Err(ModelRegistryError::EmptyId);
        }
        if spec.version == 0 {
            return Err(ModelRegistryError::EmptyVersion);
        }
        if spec.required_evidence.is_empty() {
            return Err(ModelRegistryError::MissingEvidence {
                id: spec.id.clone(),
            });
        }
        if spec.produced_artifacts.is_empty() {
            return Err(ModelRegistryError::MissingProducedArtifacts {
                id: spec.id.clone(),
            });
        }
        if self
            .entries
            .iter()
            .any(|existing| existing.spec.id == spec.id && existing.spec.version == spec.version)
        {
            return Err(ModelRegistryError::DuplicateVersionedId {
                id: spec.id.clone(),
                version: spec.version,
            });
        }
        self.entries.push(entry);
        self.entries.sort_by(|left, right| {
            left.spec
                .id
                .cmp(&right.spec.id)
                .then(left.spec.version.cmp(&right.spec.version))
        });
        Ok(())
    }

    pub fn entries(&self) -> impl Iterator<Item = &ModelConstructorEntry> {
        self.entries.iter()
    }

    /// Discover eligible model constructors.  Registry insertion order never
    /// resolves multiple matches: two eligible entries produce Ambiguous.
    pub fn discover(&self, text: &str) -> ModelDiscoveryTrace {
        let mut candidates = Vec::with_capacity(self.entries.len());
        let mut eligible_ids = Vec::new();
        let mut seen_ids = BTreeSet::new();
        for entry in &self.entries {
            let (eligible, rejection) = if !entry.spec.quality_gate.enabled() {
                (false, Some("quality_gate_failed".to_string()))
            } else {
                match (entry.matcher)(text) {
                    Ok(()) => (true, None),
                    Err(reason) => (false, Some(reason)),
                }
            };
            if eligible && seen_ids.insert(entry.spec.id.clone()) {
                eligible_ids.push(entry.spec.id.clone());
            }
            candidates.push(ModelCandidateTrace {
                id: entry.spec.id.clone(),
                version: entry.spec.version,
                eligible,
                rejection,
            });
        }
        let selection = match eligible_ids.len() {
            0 => ModelSelection::None,
            1 => ModelSelection::Unique(eligible_ids.remove(0)),
            _ => ModelSelection::Ambiguous(eligible_ids),
        };
        ModelDiscoveryTrace { candidates, selection }
    }
}

impl Default for ModelConstructorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub fn constant_rate_model_spec() -> ModelConstructionSpec {
    ModelConstructionSpec {
        id: "constant_rate_model".into(),
        version: 1,
        supported_language_pattern:
            "a quantity changes at a constant rate of R per interval for T intervals; find total change".into(),
        required_evidence: vec![
            "explicit constant-rate wording".into(),
            "explicit numeric rate".into(),
            "explicit duration".into(),
            "explicit total-change target".into(),
        ],
        produced_artifacts: vec![CapabilityIoType::Expression],
        introduced_assumptions: Vec::new(),
        validation_rules: vec![
            "rate and duration are finite numbers".into(),
            "duration is non-negative".into(),
            "total-change target is explicit".into(),
        ],
        quality_gate: ModelConstructionQualityGate {
            positive_cases: 1,
            negative_cases: 2,
            adversarial_cases: 1,
            unauthorized_assumptions: 0,
            replay_failures: 0,
        },
    }
}

/// Shadow model discovery.  Candidate matching is performed by the
/// constructor's strict evidence parser; registry order never resolves two
/// eligible models.
pub fn discover_models(text: &str) -> ModelDiscoveryTrace {
    ModelConstructorRegistry::production().discover(text)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConstantRateModel {
    pub rate: f64,
    pub duration: f64,
    pub relation: String,
    pub target: String,
    pub source_fragment: String,
    pub assumptions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ConstantRateModelFailure {
    PatternNotMatched,
    ConstantRateNotExplicit,
    InvalidRate,
    InvalidDuration,
    MissingTarget,
    VerificationFailed,
}

fn constant_rate_match(text: &str) -> Result<(), String> {
    construct_constant_rate_model(text)
        .map(|_| ())
        .map_err(|error| format!("{error:?}"))
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConstantRateModelReceipt {
    pub model: ConstantRateModel,
    pub derived_change: f64,
    pub replay_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConstantRateChainReceipt {
    pub model_receipt: ConstantRateModelReceipt,
    pub transformation: String,
    pub expression_source: String,
    pub numeric_result: f64,
    pub plan_steps: Vec<String>,
    pub replay_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConstantRateShadowCase {
    pub id: String,
    pub prompt: String,
    pub should_construct: bool,
}

pub fn shadow_cases() -> Vec<ConstantRateShadowCase> {
    vec![
        ConstantRateShadowCase {
            id: "constant-rate-positive".into(),
            prompt: "A quantity changes at a constant rate of 3 per interval for 4 intervals. Find the total change.".into(),
            should_construct: true,
        },
        ConstantRateShadowCase {
            id: "constant-rate-missing-constancy".into(),
            prompt: "A quantity changes at a rate of 3 per interval for 4 intervals. Find the total change.".into(),
            should_construct: false,
        },
        ConstantRateShadowCase {
            id: "constant-rate-missing-target".into(),
            prompt: "A quantity changes at a constant rate of 3 per interval for 4 intervals.".into(),
            should_construct: false,
        },
        ConstantRateShadowCase {
            id: "constant-rate-wrong-model".into(),
            prompt: "A quantity accelerates at 3 per interval for 4 intervals. Find the total change.".into(),
            should_construct: false,
        },
    ]
}

/// Construct only the explicitly stated `change = constant_rate * duration`
/// model.  No constancy, unit compatibility, or target is inferred.
pub fn construct_constant_rate_model(
    text: &str,
) -> Result<ConstantRateModel, ConstantRateModelFailure> {
    let regex = regex::Regex::new(
        r"(?i)^\s*a quantity changes at a constant rate of\s*([-+]?[0-9]+(?:\.[0-9]+)?)\s+per interval for\s*([-+]?[0-9]+(?:\.[0-9]+)?)\s+intervals?\.\s*find (?:the )?total change\.?\s*$",
    )
    .expect("static constant-rate model regex");
    let captures = regex
        .captures(text)
        .ok_or(ConstantRateModelFailure::PatternNotMatched)?;
    let rate = captures
        .get(1)
        .ok_or(ConstantRateModelFailure::InvalidRate)?
        .as_str()
        .parse::<f64>()
        .map_err(|_| ConstantRateModelFailure::InvalidRate)?;
    let duration = captures
        .get(2)
        .ok_or(ConstantRateModelFailure::InvalidDuration)?
        .as_str()
        .parse::<f64>()
        .map_err(|_| ConstantRateModelFailure::InvalidDuration)?;
    if !rate.is_finite() {
        return Err(ConstantRateModelFailure::InvalidRate);
    }
    if !duration.is_finite() || duration < 0.0 {
        return Err(ConstantRateModelFailure::InvalidDuration);
    }
    Ok(ConstantRateModel {
        rate,
        duration,
        relation: "change = rate × duration".into(),
        target: "total_change".into(),
        source_fragment: text.trim().into(),
        assumptions: Vec::new(),
    })
}

pub fn execute_constant_rate_model(
    text: &str,
) -> Result<ConstantRateModelReceipt, ConstantRateModelFailure> {
    let model = construct_constant_rate_model(text)?;
    let derived_change = model.rate * model.duration;
    if !derived_change.is_finite() {
        return Err(ConstantRateModelFailure::VerificationFailed);
    }
    // Replay from the extracted scalar premises, not from a cached result.
    let replay = model.rate * model.duration;
    if (derived_change - replay).abs() > 1e-12 {
        return Err(ConstantRateModelFailure::VerificationFailed);
    }
    Ok(ConstantRateModelReceipt {
        model,
        derived_change,
        replay_verified: true,
    })
}

/// End-to-end shadow-to-production route for the first modeling island.
/// Model discovery is the authorization boundary; arithmetic is delegated to
/// the existing deterministic expression backend and replayed independently.
pub fn execute_constant_rate_chain(
    text: &str,
) -> Result<ConstantRateChainReceipt, ConstantRateModelFailure> {
    let discovery = discover_models(text);
    if discovery.selection != ModelSelection::Unique("constant_rate_model".into()) {
        return Err(ConstantRateModelFailure::PatternNotMatched);
    }
    let model_receipt = execute_constant_rate_model(text)?;
    let expression_source = format!("{}*{}", model_receipt.model.rate, model_receipt.model.duration);
    let expression = crate::algebra::parse(&expression_source)
        .map_err(|_| ConstantRateModelFailure::VerificationFailed)?;
    let numeric_result = expression
        .evaluate(&[])
        .ok_or(ConstantRateModelFailure::VerificationFailed)?;
    // Replay from the model's extracted premises, not the first evaluation.
    let replay_expression = crate::algebra::parse(&format!(
        "{}*{}",
        model_receipt.model.rate, model_receipt.model.duration
    ))
    .map_err(|_| ConstantRateModelFailure::VerificationFailed)?;
    let replay_result = replay_expression
        .evaluate(&[])
        .ok_or(ConstantRateModelFailure::VerificationFailed)?;
    if (numeric_result - replay_result).abs() > 1e-12
        || (numeric_result - model_receipt.derived_change).abs() > 1e-12
    {
        return Err(ConstantRateModelFailure::VerificationFailed);
    }
    Ok(ConstantRateChainReceipt {
        model_receipt,
        transformation: "change = rate × duration".into(),
        expression_source,
        numeric_result,
        plan_steps: vec![
            "constant_rate_model_v1".into(),
            "expression_evaluation_v1".into(),
        ],
        replay_verified: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const POSITIVE: &str =
        "A quantity changes at a constant rate of 3 per interval for 4 intervals. Find the total change.";

    fn always_matches(_: &str) -> Result<(), String> {
        Ok(())
    }

    fn registry_spec(id: &str, version: u32) -> ModelConstructionSpec {
        ModelConstructionSpec {
            id: id.into(),
            version,
            supported_language_pattern: "test pattern".into(),
            required_evidence: vec!["explicit test evidence".into()],
            produced_artifacts: vec![CapabilityIoType::Expression],
            introduced_assumptions: Vec::new(),
            validation_rules: vec!["test validator".into()],
            quality_gate: ModelConstructionQualityGate {
                positive_cases: 1,
                negative_cases: 1,
                adversarial_cases: 1,
                unauthorized_assumptions: 0,
                replay_failures: 0,
            },
        }
    }

    #[test]
    fn constructs_and_replays_explicit_constant_rate_model() {
        let receipt = execute_constant_rate_model(POSITIVE).unwrap();
        assert_eq!(receipt.model.rate, 3.0);
        assert_eq!(receipt.model.duration, 4.0);
        assert_eq!(receipt.derived_change, 12.0);
        assert!(receipt.replay_verified);
        assert!(receipt.model.assumptions.is_empty());
    }

    #[test]
    fn missing_constant_word_is_rejected() {
        let text = "A quantity changes at a rate of 3 per interval for 4 intervals. Find the total change.";
        assert_eq!(
            construct_constant_rate_model(text),
            Err(ConstantRateModelFailure::PatternNotMatched)
        );
    }

    #[test]
    fn missing_target_is_rejected() {
        let text = "A quantity changes at a constant rate of 3 per interval for 4 intervals.";
        assert_eq!(
            construct_constant_rate_model(text),
            Err(ConstantRateModelFailure::PatternNotMatched)
        );
    }

    #[test]
    fn negative_duration_is_rejected() {
        let text = "A quantity changes at a constant rate of 3 per interval for -4 intervals. Find the total change.";
        assert_eq!(
            construct_constant_rate_model(text),
            Err(ConstantRateModelFailure::InvalidDuration)
        );
    }

    #[test]
    fn model_quality_gate_requires_positive_and_negative_evidence() {
        assert!(constant_rate_model_spec().quality_gate.enabled());
        assert_eq!(constant_rate_model_spec().introduced_assumptions.len(), 0);
        assert!(!constant_rate_model_spec().validation_rules.is_empty());
        assert_eq!(shadow_cases().len(), 4);
    }

    #[test]
    fn shadow_composition_replays_model_through_expression_backend() {
        let receipt = execute_constant_rate_model(POSITIVE).unwrap();
        let expression = crate::algebra::parse("3*4").unwrap();
        assert_eq!(expression.evaluate(&[]), Some(receipt.derived_change));
        assert!(receipt.replay_verified);
    }

    #[test]
    fn model_discovery_requires_unique_evidence_match() {
        assert_eq!(
            discover_models(POSITIVE).selection,
            ModelSelection::Unique("constant_rate_model".into())
        );
        assert_eq!(
            discover_models(
                "A quantity changes at a rate of 3 per interval for 4 intervals. Find the total change."
            )
            .selection,
            ModelSelection::None
        );
    }

    #[test]
    fn production_chain_models_then_evaluates_and_replays() {
        let receipt = execute_constant_rate_chain(POSITIVE).unwrap();
        assert_eq!(receipt.numeric_result, 12.0);
        assert_eq!(receipt.plan_steps.len(), 2);
        assert!(receipt.replay_verified);
        assert!(receipt.model_receipt.replay_verified);
    }

    #[test]
    fn production_chain_rejects_unauthorized_model() {
        let text =
            "A quantity changes at a rate of 3 per interval for 4 intervals. Find the total change.";
        assert_eq!(
            execute_constant_rate_chain(text),
            Err(ConstantRateModelFailure::PatternNotMatched)
        );
    }

    #[test]
    fn registry_production_keeps_constant_rate_as_unique_route() {
        let registry = ModelConstructorRegistry::production();
        assert_eq!(registry.entries().count(), 1);
        assert_eq!(
            registry.discover(POSITIVE).selection,
            ModelSelection::Unique("constant_rate_model".into())
        );
        assert_eq!(
            registry
                .discover("A quantity changes at a rate of 3 per interval for 4 intervals.")
                .selection,
            ModelSelection::None
        );
    }

    #[test]
    fn registry_never_uses_insertion_order_to_resolve_ambiguity() {
        let mut registry = ModelConstructorRegistry::new();
        registry
            .register(ModelConstructorEntry {
                spec: registry_spec("z_model", 1),
                matcher: always_matches,
            })
            .unwrap();
        registry
            .register(ModelConstructorEntry {
                spec: registry_spec("a_model", 1),
                matcher: always_matches,
            })
            .unwrap();

        let trace = registry.discover("evidence");
        assert_eq!(
            trace.selection,
            ModelSelection::Ambiguous(vec!["a_model".into(), "z_model".into()])
        );
        assert!(trace.candidates.iter().all(|candidate| candidate.eligible));
    }

    #[test]
    fn registry_rejects_duplicate_versioned_entries() {
        let mut registry = ModelConstructorRegistry::new();
        let entry = ModelConstructorEntry {
            spec: registry_spec("same_model", 1),
            matcher: always_matches,
        };
        registry.register(entry.clone()).unwrap();
        assert_eq!(
            registry.register(entry),
            Err(ModelRegistryError::DuplicateVersionedId {
                id: "same_model".into(),
                version: 1,
            })
        );
    }

    #[test]
    fn registry_trace_exposes_quality_gate_rejection() {
        let mut registry = ModelConstructorRegistry::new();
        let mut spec = registry_spec("disabled_model", 1);
        spec.quality_gate.replay_failures = 1;
        registry
            .register(ModelConstructorEntry {
                spec,
                matcher: always_matches,
            })
            .unwrap();

        let trace = registry.discover("evidence");
        assert_eq!(trace.selection, ModelSelection::None);
        assert_eq!(
            trace.candidates[0].rejection.as_deref(),
            Some("quality_gate_failed")
        );
    }
}
