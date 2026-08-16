//! Independent language-boundary campaign for the source-derived metric pack.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::source_metric_pack::{
    evaluate_metric, extract_metric_definitions, source_metric_frontend::formalize_metric_text,
    source_metric_frontend::FrontendStatus, MetricOperation, MetricStatus,
};

const SOURCE: &str =
    include_str!("../../docs/sources/topology_without_tears_finite_metric_definition.txt");
const REPORT_JSON: &str = "docs/phase73_source_metric_frontend.json";
const REPORT_MD: &str = "docs/phase73_source_metric_frontend.md";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    frontend_status: FrontendStatus,
    execution_status: Option<MetricStatus>,
    artifact_emitted: bool,
    exact: bool,
    source_provenance: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_document_sha256: String,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    supported_artifacts: usize,
    source_provenance_preserved: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    operation_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn table() -> String {
    "points: p0,p1,p2; distances: p0-p0=0,p0-p1=1,p0-p2=2,p1-p1=0,p1-p2=1,p2-p2=0".into()
}

fn supported(operation: MetricOperation, index: usize) -> String {
    let suffix = match operation {
        MetricOperation::ValidateMetric => "check the metric axioms",
        MetricOperation::Distance => "determine the distance from p0 to p2",
        MetricOperation::OpenBall => "determine the open ball centered at p0 with radius 2",
        MetricOperation::Diameter => "determine the diameter",
    };
    format!(
        "For a finite metric {}, {}; {}.",
        index % 2 == 0,
        table(),
        suffix
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let records = extract_metric_definitions(SOURCE).expect("source metric definition extracts");
    let mut cases = Vec::new();
    for index in 0..120 {
        let operation = match index % 4 {
            0 => MetricOperation::ValidateMetric,
            1 => MetricOperation::Distance,
            2 => MetricOperation::OpenBall,
            _ => MetricOperation::Diameter,
        };
        cases.push((
            format!("supported_{index}"),
            Expected::Supported,
            supported(operation, index),
        ));
    }
    for index in 0..40 {
        let text = match index % 4 {
            0 => format!("For a finite metric {}; determine a result.", table()),
            1 => format!("For a finite metric {}; determine the distance.", table()),
            2 => format!(
                "For a finite metric {}; determine the open ball centered at p0.",
                table()
            ),
            _ => format!(
                "For either a metric or distance function {}; check the axioms.",
                table()
            ),
        };
        cases.push((format!("ambiguous_{index}"), Expected::Ambiguous, text));
    }
    for index in 0..80 {
        let text = match index % 4 {
            0 => "Prove completeness of an infinite geodesic metric space.".into(),
            1 => "For a finite metric on points p0,p1, determine the diameter.".into(),
            2 => format!(
                "For a finite metric {}; determine the diameter.",
                table().replace("p0-p2=2", "p0-p2")
            ),
            _ => "Compute a Hausdorff dimension from a metric space.".into(),
        };
        cases.push((format!("unsupported_{index}"), Expected::Unsupported, text));
    }
    assert_eq!(cases.len(), 240);
    let mut receipts = Vec::with_capacity(cases.len());
    for (id, expected, text) in cases {
        let frontend = formalize_metric_text(&text);
        let mut tampered_frontend = frontend.clone();
        tampered_frontend.replay_hash.push('x');
        let mut execution_status = None;
        let mut artifact_emitted = false;
        let mut source_provenance = frontend.replay_verified();
        let mut replay_verified = frontend.replay_verified();
        let mut tamper_rejected = !tampered_frontend.replay_verified();
        let authorized = if let Some(request) = frontend.request.clone() {
            let result = evaluate_metric(&request, &records);
            execution_status = Some(result.status);
            artifact_emitted = result.artifact.is_some();
            source_provenance =
                source_provenance && result.source.is_some() && !result.provenance.is_empty();
            replay_verified = replay_verified && result.replay_verified();
            let mut tampered_result = result.clone();
            tampered_result.replay_hash.push('x');
            tamper_rejected = tamper_rejected && !tampered_result.replay_verified();
            result.authorized()
        } else {
            false
        };
        let exact = match expected {
            Expected::Supported => authorized && frontend.status == FrontendStatus::Complete,
            Expected::Ambiguous => !authorized && frontend.status == FrontendStatus::Ambiguous,
            Expected::Unsupported => {
                !authorized
                    && matches!(
                        frontend.status,
                        FrontendStatus::Unsupported | FrontendStatus::Missing
                    )
            }
        };
        receipts.push(Receipt {
            id,
            expected,
            frontend_status: frontend.status,
            execution_status,
            artifact_emitted,
            exact,
            source_provenance,
            replay_verified,
            tamper_rejected,
            false_authorization: expected != Expected::Supported && authorized,
            false_denial: expected == Expected::Supported && !authorized,
        });
    }
    let report = Report {
        schema: "phase73-source-metric-frontend-v1",
        source_document_sha256: digest(&SOURCE),
        corpus_sha256: digest(&receipts),
        cases: receipts.len(),
        supported: receipts
            .iter()
            .filter(|r| r.expected == Expected::Supported)
            .count(),
        ambiguous: receipts
            .iter()
            .filter(|r| r.expected == Expected::Ambiguous)
            .count(),
        unsupported: receipts
            .iter()
            .filter(|r| r.expected == Expected::Unsupported)
            .count(),
        exact_decisions: receipts.iter().filter(|r| r.exact).count(),
        supported_artifacts: receipts
            .iter()
            .filter(|r| r.expected == Expected::Supported && r.artifact_emitted)
            .count(),
        source_provenance_preserved: receipts.iter().filter(|r| r.source_provenance).count(),
        replay_verified: receipts.iter().filter(|r| r.replay_verified).count(),
        tamper_rejected: receipts.iter().filter(|r| r.tamper_rejected).count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts.iter().filter(|r| r.false_denial).count(),
        operation_counts: BTreeMap::new(),
        receipts,
    };
    assert_eq!(report.cases, 240);
    assert_eq!(
        (report.supported, report.ambiguous, report.unsupported),
        (120, 40, 80)
    );
    assert_eq!(report.exact_decisions, 240);
    assert_eq!(report.supported_artifacts, 120);
    assert_eq!(report.source_provenance_preserved, 240);
    assert_eq!(report.replay_verified, 240);
    assert_eq!(report.tamper_rejected, 240);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Phase 73 — Source metric technical-language frontend\n\n- Cases: 240 (120 supported, 40 ambiguous, 80 unsupported)\n- Exact decisions: 240/240\n- Supported artifacts: 120/120\n- Source provenance preserved: 240/240\n- Replay verified: 240/240\n- Tamper rejected: 240/240\n- False authorizations / denials: 0 / 0\n- HLE questions read: 0\n- Production registry mutations: 0\n- Corpus report: `{}`\n",
            REPORT_JSON
        ),
    )?;
    Ok(())
}
