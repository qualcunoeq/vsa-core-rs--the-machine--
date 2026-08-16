//! Stage C benchmark for the bounded source-derived complex-language frontend.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::probability_pack::Rational;
use the_machine::source_complex_pack::{
    evaluate_complex, ComplexArtifact, ComplexOperation, ComplexStatus,
};
use the_machine::source_complex_pack::source_complex_frontend::{
    formalize_complex_text, FrontendStatus,
};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    frontend_status: FrontendStatus,
    downstream_status: Option<ComplexStatus>,
    operation: Option<ComplexOperation>,
    expected_artifact: Option<ComplexArtifact>,
    actual_artifact: Option<ComplexArtifact>,
    exact: bool,
    value_correct: bool,
    provenance_preserved: bool,
    frontend_replay_verified: bool,
    downstream_replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_pack: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    complete_frontends: usize,
    downstream_artifacts: usize,
    values_correct: usize,
    provenance_preserved: usize,
    frontend_replay_verified: usize,
    downstream_replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    status_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("valid rational")
}

fn expected(operation: ComplexOperation) -> Option<ComplexArtifact> {
    match operation {
        ComplexOperation::Add => Some(ComplexArtifact::Pair {
            real: q(5, 1),
            imag: q(1, 1),
        }),
        ComplexOperation::Subtract => Some(ComplexArtifact::Pair {
            real: q(1, 1),
            imag: q(-9, 1),
        }),
        ComplexOperation::Multiply => Some(ComplexArtifact::Pair {
            real: q(26, 1),
            imag: q(7, 1),
        }),
        ComplexOperation::Divide => Some(ComplexArtifact::Pair {
            real: q(-14, 1).div(&q(29, 1)).unwrap(),
            imag: q(-23, 1).div(&q(29, 1)).unwrap(),
        }),
        ComplexOperation::Conjugate => Some(ComplexArtifact::Pair {
            real: q(3, 1),
            imag: q(4, 1),
        }),
        ComplexOperation::NormSquared => Some(ComplexArtifact::Scalar(q(25, 1))),
        ComplexOperation::PolarConversion => None,
    }
}

fn operation_word(operation: ComplexOperation) -> &'static str {
    match operation {
        ComplexOperation::Add => "sum",
        ComplexOperation::Subtract => "difference",
        ComplexOperation::Multiply => "product",
        ComplexOperation::Divide => "quotient",
        ComplexOperation::Conjugate => "conjugate",
        ComplexOperation::NormSquared => "norm squared",
        ComplexOperation::PolarConversion => "polar form",
    }
}

fn run(
    id: String,
    text: String,
    expected_kind: Expected,
    expected_operation: Option<ComplexOperation>,
) -> Receipt {
    let frontend = formalize_complex_text(&text);
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    let frontend_replay_verified = frontend.replay_verified();
    let tamper_rejected = !tampered.replay_verified();
    let frontend_exact = match expected_kind {
        Expected::Supported => {
            frontend.status == FrontendStatus::Complete
                && frontend.operation == expected_operation
                && frontend.request.is_some()
        }
        Expected::Ambiguous => frontend.status == FrontendStatus::Ambiguous,
        Expected::Unsupported => matches!(
            frontend.status,
            FrontendStatus::Unsupported | FrontendStatus::Missing
        ),
    };
    let expected_artifact = expected_operation.and_then(expected);
    let (downstream_status, actual_artifact, downstream_replay_verified, value_correct) =
        if frontend.status == FrontendStatus::Complete {
            let output = evaluate_complex(frontend.request.as_ref().expect("complete request"));
            (
                Some(output.status),
                output.artifact.clone(),
                output.replay_verified(),
                output.artifact == expected_artifact,
            )
        } else {
            (None, None, true, true)
        };
    let authorized =
        downstream_status == Some(ComplexStatus::Complete) && actual_artifact.is_some();
    Receipt {
        id,
        expected: expected_kind,
        frontend_status: frontend.status,
        downstream_status,
        operation: frontend.operation,
        expected_artifact,
        actual_artifact,
        exact: frontend_exact && (expected_kind != Expected::Supported || value_correct),
        value_correct,
        provenance_preserved: !frontend.provenance_spans.is_empty()
            && frontend
                .request
                .as_ref()
                .map_or(true, |request| !request.provenance.is_empty()),
        frontend_replay_verified,
        downstream_replay_verified,
        tamper_rejected,
        false_authorization: expected_kind != Expected::Supported && authorized,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let operations = [
        ComplexOperation::Add,
        ComplexOperation::Subtract,
        ComplexOperation::Multiply,
        ComplexOperation::Divide,
        ComplexOperation::Conjugate,
        ComplexOperation::NormSquared,
    ];
    let mut receipts = Vec::with_capacity(240);
    for index in 0..120 {
        let operation = operations[index % operations.len()];
        let text = match operation {
            ComplexOperation::Conjugate | ComplexOperation::NormSquared => {
                format!("Compute the {} of (3-4i).", operation_word(operation))
            }
            _ => format!(
                "Compute the {} of (3-4i) and (2+5i).",
                operation_word(operation)
            ),
        };
        receipts.push(run(
            format!("supported_{index:03}"),
            text,
            Expected::Supported,
            Some(operation),
        ));
    }
    for index in 0..40 {
        let text = if index % 2 == 0 {
            "Find the sum or product of (3-4i) and (2+5i).".to_string()
        } else {
            "Should we add or subtract (3-4i) and (2+5i)?".to_string()
        };
        receipts.push(run(
            format!("ambiguous_{index:03}"),
            text,
            Expected::Ambiguous,
            None,
        ));
    }
    let unsupported = [
        "Convert (3-4i) to polar form.",
        "Find the argument of (3-4i).",
        "Compute the decimal approximation of the product of (3-4i) and (2+5i).",
        "Evaluate the exponential of (3-4i).",
        "Compute the product of (3.0-4.0i) and (2+5i).",
        "Compute the product of (3-4i).",
    ];
    for index in 0..80 {
        receipts.push(run(
            format!("unsupported_{index:03}"),
            unsupported[index % unsupported.len()].to_string(),
            Expected::Unsupported,
            None,
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
    let unsupported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Unsupported)
        .count();
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let complete_frontends = receipts
        .iter()
        .filter(|r| r.frontend_status == FrontendStatus::Complete)
        .count();
    let downstream_artifacts = receipts
        .iter()
        .filter(|r| r.actual_artifact.is_some())
        .count();
    let values_correct = receipts.iter().filter(|r| r.value_correct).count();
    let provenance_preserved = receipts.iter().filter(|r| r.provenance_preserved).count();
    let frontend_replay_verified = receipts
        .iter()
        .filter(|r| r.frontend_replay_verified)
        .count();
    let downstream_replay_verified = receipts
        .iter()
        .filter(|r| r.downstream_replay_verified)
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
            .entry(format!("{:?}", receipt.frontend_status))
            .or_insert(0usize) += 1;
    }
    assert_eq!((supported, ambiguous, unsupported), (120, 40, 80));
    assert_eq!(exact_decisions, cases);
    assert_eq!(complete_frontends, supported);
    assert_eq!(downstream_artifacts, supported);
    assert_eq!(values_correct, cases);
    assert_eq!(provenance_preserved, cases);
    assert_eq!(frontend_replay_verified, cases);
    assert_eq!(downstream_replay_verified, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage-c-source-derived-complex-frontend-v1",
        source_pack: "source_derived_complex_arithmetic",
        corpus_sha256: format!("{:x}", Sha256::digest(serde_json::to_vec(&receipts)?)),
        cases,
        supported,
        ambiguous,
        unsupported,
        exact_decisions,
        complete_frontends,
        downstream_artifacts,
        values_correct,
        provenance_preserved,
        frontend_replay_verified,
        downstream_replay_verified,
        tamper_rejections,
        false_authorizations,
        false_denials,
        status_counts,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage_c_source_complex_frontend.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
