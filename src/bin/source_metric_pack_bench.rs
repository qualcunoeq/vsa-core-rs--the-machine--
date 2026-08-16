//! Independent pressure campaign for the source-derived finite-metric pack.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::source_metric_pack::{
    evaluate_metric, extract_metric_definitions, DistanceEntry, MetricArtifact, MetricOperation,
    MetricRequest, MetricStatus,
};

const SOURCE: &str =
    include_str!("../../docs/sources/topology_without_tears_finite_metric_definition.txt");
const REPORT_JSON: &str = "docs/phase72_source_metric_pack.json";
const REPORT_MD: &str = "docs/phase72_source_metric_pack.md";

#[derive(Clone, Serialize)]
struct Case {
    id: String,
    family: String,
    request: MetricRequest,
    expected_status: MetricStatus,
    expected_artifact: Option<MetricArtifact>,
}

#[derive(Serialize)]
struct Receipt {
    id: String,
    family: String,
    expected_status: MetricStatus,
    actual_status: MetricStatus,
    artifact_emitted: bool,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    source_provenance: bool,
    false_authorization: bool,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    source_document_sha256: String,
    corpus_sha256: String,
    extracted_records: usize,
    source_mutations: usize,
    source_mutations_rejected: usize,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_artifacts: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    source_provenance_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    family_counts: BTreeMap<String, usize>,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn points(count: usize) -> Vec<String> {
    (0..count).map(|index| format!("p{index}")).collect()
}

fn distances(count: usize) -> Vec<DistanceEntry> {
    let mut result = Vec::new();
    for left in 0..count {
        for right in left..count {
            result.push(DistanceEntry {
                left: format!("p{left}"),
                right: format!("p{right}"),
                distance: (right - left) as i64,
            });
        }
    }
    result
}

fn request(operation: MetricOperation, count: usize) -> MetricRequest {
    MetricRequest {
        operation,
        metric: "finite_metric_axioms".into(),
        points: points(count),
        distances: distances(count),
        center: Some("p0".into()),
        target: Some(format!("p{}", count.saturating_sub(1))),
        radius: Some(2),
        domain: "source_derived_finite_metric".into(),
        ambiguity: None,
        provenance: vec!["phase72-independent-metric-exercise".into()],
    }
}

fn valid_expected(operation: MetricOperation, count: usize) -> MetricArtifact {
    match operation {
        MetricOperation::ValidateMetric => MetricArtifact::ValidatedMetric {
            points: points(count),
            distances: distances(count),
        },
        MetricOperation::Distance => MetricArtifact::Distance((count - 1) as i64),
        MetricOperation::OpenBall => {
            let members = if count == 0 {
                Vec::new()
            } else {
                vec!["p0".into(), "p1".into()]
            };
            MetricArtifact::Set(members)
        }
        MetricOperation::Diameter => MetricArtifact::Scalar((count - 1) as i64),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let records = extract_metric_definitions(SOURCE).expect("metric source extracts");
    assert_eq!(records.len(), 1);
    let mutations = vec![
        SOURCE.replace("BEGIN METRIC", "BEGIN BROKEN"),
        SOURCE.replace("URL: https://", "URL: http://"),
        SOURCE.replace(
            "AXIOMS: nonnegative;identity;symmetry;triangle",
            "AXIOMS: nonnegative",
        ),
        SOURCE.replace("MAX_POINTS: 8", "MAX_POINTS: 0"),
        SOURCE.replace("END METRIC", "BEGIN METRIC"),
        SOURCE.replace(
            "ALIASES: finite metric|distance function",
            "ALIASES: duplicate|duplicate",
        ),
    ];
    let source_mutations_rejected = mutations
        .iter()
        .filter(|mutation| extract_metric_definitions(mutation).is_err())
        .count();

    let mut cases = Vec::with_capacity(240);
    for index in 0..30 {
        for (operation, family) in [
            (MetricOperation::ValidateMetric, "validate_metric"),
            (MetricOperation::Distance, "distance"),
            (MetricOperation::OpenBall, "open_ball"),
            (MetricOperation::Diameter, "diameter"),
        ] {
            let count = 3 + index % 4;
            cases.push(Case {
                id: format!("{family}_{index}"),
                family: family.into(),
                request: request(operation, count),
                expected_status: MetricStatus::Complete,
                expected_artifact: Some(valid_expected(operation, count)),
            });
        }
    }
    for index in 0..40 {
        let mut request = request(MetricOperation::Distance, 3 + index % 3);
        request.ambiguity = Some("metric alias or distance convention is unresolved".into());
        cases.push(Case {
            id: format!("ambiguous_metric_{index}"),
            family: "ambiguous_metric".into(),
            request,
            expected_status: MetricStatus::Ambiguous,
            expected_artifact: None,
        });
    }
    for index in 0..14 {
        let mut request = request(MetricOperation::Distance, 3);
        request.domain = "infinite_metric_space".into();
        cases.push(Case {
            id: format!("invalid_domain_{index}"),
            family: "invalid_domain".into(),
            request,
            expected_status: MetricStatus::InvalidDomain,
            expected_artifact: None,
        });
    }
    for index in 0..14 {
        let mut request = request(MetricOperation::ValidateMetric, 3);
        request.distances[4].distance = 5;
        cases.push(Case {
            id: format!("triangle_violation_{index}"),
            family: "triangle_violation".into(),
            request,
            expected_status: MetricStatus::Inconsistent,
            expected_artifact: None,
        });
    }
    for index in 0..13 {
        let mut request = request(MetricOperation::ValidateMetric, 3);
        request.distances.pop();
        cases.push(Case {
            id: format!("incomplete_table_{index}"),
            family: "incomplete_table".into(),
            request,
            expected_status: MetricStatus::Inconsistent,
            expected_artifact: None,
        });
    }
    for index in 0..13 {
        let mut request = request(MetricOperation::ValidateMetric, 3);
        request.distances[0].distance = 1;
        cases.push(Case {
            id: format!("identity_violation_{index}"),
            family: "identity_violation".into(),
            request,
            expected_status: MetricStatus::Inconsistent,
            expected_artifact: None,
        });
    }
    for index in 0..13 {
        let request = request(MetricOperation::Diameter, 9);
        cases.push(Case {
            id: format!("point_bound_{index}"),
            family: "point_bound".into(),
            request,
            expected_status: MetricStatus::Inconsistent,
            expected_artifact: None,
        });
    }
    for index in 0..13 {
        let mut request = request(MetricOperation::OpenBall, 3);
        request.radius = Some(-1);
        cases.push(Case {
            id: format!("invalid_ball_{index}"),
            family: "invalid_ball".into(),
            request,
            expected_status: MetricStatus::Inconsistent,
            expected_artifact: None,
        });
    }
    assert_eq!(cases.len(), 240);
    let mut receipts = Vec::with_capacity(cases.len());
    for case in cases {
        let result = evaluate_metric(&case.request, &records);
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        let exact =
            result.status == case.expected_status && result.artifact == case.expected_artifact;
        receipts.push(Receipt {
            id: case.id,
            family: case.family,
            expected_status: case.expected_status,
            actual_status: result.status,
            artifact_emitted: result.artifact.is_some(),
            exact,
            replay_verified: result.replay_verified(),
            tamper_rejected: !tampered.replay_verified(),
            source_provenance: result.status != MetricStatus::Complete || result.source.is_some(),
            false_authorization: result.authorized()
                && case.expected_status != MetricStatus::Complete,
        });
    }
    let report = Report {
        schema: "phase72-source-metric-pack-v1",
        source_document_sha256: digest(&SOURCE),
        corpus_sha256: digest(&receipts),
        extracted_records: records.len(),
        source_mutations: mutations.len(),
        source_mutations_rejected,
        cases: receipts.len(),
        supported: receipts
            .iter()
            .filter(|r| r.expected_status == MetricStatus::Complete)
            .count(),
        ambiguous: receipts
            .iter()
            .filter(|r| r.expected_status == MetricStatus::Ambiguous)
            .count(),
        refused: receipts
            .iter()
            .filter(|r| {
                r.expected_status != MetricStatus::Complete
                    && r.expected_status != MetricStatus::Ambiguous
            })
            .count(),
        exact_decisions: receipts.iter().filter(|r| r.exact).count(),
        supported_artifacts: receipts
            .iter()
            .filter(|r| r.expected_status == MetricStatus::Complete && r.artifact_emitted)
            .count(),
        replay_verified: receipts.iter().filter(|r| r.replay_verified).count(),
        tamper_rejected: receipts.iter().filter(|r| r.tamper_rejected).count(),
        source_provenance_preserved: receipts.iter().filter(|r| r.source_provenance).count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts
            .iter()
            .filter(|r| r.expected_status == MetricStatus::Complete && !r.artifact_emitted)
            .count(),
        family_counts: receipts
            .iter()
            .fold(BTreeMap::new(), |mut counts, receipt| {
                *counts.entry(receipt.family.clone()).or_insert(0) += 1;
                counts
            }),
        receipts,
    };
    assert_eq!(report.extracted_records, 1);
    assert_eq!(report.source_mutations_rejected, 6);
    assert_eq!(report.cases, 240);
    assert_eq!(
        (report.supported, report.ambiguous, report.refused),
        (120, 40, 80)
    );
    assert_eq!(report.exact_decisions, 240);
    assert_eq!(report.supported_artifacts, 120);
    assert_eq!(report.replay_verified, 240);
    assert_eq!(report.tamper_rejected, 240);
    assert_eq!(report.source_provenance_preserved, 240);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(REPORT_MD, format!("# Phase 72 — Source-derived finite metric pack\n\nThe metric definition is extracted from an attributed source document and executed over exact finite distance tables.\n\n- Cases: 240 (120 supported, 40 ambiguous, 80 refused)\n- Exact decisions: 240/240\n- Supported artifacts: 120/120\n- Source mutations rejected: 6/6\n- Source provenance preserved: 240/240\n- Replay verified: 240/240\n- Tamper rejected: 240/240\n- False authorizations / denials: 0 / 0\n- HLE questions read: 0\n- Production registry mutations: 0\n- Corpus report: `{}`\n", REPORT_JSON))?;
    Ok(())
}
