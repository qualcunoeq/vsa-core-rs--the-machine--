//! Stage 184: independent bounded complex-analysis curriculum gate.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::bounded_complex_analysis_pack::{
    evaluate_complex_analysis, replay_verified, ComplexAnalysisOperation, ComplexAnalysisRequest,
    ComplexAnalysisStatus, ComplexNumber, DOMAIN,
};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::probability_pack::Rational;

const SOURCE: &str =
    include_str!("../../docs/sources/openstax_bounded_complex_analysis_source.txt");
const REPORT_JSON: &str = "docs/stage184_bounded_complex_analysis.json";
const REPORT_MD: &str = "docs/stage184_bounded_complex_analysis.md";

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: String,
    actual: String,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_sha256: String,
    manifest_before_sha256: String,
    manifest_after_sha256: String,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_artifacts: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<Receipt>,
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("valid rational")
}

fn z(real: i128, imag: i128) -> ComplexNumber {
    ComplexNumber::new(q(real, 1), q(imag, 1))
}

fn polynomial_request(operation: ComplexAnalysisOperation, index: usize) -> ComplexAnalysisRequest {
    ComplexAnalysisRequest {
        operation,
        coefficients: (0..=(index % 4))
            .map(|power| z(power as i128 + 1, (index % 3) as i128))
            .collect(),
        point: Some(z((index % 5) as i128, (index % 2) as i128)),
        ux: None,
        uy: None,
        vx: None,
        vy: None,
        domain: DOMAIN.into(),
        ambiguity: None,
        provenance: vec![format!("stage184-independent-case:{index}")],
    }
}

fn affine_request(operation: ComplexAnalysisOperation, index: usize) -> ComplexAnalysisRequest {
    ComplexAnalysisRequest {
        operation,
        coefficients: Vec::new(),
        point: None,
        ux: Some(q(2, 1)),
        uy: Some(q(-1, 1)),
        vx: Some(q(1, 1)),
        vy: Some(q(2, 1)),
        domain: DOMAIN.into(),
        ambiguity: None,
        provenance: vec![format!("stage184-independent-affine:{index}")],
    }
}

fn ambiguous_request(index: usize) -> ComplexAnalysisRequest {
    let mut request = polynomial_request(ComplexAnalysisOperation::PolynomialValue, index);
    request.ambiguity = Some("analytic interpretation remains unresolved".into());
    request
}

fn refused_request(index: usize) -> ComplexAnalysisRequest {
    let operation = match index % 4 {
        0 => ComplexAnalysisOperation::PolarConversion,
        1 => ComplexAnalysisOperation::ContourIntegral,
        2 => ComplexAnalysisOperation::InfiniteSeries,
        _ => ComplexAnalysisOperation::PolynomialValue,
    };
    let mut request = polynomial_request(operation, index);
    if index % 4 == 3 {
        request.coefficients = (0..=5).map(|power| z(power as i128, 0)).collect();
    }
    request
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("stage184 serializes"))
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_before_sha256 = breadth_first_manifest().replay_hash();
    let source_sha256 = format!("{:x}", Sha256::digest(SOURCE.as_bytes()));
    let mut receipts = Vec::with_capacity(240);
    let mut exact_decisions = 0;
    let mut supported_artifacts = 0;
    let mut replay_count = 0;
    let mut tamper_count = 0;

    for index in 0..240 {
        let (expected, request) = if index < 60 {
            (
                "supported",
                polynomial_request(ComplexAnalysisOperation::PolynomialValue, index),
            )
        } else if index < 100 {
            (
                "supported",
                polynomial_request(ComplexAnalysisOperation::PolynomialDerivative, index),
            )
        } else if index < 120 {
            let operation = if index % 2 == 0 {
                ComplexAnalysisOperation::CauchyRiemannCheck
            } else {
                ComplexAnalysisOperation::AffineHolomorphicDerivative
            };
            ("supported", affine_request(operation, index))
        } else if index < 160 {
            ("ambiguous", ambiguous_request(index))
        } else {
            ("refused", refused_request(index))
        };
        let result = evaluate_complex_analysis(&request);
        let actual = format!("{:?}", result.status);
        let expected_status = match expected {
            "supported" => Some(ComplexAnalysisStatus::Complete),
            "ambiguous" => Some(ComplexAnalysisStatus::Ambiguous),
            _ => None,
        };
        let exact = expected_status
            .map_or(result.status != ComplexAnalysisStatus::Complete, |status| {
                result.status == status
            });
        let replay = replay_verified(&result);
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        let tamper_rejected = !replay_verified(&tampered);
        let authorized =
            result.status == ComplexAnalysisStatus::Complete && result.artifact.is_some();
        let false_authorization = expected != "supported" && authorized;
        let false_denial = expected == "supported" && !authorized;
        exact_decisions += usize::from(exact);
        supported_artifacts += usize::from(expected == "supported" && authorized);
        replay_count += usize::from(replay);
        tamper_count += usize::from(tamper_rejected);
        receipts.push(Receipt {
            id: format!("complex_analysis_{index:03}"),
            expected: expected.into(),
            actual,
            exact,
            replay_verified: replay,
            tamper_rejected,
            false_authorization,
            false_denial,
        });
    }

    let manifest_after_sha256 = breadth_first_manifest().replay_hash();
    let corpus_sha256 = digest(&receipts);
    let false_authorizations = receipts
        .iter()
        .filter(|receipt| receipt.false_authorization)
        .count();
    let false_denials = receipts
        .iter()
        .filter(|receipt| receipt.false_denial)
        .count();
    assert_eq!(exact_decisions, 240);
    assert_eq!(supported_artifacts, 120);
    assert_eq!(replay_count, 240);
    assert_eq!(tamper_count, 240);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    assert_eq!(manifest_before_sha256, manifest_after_sha256);

    let report = Report {
        schema: "stage184-bounded-complex-analysis-v1",
        source_sha256: source_sha256.clone(),
        manifest_before_sha256,
        manifest_after_sha256,
        corpus_sha256,
        cases: 240,
        supported: 120,
        ambiguous: 40,
        refused: 80,
        exact_decisions,
        supported_artifacts,
        replay_verified: replay_count,
        tamper_rejections: tamper_count,
        false_authorizations,
        false_denials,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    let markdown = format!(
        "# Stage 184 — bounded exact complex analysis\n\nA source-attributed theorem contract extends validated complex arithmetic and calculus without authorizing polar, contour, infinite, or approximate semantics.\n\n| Measure | Result |\n|---|---:|\n| Cases | 240 |\n| Supported / ambiguous / refused | 120 / 40 / 80 |\n| Exact decisions | {exact_decisions}/240 |\n| Supported artifacts | {supported_artifacts}/120 |\n| Replay / tamper | {replay_count}/240 / {tamper_count}/240 |\n| False authorizations / denials | {false_authorizations} / {false_denials} |\n| Manifest mutation | false |\n\nSource SHA-256: `{source_sha256}`\n\nMachine-readable report: `{REPORT_JSON}`\n"
    );
    fs::write(REPORT_MD, markdown)?;
    println!("{serialized}");
    Ok(())
}
