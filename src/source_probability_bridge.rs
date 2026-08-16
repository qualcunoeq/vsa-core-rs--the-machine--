//! Generic bridge from source-derived scalar formulas to finite expectation.
//!
//! This bridge carries source provenance into the probability pack but never
//! infers probability semantics from a scalar list. Every source result must
//! be complete and replayable, values must be representable by the finite
//! probability artifact, and probabilities must pass that pack's validation.

use crate::probability_pack::{
    evaluate_probability, ProbabilityArtifact, ProbabilityOperation, ProbabilityRequest,
    ProbabilityResult, ProbabilityStatus, Rational,
};
use crate::source_formula_pack::FormulaResult;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceProbabilityBridgeStatus {
    Complete,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceProbabilityBridgeResult {
    pub status: SourceProbabilityBridgeStatus,
    pub expectation: Option<ProbabilityResult>,
    pub source_formula_ids: Vec<Option<String>>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("bridge serializes"))
    )
}

fn payload(result: &SourceProbabilityBridgeResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.expectation,
        &result.source_formula_ids,
        &result.reasons,
        &result.provenance,
    )
}

fn finish(
    status: SourceProbabilityBridgeStatus,
    expectation: Option<ProbabilityResult>,
    source_formula_ids: Vec<Option<String>>,
    reasons: Vec<String>,
    provenance: Vec<String>,
) -> SourceProbabilityBridgeResult {
    let mut result = SourceProbabilityBridgeResult {
        status,
        expectation,
        source_formula_ids,
        reasons,
        provenance,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

/// Compose explicitly mapped source scalar results into a finite expectation.
pub fn bridge_source_scalars_to_expectation(
    outcomes: Vec<String>,
    probabilities: Vec<Rational>,
    source_results: &[FormulaResult],
    ambiguity: Option<String>,
    provenance: Vec<String>,
) -> SourceProbabilityBridgeResult {
    let source_formula_ids = source_results
        .iter()
        .map(|result| result.formula_id.clone())
        .collect::<Vec<_>>();
    if let Some(ambiguity) = ambiguity {
        return finish(
            SourceProbabilityBridgeStatus::Ambiguous,
            None,
            source_formula_ids,
            vec![ambiguity],
            provenance,
        );
    }
    if source_results.is_empty()
        || outcomes.len() != probabilities.len()
        || outcomes.len() != source_results.len()
        || outcomes.is_empty()
        || source_results.iter().any(|result| {
            result.status != crate::source_formula_pack::FormulaStatus::Complete
                || result.value.is_none()
                || !result.replay_verified()
        })
    {
        return finish(
            SourceProbabilityBridgeStatus::Unsupported,
            None,
            source_formula_ids,
            vec!["source values are incomplete, non-replayable, or dimensionally unmapped".into()],
            provenance,
        );
    }
    let mut values = Vec::with_capacity(source_results.len());
    for result in source_results {
        let value = result.value.as_ref().unwrap();
        if value.denominator != 1
            || value.numerator < i64::MIN as i128
            || value.numerator > i64::MAX as i128
        {
            return finish(
                SourceProbabilityBridgeStatus::Unsupported,
                None,
                source_formula_ids,
                vec!["source scalar is not representable as a finite integer outcome".into()],
                provenance,
            );
        }
        values.push(value.numerator as i64);
    }
    let request = ProbabilityRequest {
        operation: ProbabilityOperation::Expectation,
        domain: "finite_exact_probability".into(),
        outcomes,
        probabilities,
        values,
        event_a: None,
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: None,
        provenance,
    };
    let expectation = evaluate_probability(&request);
    let status = if expectation.status == ProbabilityStatus::Complete
        && matches!(expectation.artifact, Some(ProbabilityArtifact::Scalar(_)))
        && expectation.replay_verified()
    {
        SourceProbabilityBridgeStatus::Complete
    } else {
        SourceProbabilityBridgeStatus::Unsupported
    };
    finish(
        status,
        Some(expectation),
        source_formula_ids,
        Vec::new(),
        request.provenance,
    )
}

impl SourceProbabilityBridgeResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && self
                .expectation
                .as_ref()
                .is_none_or(ProbabilityResult::replay_verified)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_refuses_ambiguous_and_non_integral_values() {
        let result = bridge_source_scalars_to_expectation(
            vec!["a".into()],
            vec![Rational::one()],
            &[],
            Some("mapping is ambiguous".into()),
            vec!["test".into()],
        );
        assert_eq!(result.status, SourceProbabilityBridgeStatus::Ambiguous);
        assert!(result.replay_verified());
    }
}
