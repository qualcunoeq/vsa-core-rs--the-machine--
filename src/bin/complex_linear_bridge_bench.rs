//! Cross-domain composition benchmark for complex arithmetic and linear algebra.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::linear_algebra_pack::LinearAlgebraArtifact;
use the_machine::probability_pack::Rational;
use the_machine::source_complex_pack::complex_linear_bridge::{
    bridge_complex_to_real_matrix, BridgeStatus, ComplexMatrixBridgeRequest, DOMAIN,
};
use the_machine::source_complex_pack::{
    evaluate_complex, ComplexArtifact, ComplexOperation, ComplexRequest, ComplexStatus,
};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    upstream_status: Option<ComplexStatus>,
    bridge_status: BridgeStatus,
    expected_matrix: Option<Vec<Vec<i64>>>,
    actual_matrix: Option<Vec<Vec<i64>>>,
    expected_norm_squared: Option<Rational>,
    actual_norm_squared: Option<Rational>,
    determinant_artifact: Option<LinearAlgebraArtifact>,
    exact: bool,
    invariant_preserved: bool,
    upstream_replay_verified: bool,
    bridge_replay_verified: bool,
    linear_algebra_replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_handoffs: usize,
    invariant_preservation: usize,
    upstream_replay_verified: usize,
    bridge_replay_verified: usize,
    linear_algebra_replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    status_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("valid rational")
}

fn source_pair(index: usize) -> (i128, i128) {
    ((index as i128 % 11) - 5, (index as i128 % 13) - 6)
}

fn source_request(index: usize) -> ComplexRequest {
    let (real, imag) = source_pair(index);
    ComplexRequest {
        operation: ComplexOperation::Conjugate,
        a: Some(q(real, 1)),
        b: Some(q(imag, 1)),
        c: None,
        d: None,
        domain: "source_derived_complex_arithmetic".into(),
        ambiguity: None,
        provenance: vec![format!("stage-b-complex-linear:{index}")],
    }
}

fn expected_pair(index: usize) -> (i64, i64) {
    let (real, imag) = source_pair(index);
    (real as i64, -imag as i64)
}

fn run_supported(index: usize) -> Receipt {
    let upstream = evaluate_complex(&source_request(index));
    let complex = upstream.artifact.clone();
    let bridge = bridge_complex_to_real_matrix(&ComplexMatrixBridgeRequest {
        complex,
        domain: DOMAIN.into(),
        ambiguity: None,
        provenance: vec![format!("stage-b-complex-linear:{index}")],
    });
    let (real, imag) = expected_pair(index);
    let expected_matrix = Some(vec![vec![real, -imag], vec![imag, real]]);
    let (a, b) = source_pair(index);
    let expected_norm = q(a * a + b * b, 1);
    let determinant = bridge
        .linear_algebra
        .as_ref()
        .and_then(|result| result.artifact.clone());
    let invariant_preserved = bridge.norm_squared == Some(expected_norm.clone())
        && determinant == Some(LinearAlgebraArtifact::Scalar(expected_norm.numerator));
    let mut tampered = bridge.clone();
    tampered.replay_hash.push('x');
    Receipt {
        id: format!("supported_{index:03}"),
        expected: Expected::Supported,
        upstream_status: Some(upstream.status),
        bridge_status: bridge.status,
        expected_matrix: expected_matrix.clone(),
        actual_matrix: bridge.matrix.clone(),
        expected_norm_squared: Some(expected_norm.clone()),
        actual_norm_squared: bridge.norm_squared.clone(),
        determinant_artifact: determinant,
        exact: upstream.status == ComplexStatus::Complete
            && bridge.status == BridgeStatus::Complete
            && bridge.matrix == expected_matrix,
        invariant_preserved,
        upstream_replay_verified: upstream.replay_verified(),
        bridge_replay_verified: bridge.replay_verified(),
        linear_algebra_replay_verified: bridge
            .linear_algebra
            .as_ref()
            .is_some_and(|result| result.replay_verified()),
        tamper_rejected: !tampered.replay_verified(),
        false_authorization: false,
    }
}

fn run_request(id: String, expected: Expected, request: ComplexMatrixBridgeRequest) -> Receipt {
    let bridge = bridge_complex_to_real_matrix(&request);
    let mut tampered = bridge.clone();
    tampered.replay_hash.push('x');
    let authorized = bridge.status == BridgeStatus::Complete && bridge.matrix.is_some();
    Receipt {
        id,
        expected,
        upstream_status: None,
        bridge_status: bridge.status,
        expected_matrix: None,
        actual_matrix: bridge.matrix.clone(),
        expected_norm_squared: None,
        actual_norm_squared: bridge.norm_squared.clone(),
        determinant_artifact: bridge
            .linear_algebra
            .as_ref()
            .and_then(|result| result.artifact.clone()),
        exact: match expected {
            Expected::Ambiguous => bridge.status == BridgeStatus::Ambiguous,
            Expected::Refused => bridge.status != BridgeStatus::Complete,
            Expected::Supported => false,
        },
        invariant_preserved: false,
        upstream_replay_verified: true,
        bridge_replay_verified: bridge.replay_verified(),
        linear_algebra_replay_verified: bridge
            .linear_algebra
            .as_ref()
            .is_some_and(|result| result.replay_verified()),
        tamper_rejected: !tampered.replay_verified(),
        false_authorization: expected != Expected::Supported && authorized,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut receipts = Vec::with_capacity(240);
    for index in 0..120 {
        receipts.push(run_supported(index));
    }
    for index in 0..40 {
        receipts.push(run_request(
            format!("ambiguous_{index:03}"),
            Expected::Ambiguous,
            ComplexMatrixBridgeRequest {
                complex: None,
                domain: DOMAIN.into(),
                ambiguity: Some("complex representation convention is unresolved".into()),
                provenance: vec![format!("stage-b-complex-linear-ambiguous:{index}")],
            },
        ));
    }
    for index in 0..20 {
        receipts.push(run_request(
            format!("fractional_{index:03}"),
            Expected::Refused,
            ComplexMatrixBridgeRequest {
                complex: Some(ComplexArtifact::Pair {
                    real: q(1, 2),
                    imag: q(3, 1),
                }),
                domain: DOMAIN.into(),
                ambiguity: None,
                provenance: vec![format!("stage-b-complex-linear-fractional:{index}")],
            },
        ));
    }
    for index in 0..20 {
        receipts.push(run_request(
            format!("scalar_{index:03}"),
            Expected::Refused,
            ComplexMatrixBridgeRequest {
                complex: Some(ComplexArtifact::Scalar(q(25, 1))),
                domain: DOMAIN.into(),
                ambiguity: None,
                provenance: vec![format!("stage-b-complex-linear-scalar:{index}")],
            },
        ));
    }
    for index in 0..20 {
        receipts.push(run_request(
            format!("invalid_domain_{index:03}"),
            Expected::Refused,
            ComplexMatrixBridgeRequest {
                complex: Some(ComplexArtifact::Pair {
                    real: q(3, 1),
                    imag: q(4, 1),
                }),
                domain: "untrusted_complex_matrix_semantics".into(),
                ambiguity: None,
                provenance: vec![format!("stage-b-complex-linear-invalid:{index}")],
            },
        ));
    }
    for index in 0..20 {
        receipts.push(run_request(
            format!("missing_{index:03}"),
            Expected::Refused,
            ComplexMatrixBridgeRequest {
                complex: None,
                domain: DOMAIN.into(),
                ambiguity: None,
                provenance: vec![format!("stage-b-complex-linear-missing:{index}")],
            },
        ));
    }
    assert_eq!(receipts.len(), 240);
    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|r| r.expected == Expected::Ambiguous)
        .count();
    let refused = receipts
        .iter()
        .filter(|r| r.expected == Expected::Refused)
        .count();
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let supported_handoffs = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && r.exact)
        .count();
    let invariant_preservation = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && r.invariant_preserved)
        .count();
    let upstream_replay_verified = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && r.upstream_replay_verified)
        .count();
    let bridge_replay_verified = receipts.iter().filter(|r| r.bridge_replay_verified).count();
    let linear_algebra_replay_verified = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && r.linear_algebra_replay_verified)
        .count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && !r.exact)
        .count();
    let mut status_counts = BTreeMap::new();
    for receipt in &receipts {
        *status_counts
            .entry(format!("{:?}", receipt.bridge_status))
            .or_insert(0usize) += 1;
    }
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!(exact_decisions, cases);
    assert_eq!(supported_handoffs, supported);
    assert_eq!(invariant_preservation, supported);
    assert_eq!(upstream_replay_verified, supported);
    assert_eq!(bridge_replay_verified, cases);
    assert_eq!(linear_algebra_replay_verified, supported);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage-b-source-complex-linear-bridge-v1",
        source: "source-derived complex arithmetic + finite exact integer linear algebra",
        corpus_sha256: format!("{:x}", Sha256::digest(serde_json::to_vec(&receipts)?)),
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_handoffs,
        invariant_preservation,
        upstream_replay_verified,
        bridge_replay_verified,
        linear_algebra_replay_verified,
        tamper_rejections,
        false_authorizations,
        false_denials,
        status_counts,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage_b_source_complex_linear_bridge.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
