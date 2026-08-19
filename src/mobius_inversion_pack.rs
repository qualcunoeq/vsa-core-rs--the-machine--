//! Source-derived finite Möbius inversion and divisor convolution.
//!
//! This is a deliberately bounded number-theory layer.  It consumes an
//! explicitly indexed sequence `f(1)..f(n)` and produces exact integer
//! sequences; it never infers indexing, asymptotics, analytic continuation,
//! or an infinite convolution.  The formula and its assumptions are tied to
//! the cited MIT OpenCourseWare source in every replayable result.

use crate::source_formula_pack::SourceCitation;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAX_LENGTH: usize = 32;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MobiusOperation {
    InvertFiniteSequence,
    DivisorConvolution,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MobiusStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidDomain,
    Inconsistent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MobiusArtifact {
    InvertedSequence { values: Vec<i128>, index_origin: u8 },
    ConvolutionSequence { values: Vec<i128>, index_origin: u8 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MobiusRequest {
    pub operation: MobiusOperation,
    /// Values are indexed explicitly as f(1), ..., f(n).
    pub values: Option<Vec<i128>>,
    /// The second sequence g(1), ..., g(n), used by convolution.
    pub second_values: Option<Vec<i128>>,
    pub domain: String,
    pub indexing_declared: bool,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MobiusResult {
    pub status: MobiusStatus,
    pub operation: MobiusOperation,
    pub artifact: Option<MobiusArtifact>,
    pub source: SourceCitation,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn source() -> SourceCitation {
    SourceCitation {
        source_id: "mit-ocw-18-781:mobius-inversion".into(),
        title: "Theory of Numbers, MIT OpenCourseWare 18.781".into(),
        section: "Möbius inversion and divisor convolution".into(),
        url: "https://ocw.mit.edu/courses/18-781-theory-of-numbers-spring-2012/".into(),
        license: "MIT OpenCourseWare attribution required".into(),
        retrieved_utc: "2026-08-17".into(),
        evidence_span: "finite divisor convolution and Möbius inversion identity".into(),
    }
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("mobius serializes"))
    )
}

fn payload(result: &MobiusResult) -> impl Serialize + '_ {
    (
        result.status,
        result.operation,
        &result.artifact,
        &result.source,
        &result.assumptions,
        &result.reasons,
        &result.provenance,
    )
}

fn output(
    request: &MobiusRequest,
    status: MobiusStatus,
    artifact: Option<MobiusArtifact>,
    assumptions: Vec<String>,
    reasons: Vec<String>,
) -> MobiusResult {
    let mut result = MobiusResult {
        status,
        operation: request.operation,
        artifact,
        source: source(),
        assumptions,
        reasons,
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    let replay_hash = digest(&(
        result.status,
        result.operation,
        result.artifact.clone(),
        result.source.clone(),
        result.assumptions.clone(),
        result.reasons.clone(),
        result.provenance.clone(),
    ));
    result.replay_hash = replay_hash;
    result
}

fn divisors(n: usize) -> Vec<usize> {
    (1..=n).filter(|candidate| n % candidate == 0).collect()
}

fn mobius(n: usize) -> i8 {
    if n == 1 {
        return 1;
    }
    let mut value = n;
    let mut prime = 2;
    let mut distinct = 0;
    while prime * prime <= value {
        if value % prime == 0 {
            value /= prime;
            distinct += 1;
            if value % prime == 0 {
                return 0;
            }
            while value % prime == 0 {
                value /= prime;
            }
        }
        prime += 1;
    }
    if value > 1 {
        distinct += 1;
    }
    if distinct % 2 == 0 {
        1
    } else {
        -1
    }
}

fn checked_sum(mut terms: impl Iterator<Item = Option<i128>>) -> Option<i128> {
    terms.try_fold(0i128, |sum, term| {
        term.and_then(|value| sum.checked_add(value))
    })
}

/// Evaluate a finite exact source-derived number-theory request.
pub fn evaluate(request: &MobiusRequest) -> MobiusResult {
    if request.domain != "bounded_source_mobius_inversion" {
        return output(
            request,
            MobiusStatus::InvalidDomain,
            None,
            Vec::new(),
            vec!["domain is outside the bounded source-derived Möbius contract".into()],
        );
    }
    if let Some(ambiguity) = &request.ambiguity {
        return output(
            request,
            MobiusStatus::Ambiguous,
            None,
            Vec::new(),
            vec![ambiguity.clone()],
        );
    }
    if !request.indexing_declared {
        return output(
            request,
            MobiusStatus::Ambiguous,
            None,
            Vec::new(),
            vec!["sequence index origin must be explicitly declared as one".into()],
        );
    }
    let Some(values) = request.values.as_ref() else {
        return output(
            request,
            MobiusStatus::Missing,
            None,
            Vec::new(),
            vec!["the first finite sequence is required".into()],
        );
    };
    if values.is_empty() {
        return output(
            request,
            MobiusStatus::Inconsistent,
            None,
            Vec::new(),
            vec!["the finite sequence must contain at least f(1)".into()],
        );
    }
    if values.len() > MAX_LENGTH {
        return output(
            request,
            MobiusStatus::Unsupported,
            None,
            Vec::new(),
            vec![format!(
                "finite sequence length exceeds the bound {MAX_LENGTH}"
            )],
        );
    }
    let assumptions = vec![
        "sequence values are indexed f(1) through f(n)".into(),
        format!("finite length n <= {MAX_LENGTH}"),
        "exact integer arithmetic only".into(),
        "Möbius function is the square-free divisor kernel".into(),
        "no infinite or asymptotic claim is inferred".into(),
    ];
    let values_result = match request.operation {
        MobiusOperation::InvertFiniteSequence => {
            let values = (1..=values.len())
                .map(|n| {
                    checked_sum(divisors(n).into_iter().map(|divisor| {
                        values
                            .get(n / divisor - 1)
                            .and_then(|value| i128::from(mobius(divisor)).checked_mul(*value))
                    }))
                })
                .collect::<Option<Vec<_>>>();
            values.map(|values| MobiusArtifact::InvertedSequence {
                values,
                index_origin: 1,
            })
        }
        MobiusOperation::DivisorConvolution => {
            let Some(second) = request.second_values.as_ref() else {
                return output(
                    request,
                    MobiusStatus::Missing,
                    None,
                    assumptions,
                    vec!["the second finite sequence is required for convolution".into()],
                );
            };
            if second.len() != values.len() {
                return output(
                    request,
                    MobiusStatus::Inconsistent,
                    None,
                    assumptions,
                    vec!["convolution sequences must have identical finite lengths".into()],
                );
            }
            let values = (1..=values.len())
                .map(|n| {
                    checked_sum(divisors(n).into_iter().map(|divisor| {
                        values.get(divisor - 1).and_then(|left| {
                            second
                                .get(n / divisor - 1)
                                .and_then(|right| left.checked_mul(*right))
                        })
                    }))
                })
                .collect::<Option<Vec<_>>>();
            values.map(|values| MobiusArtifact::ConvolutionSequence {
                values,
                index_origin: 1,
            })
        }
    };
    match values_result {
        Some(artifact) => output(
            request,
            MobiusStatus::Complete,
            Some(artifact),
            assumptions,
            Vec::new(),
        ),
        None => output(
            request,
            MobiusStatus::Inconsistent,
            None,
            assumptions,
            vec!["exact integer arithmetic overflowed the bounded artifact".into()],
        ),
    }
}

impl MobiusResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != MobiusStatus::Complete || self.artifact.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(operation: MobiusOperation) -> MobiusRequest {
        MobiusRequest {
            operation,
            values: Some(vec![1, 2, 3, 4]),
            second_values: Some(vec![1, 1, 1, 1]),
            domain: "bounded_source_mobius_inversion".into(),
            indexing_declared: true,
            ambiguity: None,
            provenance: vec!["mobius-unit-test".into()],
        }
    }

    #[test]
    fn inversion_and_convolution_are_exact_and_replayable() {
        let inverted = evaluate(&request(MobiusOperation::InvertFiniteSequence));
        assert_eq!(inverted.status, MobiusStatus::Complete);
        assert_eq!(
            inverted.artifact,
            Some(MobiusArtifact::InvertedSequence {
                values: vec![1, 1, 2, 2],
                index_origin: 1,
            })
        );
        assert!(inverted.replay_verified());
        let convolution = evaluate(&request(MobiusOperation::DivisorConvolution));
        assert_eq!(convolution.status, MobiusStatus::Complete);
        assert_eq!(
            convolution.artifact,
            Some(MobiusArtifact::ConvolutionSequence {
                values: vec![1, 3, 4, 7],
                index_origin: 1,
            })
        );
        assert!(convolution.replay_verified());
    }

    #[test]
    fn missing_indexing_and_tampering_fail_closed() {
        let mut request = request(MobiusOperation::InvertFiniteSequence);
        request.indexing_declared = false;
        let result = evaluate(&request);
        assert_eq!(result.status, MobiusStatus::Ambiguous);
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        assert!(!tampered.replay_verified());
    }
}
