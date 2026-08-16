//! Stage D benchmark for the source-derived complex-arithmetic catalog.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::probability_pack::Rational;
use the_machine::source_complex_pack::{
    evaluate_complex, ComplexArtifact, ComplexOperation, ComplexRequest, ComplexStatus, DOMAIN,
};
use the_machine::source_formula_pack::extract_formula_records;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Complete,
    Ambiguous,
    Refused,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    operation: ComplexOperation,
    expected: Expected,
    actual: ComplexStatus,
    expected_artifact: Option<ComplexArtifact>,
    actual_artifact: Option<ComplexArtifact>,
    exact: bool,
    value_correct: bool,
    source_preserved: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source: &'static str,
    source_document_sha256: String,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_artifacts: usize,
    values_correct: usize,
    source_preserved: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    mutated_documents: usize,
    mutated_documents_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    operation_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("valid rational")
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn base(operation: ComplexOperation, index: usize) -> ComplexRequest {
    let shift = (index % 7) as i128;
    ComplexRequest {
        operation,
        a: Some(q(3 + shift, 2)),
        b: Some(q(-4 + (index % 5) as i128, 3)),
        c: Some(q(2 + (index % 4) as i128, 2)),
        d: Some(q(5 - (index % 3) as i128, 3)),
        domain: DOMAIN.into(),
        ambiguity: None,
        provenance: vec!["stage-d-source-complex-independent-corpus".into()],
    }
}

fn expected(request: &ComplexRequest) -> Option<ComplexArtifact> {
    let a = request.a.as_ref()?;
    let b = request.b.as_ref()?;
    match request.operation {
        ComplexOperation::Conjugate => Some(ComplexArtifact::Pair {
            real: a.clone(),
            imag: q(-b.numerator, b.denominator),
        }),
        ComplexOperation::NormSquared => Some(ComplexArtifact::Scalar(a.mul(a)?.add(&b.mul(b)?)?)),
        ComplexOperation::Add
        | ComplexOperation::Subtract
        | ComplexOperation::Multiply
        | ComplexOperation::Divide => {
            let c = request.c.as_ref()?;
            let d = request.d.as_ref()?;
            match request.operation {
                ComplexOperation::Add => Some(ComplexArtifact::Pair {
                    real: a.add(c)?,
                    imag: b.add(d)?,
                }),
                ComplexOperation::Subtract => Some(ComplexArtifact::Pair {
                    real: a.sub(c)?,
                    imag: b.sub(d)?,
                }),
                ComplexOperation::Multiply => Some(ComplexArtifact::Pair {
                    real: a.mul(c)?.sub(&b.mul(d)?)?,
                    imag: a.mul(d)?.add(&b.mul(c)?)?,
                }),
                ComplexOperation::Divide => {
                    let denominator = c.mul(c)?.add(&d.mul(d)?)?;
                    if denominator.numerator == 0 {
                        None
                    } else {
                        Some(ComplexArtifact::Pair {
                            real: a.mul(c)?.add(&b.mul(d)?)?.div(&denominator)?,
                            imag: b.mul(c)?.sub(&a.mul(d)?)?.div(&denominator)?,
                        })
                    }
                }
                _ => None,
            }
        }
        ComplexOperation::PolarConversion => None,
    }
}

fn run(id: String, expected_kind: Expected, request: ComplexRequest) -> Receipt {
    let expected_artifact = if expected_kind == Expected::Complete {
        expected(&request)
    } else {
        None
    };
    let output = evaluate_complex(&request);
    let actual = output.status;
    let exact = match expected_kind {
        Expected::Complete => {
            actual == ComplexStatus::Complete && output.artifact == expected_artifact
        }
        Expected::Ambiguous => actual == ComplexStatus::Ambiguous && output.artifact.is_none(),
        Expected::Refused => actual != ComplexStatus::Complete && output.artifact.is_none(),
    };
    let mut tampered = output.clone();
    tampered.replay_hash.push('x');
    Receipt {
        id,
        operation: request.operation,
        expected: expected_kind,
        actual,
        expected_artifact: expected_artifact.clone(),
        actual_artifact: output.artifact.clone(),
        exact,
        value_correct: expected_kind != Expected::Complete || output.artifact == expected_artifact,
        source_preserved: expected_kind == Expected::Complete && !output.sources.is_empty(),
        replay_verified: output.replay_verified(),
        tamper_rejected: !tampered.replay_verified(),
        false_authorization: expected_kind != Expected::Complete
            && actual == ComplexStatus::Complete,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut receipts = Vec::with_capacity(240);
    let operations = [
        ComplexOperation::Add,
        ComplexOperation::Subtract,
        ComplexOperation::Multiply,
        ComplexOperation::Divide,
        ComplexOperation::Conjugate,
        ComplexOperation::NormSquared,
    ];
    for index in 0..120 {
        let operation = operations[index % operations.len()];
        receipts.push(run(
            format!("supported_{index:03}"),
            Expected::Complete,
            base(operation, index),
        ));
    }
    for index in 0..40 {
        let operation = operations[index % operations.len()];
        let mut request = base(operation, index + 200);
        request.ambiguity = Some("source notation does not select one complex operation".into());
        receipts.push(run(
            format!("ambiguous_{index:03}"),
            Expected::Ambiguous,
            request,
        ));
    }
    for index in 0..20 {
        let mut request = base(ComplexOperation::Add, index + 400);
        request.domain = "source_derived_complex_analysis".into();
        receipts.push(run(
            format!("invalid_domain_{index:03}"),
            Expected::Refused,
            request,
        ));
    }
    for index in 0..20 {
        let mut request = base(ComplexOperation::Multiply, index + 500);
        request.d = None;
        receipts.push(run(
            format!("missing_component_{index:03}"),
            Expected::Refused,
            request,
        ));
    }
    for index in 0..20 {
        let mut request = base(ComplexOperation::Divide, index + 600);
        request.c = Some(q(0, 1));
        request.d = Some(q(0, 1));
        receipts.push(run(
            format!("zero_divisor_{index:03}"),
            Expected::Refused,
            request,
        ));
    }
    for index in 0..20 {
        receipts.push(run(
            format!("polar_unsupported_{index:03}"),
            Expected::Refused,
            base(ComplexOperation::PolarConversion, index + 700),
        ));
    }
    assert_eq!(receipts.len(), 240);

    let source_document = include_str!("../../docs/sources/openstax_complex_arithmetic_source.txt");
    let mutations = [
        source_document.replacen("END FORMULA", "", 1),
        source_document.replacen("EXPRESSION: a + c", "EXPRESSION: a + unknown", 1),
        source_document.replacen(
            "ALIASES: real part of complex addition",
            "ALIASES: imaginary part of complex addition",
            1,
        ),
        source_document.replacen("EXPRESSION: a + c", "EXPRESSION: a +", 1),
        source_document.replacen(
            "EVIDENCE: Complex Numbers: Addition and Subtraction; combine real parts",
            "",
            1,
        ),
        source_document.replacen(
            "TITLE: Precalculus 2e",
            "TITLE: Precalculus 2e\nTITLE: duplicate",
            1,
        ),
    ];
    let mutated_documents_rejected = mutations
        .iter()
        .filter(|document| extract_formula_records(document).is_err())
        .count();

    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Complete)
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
    let supported_artifacts = receipts
        .iter()
        .filter(|r| r.expected == Expected::Complete && r.actual == ComplexStatus::Complete)
        .count();
    let values_correct = receipts.iter().filter(|r| r.value_correct).count();
    let source_preserved = receipts.iter().filter(|r| r.source_preserved).count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts
        .iter()
        .filter(|r| r.expected == Expected::Complete && !r.exact)
        .count();
    let mut operation_counts = BTreeMap::new();
    for receipt in &receipts {
        *operation_counts
            .entry(format!("{:?}", receipt.operation))
            .or_insert(0usize) += 1;
    }
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!(exact_decisions, cases);
    assert_eq!(supported_artifacts, supported);
    assert_eq!(values_correct, cases);
    assert_eq!(source_preserved, supported);
    assert_eq!(replay_verified, cases);
    assert_eq!(tamper_rejections, cases);
    assert_eq!(mutations.len(), 6);
    assert_eq!(mutated_documents_rejected, mutations.len());
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);

    let report = Report {
        schema: "stage-d-source-derived-complex-arithmetic-v1",
        source: "OpenStax Precalculus 2e, section 3.1 Complex Numbers",
        source_document_sha256: digest(&source_document),
        corpus_sha256: digest(&receipts),
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_artifacts,
        values_correct,
        source_preserved,
        replay_verified,
        tamper_rejections,
        mutated_documents: mutations.len(),
        mutated_documents_rejected,
        false_authorizations,
        false_denials,
        operation_counts,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage_d_source_complex_arithmetic.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
