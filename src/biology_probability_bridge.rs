//! Guarded biology-to-probability handoff.
//!
//! Base counts become a finite distribution only when the sampling policy says
//! that a position is selected uniformly.  This bridge does not infer
//! independence, population frequencies, genotype probabilities, or any
//! stochastic process from a DNA sequence.

use super::{BiologyArtifact, BiologyResult, BiologyStatus};
use crate::probability_pack::{ProbabilityRequest, Rational};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BiologyProbabilityBridgeStatus {
    Complete,
    Ambiguous,
    Unsupported,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BiologyProbabilityHandoff {
    pub request: ProbabilityRequest,
    pub sampling_policy: String,
    pub source_biology_replay_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BiologyProbabilityBridgeResult {
    pub status: BiologyProbabilityBridgeStatus,
    pub handoff: Option<BiologyProbabilityHandoff>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("biology probability serializes"))
    )
}

fn payload(result: &BiologyProbabilityBridgeResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.handoff,
        &result.reasons,
        &result.provenance,
    )
}

fn output(
    status: BiologyProbabilityBridgeStatus,
    handoff: Option<BiologyProbabilityHandoff>,
    biology: &BiologyResult,
    reasons: Vec<String>,
) -> BiologyProbabilityBridgeResult {
    let mut result = BiologyProbabilityBridgeResult {
        status,
        handoff,
        reasons,
        provenance: biology
            .provenance
            .iter()
            .map(|value| format!("biology:{value}"))
            .collect(),
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

/// Construct a finite probability request from exact base counts only under
/// an explicit uniform-position sampling policy.
pub fn bridge_base_composition(
    biology: &BiologyResult,
    sampling_policy: Option<&str>,
) -> BiologyProbabilityBridgeResult {
    if biology.status != BiologyStatus::Complete || !biology.replay_verified() {
        let status = match biology.status {
            BiologyStatus::Ambiguous => BiologyProbabilityBridgeStatus::Ambiguous,
            BiologyStatus::Missing => BiologyProbabilityBridgeStatus::Missing,
            BiologyStatus::Unsupported
            | BiologyStatus::InvalidDomain
            | BiologyStatus::Inconsistent
            | BiologyStatus::Complete => BiologyProbabilityBridgeStatus::Unsupported,
        };
        return output(
            status,
            None,
            biology,
            vec!["only a replayable complete biology artifact may cross the bridge".into()],
        );
    }
    let Some(policy) = sampling_policy else {
        return output(
            BiologyProbabilityBridgeStatus::Ambiguous,
            None,
            biology,
            vec!["sampling policy is required before counts become probabilities".into()],
        );
    };
    if policy != "uniform_position" {
        return output(
            BiologyProbabilityBridgeStatus::Unsupported,
            None,
            biology,
            vec!["only uniform position sampling is validated".into()],
        );
    }
    let Some(BiologyArtifact::BaseComposition { length, counts, .. }) = biology.artifact.as_ref()
    else {
        return output(
            BiologyProbabilityBridgeStatus::Unsupported,
            None,
            biology,
            vec!["only base-composition artifacts have a probability handoff".into()],
        );
    };
    if *length == 0 || counts.len() != 4 {
        return output(
            BiologyProbabilityBridgeStatus::Unsupported,
            None,
            biology,
            vec!["base composition is incomplete".into()],
        );
    }
    let outcomes = vec!["A", "C", "G", "T"]
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();
    let probabilities = outcomes
        .iter()
        .map(|base| {
            Rational::new(i128::from(counts[base]), i128::from(*length)).expect("length nonzero")
        })
        .collect::<Vec<_>>();
    let request = ProbabilityRequest {
        operation: crate::probability_pack::ProbabilityOperation::DistributionConstruction,
        domain: "finite_exact_probability".into(),
        outcomes,
        probabilities,
        values: vec![0, 1, 2, 3],
        event_a: None,
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: None,
        provenance: vec!["biology-uniform-position-handoff".into()],
    };
    output(
        BiologyProbabilityBridgeStatus::Complete,
        Some(BiologyProbabilityHandoff {
            request,
            sampling_policy: policy.into(),
            source_biology_replay_hash: biology.replay_hash.clone(),
        }),
        biology,
        Vec::new(),
    )
}

impl BiologyProbabilityBridgeResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != BiologyProbabilityBridgeStatus::Complete || self.handoff.is_some())
    }

    pub fn authorized(&self) -> bool {
        self.status == BiologyProbabilityBridgeStatus::Complete && self.replay_verified()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_formula_pack::biology_pack::{
        evaluate_biology, BiologyOperation, BiologyRequest,
    };

    fn composition() -> BiologyResult {
        evaluate_biology(&BiologyRequest {
            operation: BiologyOperation::BaseComposition,
            sequence: Some("AATTGGCC".into()),
            orientation: None,
            domain: "source_derived_bounded_dna".into(),
            ambiguity: None,
            provenance: vec!["bridge-test".into()],
        })
    }

    #[test]
    fn uniform_position_is_the_only_authorized_policy() {
        let bridge = bridge_base_composition(&composition(), Some("uniform_position"));
        assert!(bridge.authorized());
        assert_eq!(
            bridge.handoff.as_ref().unwrap().request.probabilities.len(),
            4
        );
        let ambiguous = bridge_base_composition(&composition(), None);
        assert_eq!(ambiguous.status, BiologyProbabilityBridgeStatus::Ambiguous);
        let refused = bridge_base_composition(&composition(), Some("independent_bases"));
        assert_eq!(refused.status, BiologyProbabilityBridgeStatus::Unsupported);
    }
}
