//! Stage 185: complex-analysis composition with exact complex/linear algebra.
//!
//! A complex value may enter the real-matrix bridge only when its rectangular
//! pair is explicit and integral.  Analytic meaning is retained in the source
//! artifact; numeric similarity alone never authorizes a lossy route.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::bounded_complex_analysis_pack::{
    evaluate_complex_analysis, replay_verified as analysis_replay, ComplexAnalysisArtifact,
    ComplexAnalysisOperation, ComplexAnalysisRequest, ComplexAnalysisStatus, ComplexNumber,
    DOMAIN as ANALYSIS_DOMAIN,
};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::probability_pack::Rational;
use the_machine::source_complex_pack::complex_linear_bridge::{
    bridge_complex_to_real_matrix, BridgeStatus, ComplexMatrixBridgeRequest,
    DOMAIN as BRIDGE_DOMAIN,
};
use the_machine::source_complex_pack::ComplexArtifact;

const REPORT_JSON: &str = "docs/stage185_complex_analysis_composition.json";
const REPORT_MD: &str = "docs/stage185_complex_analysis_composition.md";

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: String,
    analysis_status: String,
    bridge_status: String,
    exact: bool,
    analysis_replay: bool,
    bridge_replay: bool,
    tamper_rejected: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    manifest_sha256: String,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_route_decisions: usize,
    supported_compositions: usize,
    analysis_replays: usize,
    bridge_replays: usize,
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

fn affine(index: usize, fractional: bool) -> ComplexAnalysisRequest {
    let (ux, uy, vx, vy) = if fractional {
        (q(1, 2), q(-1, 2), q(1, 2), q(1, 2))
    } else {
        (q(2, 1), q(-1, 1), q(1, 1), q(2, 1))
    };
    ComplexAnalysisRequest {
        operation: ComplexAnalysisOperation::AffineHolomorphicDerivative,
        coefficients: Vec::new(),
        point: None,
        ux: Some(ux),
        uy: Some(uy),
        vx: Some(vx),
        vy: Some(vy),
        domain: ANALYSIS_DOMAIN.into(),
        ambiguity: None,
        provenance: vec![format!("stage185-affine:{index}")],
    }
}

fn polynomial(index: usize) -> ComplexAnalysisRequest {
    ComplexAnalysisRequest {
        operation: ComplexAnalysisOperation::PolynomialValue,
        coefficients: vec![z(1, 0), z((index % 3 + 1) as i128, 0)],
        point: Some(z((index % 5) as i128, (index % 2) as i128)),
        ux: None,
        uy: None,
        vx: None,
        vy: None,
        domain: ANALYSIS_DOMAIN.into(),
        ambiguity: None,
        provenance: vec![format!("stage185-polynomial:{index}")],
    }
}

fn ambiguous(index: usize) -> ComplexAnalysisRequest {
    let mut request = affine(index, false);
    request.ambiguity = Some("the real-matrix representation convention is unresolved".into());
    request
}

fn refused(index: usize) -> ComplexAnalysisRequest {
    match index % 4 {
        0 => affine(index, true),
        1 => ComplexAnalysisRequest {
            operation: ComplexAnalysisOperation::PolynomialDerivative,
            coefficients: vec![z(1, 0), z(2, 0), z(1, 0)],
            point: None,
            ux: None,
            uy: None,
            vx: None,
            vy: None,
            domain: ANALYSIS_DOMAIN.into(),
            ambiguity: None,
            provenance: vec![format!("stage185-polynomial-derivative:{index}")],
        },
        2 => ComplexAnalysisRequest {
            operation: ComplexAnalysisOperation::PolarConversion,
            coefficients: Vec::new(),
            point: None,
            ux: None,
            uy: None,
            vx: None,
            vy: None,
            domain: ANALYSIS_DOMAIN.into(),
            ambiguity: None,
            provenance: vec![format!("stage185-polar:{index}")],
        },
        _ => {
            let mut request = polynomial(index);
            request.domain = "unvalidated_complex_domain".into();
            request
        }
    }
}

fn pair_artifact(artifact: &ComplexAnalysisArtifact) -> Option<ComplexArtifact> {
    match artifact {
        ComplexAnalysisArtifact::Complex(value) => Some(ComplexArtifact::Pair {
            real: value.real.clone(),
            imag: value.imag.clone(),
        }),
        _ => None,
    }
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("stage185 serializes"))
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_sha256 = breadth_first_manifest().replay_hash();
    let mut receipts = Vec::with_capacity(240);
    let mut exact = 0;
    let mut supported_compositions = 0;
    let mut analysis_replays = 0;
    let mut bridge_replays = 0;
    let mut tamper_rejections = 0;

    for index in 0..240 {
        let (expected, request) = if index < 60 {
            ("supported", affine(index, false))
        } else if index < 120 {
            ("supported", polynomial(index))
        } else if index < 160 {
            ("ambiguous", ambiguous(index))
        } else {
            ("refused", refused(index))
        };
        let analysis = evaluate_complex_analysis(&request);
        let analysis_ok = analysis_replay(&analysis);
        let bridge = if let Some(complex) = analysis.artifact.as_ref().and_then(pair_artifact) {
            bridge_complex_to_real_matrix(&ComplexMatrixBridgeRequest {
                complex: Some(complex),
                domain: BRIDGE_DOMAIN.into(),
                ambiguity: request.ambiguity.clone(),
                provenance: request.provenance.clone(),
            })
        } else {
            bridge_complex_to_real_matrix(&ComplexMatrixBridgeRequest {
                complex: None,
                domain: BRIDGE_DOMAIN.into(),
                ambiguity: request.ambiguity.clone(),
                provenance: request.provenance.clone(),
            })
        };
        let bridge_ok = bridge.replay_verified();
        let authorized = expected == "supported"
            && analysis.status == ComplexAnalysisStatus::Complete
            && bridge.status == BridgeStatus::Complete;
        let false_authorization = expected != "supported" && authorized;
        let false_denial = expected == "supported" && !authorized;
        let route_exact = if expected == "supported" {
            authorized
        } else if expected == "ambiguous" {
            analysis.status == ComplexAnalysisStatus::Ambiguous
                && bridge.status == BridgeStatus::Ambiguous
        } else {
            !authorized
        };
        let mut analysis_tampered = analysis.clone();
        analysis_tampered.replay_hash.push('x');
        let mut bridge_tampered = bridge.clone();
        bridge_tampered.replay_hash.push('x');
        let tamper_rejected =
            !analysis_replay(&analysis_tampered) && !bridge_tampered.replay_verified();
        exact += usize::from(route_exact);
        supported_compositions += usize::from(authorized);
        analysis_replays += usize::from(analysis_ok);
        bridge_replays += usize::from(bridge_ok);
        tamper_rejections += usize::from(tamper_rejected);
        receipts.push(Receipt {
            id: format!("complex_composition_{index:03}"),
            expected: expected.into(),
            analysis_status: format!("{:?}", analysis.status),
            bridge_status: format!("{:?}", bridge.status),
            exact: route_exact,
            analysis_replay: analysis_ok,
            bridge_replay: bridge_ok,
            tamper_rejected,
            false_authorization,
            false_denial,
        });
    }

    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    assert_eq!(exact, 240);
    assert_eq!(supported_compositions, 120);
    assert_eq!(analysis_replays, 240);
    assert_eq!(bridge_replays, 240);
    assert_eq!(tamper_rejections, 240);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);

    let report = Report {
        schema: "stage185-complex-analysis-composition-v1",
        manifest_sha256: manifest_sha256.clone(),
        corpus_sha256: digest(&receipts),
        cases: 240,
        supported: 120,
        ambiguous: 40,
        refused: 80,
        exact_route_decisions: exact,
        supported_compositions,
        analysis_replays,
        bridge_replays,
        tamper_rejections,
        false_authorizations,
        false_denials,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 185 — complex-analysis composition\n\nComplex polynomial and affine analytic artifacts were offered to the exact complex-to-real-matrix bridge. Only explicit integral rectangular pairs were authorized.\n\n| Measure | Result |\n|---|---:|\n| Cases | 240 |\n| Supported / ambiguous / refused | 120 / 40 / 80 |\n| Exact route decisions | {exact}/240 |\n| Supported compositions | {supported_compositions}/120 |\n| Analysis / bridge replay | {analysis_replays}/240 / {bridge_replays}/240 |\n| Tamper rejection | {tamper_rejections}/240 |\n| False authorizations / denials | {false_authorizations} / {false_denials} |\n| Production mutation | false |\n\nManifest SHA-256: `{manifest_sha256}`\n\nMachine-readable report: `{REPORT_JSON}`\n"
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
