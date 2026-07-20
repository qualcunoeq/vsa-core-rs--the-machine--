//! Narrow prompt-supplied linear-relationship model construction.
//!
//! Supported wording explicitly supplies a slope, baseline, input, and target:
//! `y increases by 3 for every unit increase in x, and y equals 2 when x is
//! 0. Find y when x is 4.`  No slope, intercept, or target is inferred from
//! weaker prose.

use crate::capabilities::CapabilityIoType;
use crate::constant_rate_model::{
    ModelArtifactType, ModelConstructionQualityGate, ModelConstructionSpec, ModelMatcherResult,
};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LinearRelationshipModel {
    pub slope: f64,
    pub intercept: f64,
    pub input: f64,
    pub relation: String,
    pub target: String,
    pub source_fragment: String,
    pub assumptions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum LinearRelationshipFailure {
    PatternNotMatched,
    InvalidSlope,
    InvalidIntercept,
    InvalidInput,
    MissingTarget,
    VerificationFailed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LinearRelationshipReceipt {
    pub model: LinearRelationshipModel,
    pub derived_value: f64,
    pub replay_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LinearRelationshipChainReceipt {
    pub model_receipt: LinearRelationshipReceipt,
    pub transformation: String,
    pub expression_source: String,
    pub numeric_result: f64,
    pub plan_steps: Vec<String>,
    pub replay_verified: bool,
}

pub fn linear_relationship_model_spec() -> ModelConstructionSpec {
    ModelConstructionSpec {
        id: "linear_relationship_model".into(),
        version: 1,
        supported_language_pattern:
            "y increases by M for every unit increase in x; y equals B when x is 0; find y when x is X".into(),
        required_evidence: vec![
            "explicit per-unit increase".into(),
            "explicit baseline value at x=0".into(),
            "explicit numeric input".into(),
            "explicit target y".into(),
        ],
        model_artifacts: vec![
            ModelArtifactType::Quantity,
            ModelArtifactType::Relation,
            ModelArtifactType::Expression,
            ModelArtifactType::Target,
        ],
        produced_artifacts: vec![CapabilityIoType::Expression, CapabilityIoType::BindingSet],
        introduced_assumptions: Vec::new(),
        validation_rules: vec![
            "slope, intercept, and input are finite numbers".into(),
            "baseline is explicitly anchored at x=0".into(),
            "target is explicitly y at the supplied input".into(),
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

pub fn linear_relationship_match(text: &str) -> ModelMatcherResult {
    let required = linear_relationship_model_spec().required_evidence;
    match construct_linear_relationship_model(text) {
        Ok(_) => ModelMatcherResult::eligible(required),
        Err(error) => ModelMatcherResult::rejected(format!("{error:?}"), required),
    }
}

pub fn construct_linear_relationship_model(
    text: &str,
) -> Result<LinearRelationshipModel, LinearRelationshipFailure> {
    let regex = regex::Regex::new(
        r"(?i)^\s*y increases by\s*([-+]?[0-9]+(?:\.[0-9]+)?)\s+for every unit increase in x,\s*and y equals\s*([-+]?[0-9]+(?:\.[0-9]+)?)\s+when x is 0\.\s*find y when x is\s*([-+]?[0-9]+(?:\.[0-9]+)?)\.?\s*$",
    )
    .expect("static linear-relationship model regex");
    let captures = regex
        .captures(text)
        .ok_or(LinearRelationshipFailure::PatternNotMatched)?;
    let slope = captures
        .get(1)
        .ok_or(LinearRelationshipFailure::InvalidSlope)?
        .as_str()
        .parse::<f64>()
        .map_err(|_| LinearRelationshipFailure::InvalidSlope)?;
    let intercept = captures
        .get(2)
        .ok_or(LinearRelationshipFailure::InvalidIntercept)?
        .as_str()
        .parse::<f64>()
        .map_err(|_| LinearRelationshipFailure::InvalidIntercept)?;
    let input = captures
        .get(3)
        .ok_or(LinearRelationshipFailure::InvalidInput)?
        .as_str()
        .parse::<f64>()
        .map_err(|_| LinearRelationshipFailure::InvalidInput)?;
    if !slope.is_finite() {
        return Err(LinearRelationshipFailure::InvalidSlope);
    }
    if !intercept.is_finite() {
        return Err(LinearRelationshipFailure::InvalidIntercept);
    }
    if !input.is_finite() {
        return Err(LinearRelationshipFailure::InvalidInput);
    }
    Ok(LinearRelationshipModel {
        slope,
        intercept,
        input,
        relation: "y = slope × x + intercept".into(),
        target: "y".into(),
        source_fragment: text.trim().into(),
        assumptions: Vec::new(),
    })
}

pub fn execute_linear_relationship_model(
    text: &str,
) -> Result<LinearRelationshipReceipt, LinearRelationshipFailure> {
    let model = construct_linear_relationship_model(text)?;
    let derived_value = model.slope * model.input + model.intercept;
    if !derived_value.is_finite() {
        return Err(LinearRelationshipFailure::VerificationFailed);
    }
    let replay = model.slope * model.input + model.intercept;
    if (derived_value - replay).abs() > 1e-12 {
        return Err(LinearRelationshipFailure::VerificationFailed);
    }
    Ok(LinearRelationshipReceipt {
        model,
        derived_value,
        replay_verified: true,
    })
}

pub fn execute_linear_relationship_chain(
    text: &str,
) -> Result<LinearRelationshipChainReceipt, LinearRelationshipFailure> {
    let model_receipt = execute_linear_relationship_model(text)?;
    let expression_source = format!(
        "{}*{}+{}",
        model_receipt.model.slope,
        model_receipt.model.input,
        model_receipt.model.intercept
    );
    let expression = crate::algebra::parse(&expression_source)
        .map_err(|_| LinearRelationshipFailure::VerificationFailed)?;
    let numeric_result = expression
        .evaluate(&[])
        .ok_or(LinearRelationshipFailure::VerificationFailed)?;
    let replay_expression = crate::algebra::parse(&expression_source)
        .map_err(|_| LinearRelationshipFailure::VerificationFailed)?;
    let replay_result = replay_expression
        .evaluate(&[])
        .ok_or(LinearRelationshipFailure::VerificationFailed)?;
    if (numeric_result - replay_result).abs() > 1e-12
        || (numeric_result - model_receipt.derived_value).abs() > 1e-12
    {
        return Err(LinearRelationshipFailure::VerificationFailed);
    }
    Ok(LinearRelationshipChainReceipt {
        model_receipt,
        transformation: "y = slope × x + intercept".into(),
        expression_source,
        numeric_result,
        plan_steps: vec![
            "linear_relationship_model_v1".into(),
            "expression_evaluation_v1".into(),
        ],
        replay_verified: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const POSITIVE: &str =
        "y increases by 3 for every unit increase in x, and y equals 2 when x is 0. Find y when x is 4.";

    #[test]
    fn constructs_and_replays_explicit_linear_relationship() {
        let receipt = execute_linear_relationship_chain(POSITIVE).unwrap();
        assert_eq!(receipt.numeric_result, 14.0);
        assert!(receipt.replay_verified);
        assert!(receipt.model_receipt.model.assumptions.is_empty());
    }

    #[test]
    fn missing_baseline_is_rejected() {
        let text = "y increases by 3 for every unit increase in x. Find y when x is 4.";
        assert_eq!(
            construct_linear_relationship_model(text),
            Err(LinearRelationshipFailure::PatternNotMatched)
        );
    }

    #[test]
    fn missing_target_is_rejected() {
        let text =
            "y increases by 3 for every unit increase in x, and y equals 2 when x is 0.";
        assert_eq!(
            construct_linear_relationship_model(text),
            Err(LinearRelationshipFailure::PatternNotMatched)
        );
    }

    #[test]
    fn registry_spec_has_no_unauthorized_assumptions() {
        let spec = linear_relationship_model_spec();
        assert!(spec.quality_gate.enabled());
        assert!(spec.introduced_assumptions.is_empty());
        assert!(!spec.model_artifacts.is_empty());
    }
}
