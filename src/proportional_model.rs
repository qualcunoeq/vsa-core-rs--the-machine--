//! Narrow prompt-supplied proportional-relationship model construction.
//!
//! The constructor requires the proportionality constant, input, and target
//! explicitly.  It does not infer proportionality from words such as
//! "scales" or "depends on".

use crate::capabilities::CapabilityIoType;
use crate::constant_rate_model::{
    ModelArtifactType, ModelConstructionQualityGate, ModelConstructionSpec, ModelEvidenceContext,
    ModelMatcherResult,
};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProportionalModel {
    pub constant: f64,
    pub input: f64,
    pub relation: String,
    pub target: String,
    pub source_fragment: String,
    pub assumptions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ProportionalFailure {
    PatternNotMatched,
    InvalidConstant,
    InvalidInput,
    VerificationFailed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProportionalReceipt {
    pub model: ProportionalModel,
    pub derived_value: f64,
    pub replay_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProportionalChainReceipt {
    pub model_receipt: ProportionalReceipt,
    pub transformation: String,
    pub expression_source: String,
    pub numeric_result: f64,
    pub plan_steps: Vec<String>,
    pub replay_verified: bool,
}

pub fn proportional_model_spec() -> ModelConstructionSpec {
    ModelConstructionSpec {
        id: "proportional_model".into(),
        version: 1,
        supported_language_pattern:
            "y is proportional to x with proportionality constant K; find y when x is X".into(),
        required_evidence: vec![
            "explicit proportionality wording".into(),
            "explicit proportionality constant".into(),
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
            "constant and input are finite numbers".into(),
            "proportionality is explicitly stated".into(),
            "target is explicit".into(),
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

pub fn proportional_model_match(context: &ModelEvidenceContext) -> ModelMatcherResult {
    let required = proportional_model_spec().required_evidence;
    match construct_proportional_model(&context.original_text) {
        Ok(_) => ModelMatcherResult::eligible(required),
        Err(error) => ModelMatcherResult::rejected(format!("{error:?}"), required),
    }
}

pub fn construct_proportional_model(
    text: &str,
) -> Result<ProportionalModel, ProportionalFailure> {
    let regex = regex::Regex::new(
        r"(?i)^\s*y is proportional to x with proportionality constant\s*([-+]?[0-9]+(?:\.[0-9]+)?)\.\s*find y when x is\s*([-+]?[0-9]+(?:\.[0-9]+)?)\.?\s*$",
    )
    .expect("static proportional model regex");
    let captures = regex
        .captures(text)
        .ok_or(ProportionalFailure::PatternNotMatched)?;
    let constant = captures
        .get(1)
        .ok_or(ProportionalFailure::InvalidConstant)?
        .as_str()
        .parse::<f64>()
        .map_err(|_| ProportionalFailure::InvalidConstant)?;
    let input = captures
        .get(2)
        .ok_or(ProportionalFailure::InvalidInput)?
        .as_str()
        .parse::<f64>()
        .map_err(|_| ProportionalFailure::InvalidInput)?;
    if !constant.is_finite() {
        return Err(ProportionalFailure::InvalidConstant);
    }
    if !input.is_finite() {
        return Err(ProportionalFailure::InvalidInput);
    }
    Ok(ProportionalModel {
        constant,
        input,
        relation: "y = constant × x".into(),
        target: "y".into(),
        source_fragment: text.trim().into(),
        assumptions: Vec::new(),
    })
}

pub fn execute_proportional_model(
    text: &str,
) -> Result<ProportionalReceipt, ProportionalFailure> {
    let model = construct_proportional_model(text)?;
    let derived_value = model.constant * model.input;
    if !derived_value.is_finite() {
        return Err(ProportionalFailure::VerificationFailed);
    }
    let replay = model.constant * model.input;
    if (derived_value - replay).abs() > 1e-12 {
        return Err(ProportionalFailure::VerificationFailed);
    }
    Ok(ProportionalReceipt {
        model,
        derived_value,
        replay_verified: true,
    })
}

pub fn execute_proportional_chain(
    text: &str,
) -> Result<ProportionalChainReceipt, ProportionalFailure> {
    let model_receipt = execute_proportional_model(text)?;
    let expression_source = format!(
        "{}*{}",
        model_receipt.model.constant, model_receipt.model.input
    );
    let expression = crate::algebra::parse(&expression_source)
        .map_err(|_| ProportionalFailure::VerificationFailed)?;
    let numeric_result = expression
        .evaluate(&[])
        .ok_or(ProportionalFailure::VerificationFailed)?;
    let replay_expression = crate::algebra::parse(&expression_source)
        .map_err(|_| ProportionalFailure::VerificationFailed)?;
    let replay_result = replay_expression
        .evaluate(&[])
        .ok_or(ProportionalFailure::VerificationFailed)?;
    if (numeric_result - replay_result).abs() > 1e-12
        || (numeric_result - model_receipt.derived_value).abs() > 1e-12
    {
        return Err(ProportionalFailure::VerificationFailed);
    }
    Ok(ProportionalChainReceipt {
        model_receipt,
        transformation: "y = constant × x".into(),
        expression_source,
        numeric_result,
        plan_steps: vec!["proportional_model_v1".into(), "expression_evaluation_v1".into()],
        replay_verified: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const POSITIVE: &str =
        "y is proportional to x with proportionality constant 3. Find y when x is 4.";

    #[test]
    fn constructs_and_replays_proportional_model() {
        let receipt = execute_proportional_chain(POSITIVE).unwrap();
        assert_eq!(receipt.numeric_result, 12.0);
        assert!(receipt.replay_verified);
    }

    #[test]
    fn missing_proportionality_constant_is_rejected() {
        let text = "y is proportional to x. Find y when x is 4.";
        assert_eq!(
            construct_proportional_model(text),
            Err(ProportionalFailure::PatternNotMatched)
        );
    }

    #[test]
    fn inferred_scaling_language_is_rejected() {
        let text = "y scales with x by 3. Find y when x is 4.";
        assert_eq!(
            construct_proportional_model(text),
            Err(ProportionalFailure::PatternNotMatched)
        );
    }

    #[test]
    fn model_spec_is_quality_gated_without_assumptions() {
        let spec = proportional_model_spec();
        assert!(spec.quality_gate.enabled());
        assert!(spec.introduced_assumptions.is_empty());
    }
}
