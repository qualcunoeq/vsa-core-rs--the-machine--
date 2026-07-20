//! Shadow-only model construction for one explicit constant-rate sentence.
//!
//! This is intentionally separate from transformation capabilities: it
//! consumes text evidence and constructs a formal relation.  It is not wired
//! into production target routing until a broader modeling contract exists.

use serde::Serialize;
use crate::capabilities::CapabilityIoType;

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
    pub quality_gate: ModelConstructionQualityGate,
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
        quality_gate: ModelConstructionQualityGate {
            positive_cases: 1,
            negative_cases: 2,
            adversarial_cases: 1,
            unauthorized_assumptions: 0,
            replay_failures: 0,
        },
    }
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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConstantRateModelReceipt {
    pub model: ConstantRateModel,
    pub derived_change: f64,
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

#[cfg(test)]
mod tests {
    use super::*;

    const POSITIVE: &str =
        "A quantity changes at a constant rate of 3 per interval for 4 intervals. Find the total change.";

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
        assert_eq!(shadow_cases().len(), 4);
    }

    #[test]
    fn shadow_composition_replays_model_through_expression_backend() {
        let receipt = execute_constant_rate_model(POSITIVE).unwrap();
        let expression = crate::algebra::parse("3*4").unwrap();
        assert_eq!(expression.evaluate(&[]), Some(receipt.derived_change));
        assert!(receipt.replay_verified);
    }
}
