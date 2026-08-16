//! Domain-specific technical-language frontend for the bounded spectral pack.
//!
//! It accepts only explicit small integer matrix literals and explicit target
//! operations.  It never infers a matrix, an eigenvalue, or an exponent from
//! specialist vocabulary alone.

use crate::spectral_linear_algebra_pack::{SpectralOperation, SpectralRequest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpectralFrontendStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpectralFrontendResult {
    pub status: SpectralFrontendStatus,
    pub request: Option<SpectralRequest>,
    pub operation: Option<SpectralOperation>,
    pub matrix_span: Option<String>,
    pub provenance_spans: Vec<String>,
    pub alternatives: Vec<String>,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn payload(result: &SpectralFrontendResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.request,
        result.operation,
        &result.matrix_span,
        &result.provenance_spans,
        &result.alternatives,
        &result.reasons,
    )
}

fn make(
    status: SpectralFrontendStatus,
    request: Option<SpectralRequest>,
    operation: Option<SpectralOperation>,
    matrix_span: Option<String>,
    provenance_spans: Vec<String>,
    alternatives: Vec<String>,
    reasons: Vec<String>,
) -> SpectralFrontendResult {
    let mut result = SpectralFrontendResult {
        status,
        request,
        operation,
        matrix_span,
        provenance_spans,
        alternatives,
        reasons,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn parse_matrix(text: &str) -> Option<(Vec<Vec<i64>>, String)> {
    let start = text.find("[[")?;
    let end = text[start + 2..]
        .find("]]")
        .map(|index| start + 2 + index)?;
    let raw = &text[start..=end + 1];
    let inner = raw
        .trim_start_matches('[')
        .trim_end_matches(']')
        .replace("], [", "],[");
    let mut rows = Vec::new();
    for row in inner.split("],[").map(|row| row.trim_matches(['[', ']'])) {
        let values = row
            .split(',')
            .map(|value| value.trim().parse::<i64>().ok())
            .collect::<Option<Vec<_>>>()?;
        if values.is_empty() {
            return None;
        }
        rows.push(values);
    }
    if rows.is_empty() || rows.iter().any(|row| row.len() != rows.len()) {
        return None;
    }
    Some((rows, raw.into()))
}

fn labeled_i64(text: &str, label: &str) -> Option<i64> {
    let lower = text.to_ascii_lowercase();
    let marker = format!("{label}=");
    let start = lower.find(&marker)? + marker.len();
    let token: String = lower[start..]
        .chars()
        .take_while(|character| character.is_ascii_digit() || *character == '-')
        .collect();
    (!token.is_empty()).then(|| token.parse().ok()).flatten()
}

/// Convert one explicit technical report into a bounded spectral request.
pub fn formalize_spectral_text(text: &str) -> SpectralFrontendResult {
    let lower = text.to_ascii_lowercase();
    if [
        "approx",
        "numerical",
        "infinite-dimensional",
        "functional analysis",
        "spectral gap",
        "complex spectrum",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        return make(
            SpectralFrontendStatus::Unsupported,
            None,
            None,
            None,
            vec![text.into()],
            Vec::new(),
            vec!["request exceeds exact bounded spectral language boundary".into()],
        );
    }
    let Some((matrix, matrix_span)) = parse_matrix(&lower) else {
        return make(
            SpectralFrontendStatus::Missing,
            None,
            None,
            None,
            vec![text.into()],
            Vec::new(),
            vec!["an explicit square integer matrix literal is required".into()],
        );
    };
    let mut operations = Vec::new();
    if lower.contains("characteristic polynomial") {
        operations.push(SpectralOperation::CharacteristicPolynomial);
    }
    if lower.contains("eigenspace") || lower.contains("eigenvector") {
        operations.push(SpectralOperation::Eigenspace);
    } else if lower.contains("eigenvalue") || lower.contains("spectrum") {
        operations.push(SpectralOperation::IntegerEigenvalues);
    }
    if lower.contains("diagonalizable") || lower.contains("diagonalizability") {
        operations.push(SpectralOperation::Diagonalizability);
    }
    if lower.contains("spectral decomposition") {
        operations.push(SpectralOperation::SpectralDecomposition);
    }
    if lower.contains("matrix power") || lower.contains("power of the matrix") {
        operations.push(SpectralOperation::MatrixPower);
    }
    operations.sort_by_key(|operation| *operation as u8);
    operations.dedup();
    if operations.len() != 1 {
        return make(
            SpectralFrontendStatus::Ambiguous,
            None,
            None,
            Some(matrix_span),
            vec![text.into()],
            operations
                .iter()
                .map(|operation| format!("{operation:?}"))
                .collect(),
            vec!["the requested spectral operation is not unique".into()],
        );
    }
    let operation = operations[0];
    let eigenvalue = labeled_i64(&lower, "eigenvalue");
    let power = labeled_i64(&lower, "power").and_then(|value| u32::try_from(value).ok());
    if operation == SpectralOperation::Eigenspace && eigenvalue.is_none() {
        return make(
            SpectralFrontendStatus::Missing,
            None,
            Some(operation),
            Some(matrix_span),
            vec![text.into()],
            Vec::new(),
            vec!["eigenspace requests require an explicit eigenvalue".into()],
        );
    }
    if operation == SpectralOperation::MatrixPower && power.is_none() {
        return make(
            SpectralFrontendStatus::Missing,
            None,
            Some(operation),
            Some(matrix_span),
            vec![text.into()],
            Vec::new(),
            vec!["matrix-power requests require an explicit finite power".into()],
        );
    }
    let request = SpectralRequest {
        operation,
        matrix: Some(matrix),
        eigenvalue,
        power,
        domain: "bounded_exact_spectral_linear_algebra".into(),
        ambiguity: None,
        provenance: vec![format!("matrix:{matrix_span}"), text.into()],
    };
    make(
        SpectralFrontendStatus::Complete,
        Some(request),
        Some(operation),
        Some(matrix_span),
        vec![text.into()],
        Vec::new(),
        Vec::new(),
    )
}

impl SpectralFrontendResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_matrix_and_target_are_required() {
        let result = formalize_spectral_text("Find the eigenvalues of [[2,0],[0,5]].");
        assert_eq!(result.status, SpectralFrontendStatus::Complete);
        assert!(result.replay_verified());
        let missing = formalize_spectral_text("Find the eigenvalues of A.");
        assert_eq!(missing.status, SpectralFrontendStatus::Missing);
    }

    #[test]
    fn eigenspace_phrase_is_not_confused_with_eigenvalue_operation() {
        let result =
            formalize_spectral_text("Find the eigenspace for eigenvalue=3 of [[2,1],[1,2]].");
        assert_eq!(result.status, SpectralFrontendStatus::Complete);
        assert_eq!(result.operation, Some(SpectralOperation::Eigenspace));
        assert!(result.replay_verified());
    }
}
