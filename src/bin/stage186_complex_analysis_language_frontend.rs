//! Stage 186: shifted technical-language transfer for bounded complex analysis.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::bounded_complex_analysis_frontend::{
    formalize, replay_verified as frontend_replay, FrontendStatus,
};
use the_machine::bounded_complex_analysis_pack::{
    evaluate_complex_analysis, replay_verified as analysis_replay, ComplexAnalysisArtifact,
    ComplexAnalysisStatus,
};
use the_machine::curriculum::breadth_first_manifest;
use the_machine::source_complex_pack::complex_linear_bridge::{
    bridge_complex_to_real_matrix, BridgeStatus, ComplexMatrixBridgeRequest,
    DOMAIN as BRIDGE_DOMAIN,
};
use the_machine::source_complex_pack::ComplexArtifact;

const REPORT_JSON: &str = "docs/stage186_complex_analysis_language_frontend.json";
const REPORT_MD: &str = "docs/stage186_complex_analysis_language_frontend.md";

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: String,
    frontend_status: String,
    analysis_status: String,
    bridge_status: String,
    exact: bool,
    frontend_replay: bool,
    analysis_replay: bool,
    bridge_replay: bool,
    downstream_artifact: bool,
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
    unsupported_or_missing: usize,
    exact_decisions: usize,
    frontend_replay_verified: usize,
    analysis_replay_verified: usize,
    bridge_replay_verified: usize,
    downstream_artifacts: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<Receipt>,
}

fn supported_text(index: usize, derivative: bool) -> String {
    if derivative {
        format!(
            "Differentiate the affine map after checking Cauchy Riemann: v_y=2; u_x=2; v_x=1; u_y=-1."
        )
    } else if index % 2 == 0 {
        "Check the Cauchy-Riemann equations: ux=2, uy=-1, vx=1, vy=2.".into()
    } else {
        "Verify Cauchy riemann for this affine map, with v_y = 2, u_y = -1, u_x = 2, v_x = 1."
            .into()
    }
}

fn ambiguous_text(index: usize) -> String {
    if index % 2 == 0 {
        "Maybe either check Cauchy-Riemann or compute the derivative; the intended operation is unclear.".into()
    } else {
        "Possibly use Cauchy riemann, but the report does not decide whether a check or derivative is requested.".into()
    }
}

fn unsupported_text(index: usize) -> String {
    match index % 4 {
        0 => "Use a contour integral after checking Cauchy-Riemann.".into(),
        1 => "Find the polar branch and argument of the analytic function.".into(),
        2 => "Check Cauchy-Riemann with ux=2 and uy=-1 only.".into(),
        _ => "Determine the complex behavior of this function.".into(),
    }
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("stage186 serializes"))
    )
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_sha256 = breadth_first_manifest().replay_hash();
    let mut receipts = Vec::with_capacity(240);
    let mut exact = 0;
    let mut frontend_replays = 0;
    let mut analysis_replays = 0;
    let mut bridge_replays = 0;
    let mut downstream_artifacts = 0;
    let mut tamper_rejections = 0;

    for index in 0..240 {
        let (expected, text) = if index < 60 {
            ("supported", supported_text(index, false))
        } else if index < 120 {
            ("supported", supported_text(index, true))
        } else if index < 160 {
            ("ambiguous", ambiguous_text(index))
        } else {
            ("unsupported_or_missing", unsupported_text(index))
        };
        let frontend = formalize(&text, &format!("case-{index:03}"));
        let analysis = frontend.request.as_ref().map(evaluate_complex_analysis);
        let bridge = analysis
            .as_ref()
            .and_then(|result| result.artifact.as_ref().and_then(pair_artifact))
            .map(|complex| {
                bridge_complex_to_real_matrix(&ComplexMatrixBridgeRequest {
                    complex: Some(complex),
                    domain: BRIDGE_DOMAIN.into(),
                    ambiguity: None,
                    provenance: vec![format!("stage186:{index}")],
                })
            });
        let frontend_ok = frontend_replay(&frontend);
        let analysis_ok = analysis.as_ref().is_none_or(analysis_replay);
        let bridge_ok = bridge
            .as_ref()
            .is_none_or(|result| result.replay_verified());
        let analytic_complete = frontend.status == FrontendStatus::Complete
            && analysis
                .as_ref()
                .is_some_and(|result| result.status == ComplexAnalysisStatus::Complete);
        let bridge_complete = bridge
            .as_ref()
            .is_none_or(|result| result.status == BridgeStatus::Complete);
        let authorized = expected == "supported" && analytic_complete && bridge_complete;
        let false_authorization = expected != "supported" && analytic_complete;
        let false_denial = expected == "supported" && !authorized;
        let exact_status = match expected {
            "supported" => authorized,
            "ambiguous" => frontend.status == FrontendStatus::Ambiguous,
            _ => frontend.status != FrontendStatus::Complete,
        };
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        let mut analysis_tampered = analysis.clone();
        if let Some(result) = analysis_tampered.as_mut() {
            result.replay_hash.push('x');
        }
        let mut bridge_tampered = bridge.clone();
        if let Some(result) = bridge_tampered.as_mut() {
            result.replay_hash.push('x');
        }
        let tamper_rejected = !frontend_replay(&frontend_tampered)
            && analysis_tampered
                .as_ref()
                .is_none_or(|result| !analysis_replay(result))
            && bridge_tampered
                .as_ref()
                .is_none_or(|result| !result.replay_verified());
        exact += usize::from(exact_status);
        frontend_replays += usize::from(frontend_ok);
        analysis_replays += usize::from(analysis_ok);
        bridge_replays += usize::from(bridge_ok);
        downstream_artifacts += usize::from(authorized);
        tamper_rejections += usize::from(tamper_rejected);
        receipts.push(Receipt {
            id: format!("complex_language_{index:03}"),
            expected: expected.into(),
            frontend_status: format!("{:?}", frontend.status),
            analysis_status: analysis
                .as_ref()
                .map_or_else(|| "None".into(), |result| format!("{:?}", result.status)),
            bridge_status: bridge.as_ref().map_or_else(
                || "NotApplicable".into(),
                |result| format!("{:?}", result.status),
            ),
            exact: exact_status,
            frontend_replay: frontend_ok,
            analysis_replay: analysis_ok,
            bridge_replay: bridge_ok,
            downstream_artifact: authorized,
            tamper_rejected,
            false_authorization,
            false_denial,
        });
    }

    let false_authorizations = receipts
        .iter()
        .filter(|receipt| receipt.false_authorization)
        .count();
    let false_denials = receipts
        .iter()
        .filter(|receipt| receipt.false_denial)
        .count();
    assert_eq!(exact, 240);
    assert_eq!(frontend_replays, 240);
    assert_eq!(analysis_replays, 240);
    assert_eq!(bridge_replays, 240);
    assert_eq!(downstream_artifacts, 120);
    assert_eq!(tamper_rejections, 240);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage186-complex-analysis-language-frontend-v1",
        manifest_sha256: manifest_sha256.clone(),
        corpus_sha256: digest(&receipts),
        cases: 240,
        supported: 120,
        ambiguous: 40,
        unsupported_or_missing: 80,
        exact_decisions: exact,
        frontend_replay_verified: frontend_replays,
        analysis_replay_verified: analysis_replays,
        bridge_replay_verified: bridge_replays,
        downstream_artifacts,
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
            "# Stage 186 — complex-analysis technical-language frontend\n\nThe bounded frontend extracts only explicit Cauchy–Riemann partials and affine derivative requests.\n\n| Measure | Result |\n|---|---:|\n| Cases | 240 |\n| Supported / ambiguous / unsupported-or-missing | 120 / 40 / 80 |\n| Exact decisions | {exact}/240 |\n| Frontend / analytic / bridge replay | {frontend_replays}/240 / {analysis_replays}/240 / {bridge_replays}/240 |\n| Downstream artifacts | {downstream_artifacts}/120 |\n| Tamper rejection | {tamper_rejections}/240 |\n| False authorizations / denials | {false_authorizations} / {false_denials} |\n| Production mutation | false |\n\nManifest SHA-256: `{manifest_sha256}`\n\nMachine-readable report: `{REPORT_JSON}`\n"
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
