//! Stage 355: independent pressure validation for the source-derived metric.
//!
//! The metric source is now discovered generically, but execution is tested
//! separately with independently authored finite distance tables and explicit
//! malformed/ambiguous boundaries.  No production route or curriculum status
//! is changed.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::source_metric_pack::{
    evaluate_metric, extract_metric_definitions, DistanceEntry, MetricOperation, MetricRequest,
    MetricStatus,
};
use the_machine::source_multiformat_discovery::{
    discover_source_catalog, SourceCatalogDocument, SourceCatalogKind, SourceCatalogRecords,
};

const SOURCE: &str = "docs/sources/topology_without_tears_finite_metric_definition.txt";
const REPORT_JSON: &str = "docs/stage355_goal7_metric_pressure.json";
const REPORT_MD: &str = "docs/stage355_goal7_metric_pressure.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_sha256: String,
    corpus_sha256: String,
    cases: usize,
    complete_cases: usize,
    ambiguous_cases: usize,
    missing_cases: usize,
    unsupported_or_inconsistent_cases: usize,
    exact_decisions: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    production_mutations: usize,
    manifest_unchanged: bool,
    report_sha256: String,
}

#[derive(Debug, Serialize)]
struct CaseReceipt {
    id: String,
    expected: MetricStatus,
    actual: MetricStatus,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn points(size: usize) -> Vec<String> {
    (0..size).map(|index| format!("p{index}")).collect()
}

fn line_distances(points: &[String], scale: i64) -> Vec<DistanceEntry> {
    let mut entries = Vec::new();
    for (left_index, left) in points.iter().enumerate() {
        for (right_index, right) in points.iter().enumerate().skip(left_index) {
            entries.push(DistanceEntry {
                left: left.clone(),
                right: right.clone(),
                distance: (right_index as i64 - left_index as i64) * scale,
            });
        }
    }
    entries
}

fn request(
    record: &the_machine::source_metric_pack::MetricDefinitionRecord,
    operation: MetricOperation,
    points: Vec<String>,
    distances: Vec<DistanceEntry>,
) -> MetricRequest {
    MetricRequest {
        operation,
        metric: record.metric_id.clone(),
        points,
        distances,
        center: Some("p0".into()),
        target: Some("p1".into()),
        radius: Some(2),
        domain: record.domain.clone(),
        ambiguity: None,
        provenance: vec!["stage355:independent-metric-corpus".into()],
    }
}

fn cases(
    record: &the_machine::source_metric_pack::MetricDefinitionRecord,
) -> Vec<(String, MetricRequest, MetricStatus)> {
    let mut cases = Vec::new();
    for size in 2..=4 {
        let carrier = points(size);
        let distances = line_distances(&carrier, 1);
        for operation in [
            MetricOperation::ValidateMetric,
            MetricOperation::Distance,
            MetricOperation::OpenBall,
            MetricOperation::Diameter,
        ] {
            cases.push((
                format!("valid_{size}_{operation:?}"),
                request(record, operation, carrier.clone(), distances.clone()),
                MetricStatus::Complete,
            ));
        }
    }
    let valid_points = points(3);
    let valid_distances = line_distances(&valid_points, 2);
    let mut missing = request(
        record,
        MetricOperation::Distance,
        valid_points.clone(),
        valid_distances.clone(),
    );
    missing.center = None;
    missing.target = None;
    cases.push((
        "missing_distance_target".into(),
        missing,
        MetricStatus::Missing,
    ));
    let mut ambiguous = request(
        record,
        MetricOperation::ValidateMetric,
        valid_points.clone(),
        valid_distances.clone(),
    );
    ambiguous.ambiguity = Some("metric identity is unresolved".into());
    cases.push((
        "ambiguous_identity".into(),
        ambiguous,
        MetricStatus::Ambiguous,
    ));
    let mut unknown = request(
        record,
        MetricOperation::ValidateMetric,
        valid_points.clone(),
        valid_distances.clone(),
    );
    unknown.metric = "unknown_metric".into();
    cases.push(("missing_identity".into(), unknown, MetricStatus::Missing));
    let mut invalid_domain = request(
        record,
        MetricOperation::ValidateMetric,
        valid_points.clone(),
        valid_distances.clone(),
    );
    invalid_domain.domain = "unbounded_metric".into();
    cases.push((
        "invalid_domain".into(),
        invalid_domain,
        MetricStatus::InvalidDomain,
    ));
    let mut negative = request(
        record,
        MetricOperation::ValidateMetric,
        valid_points.clone(),
        valid_distances.clone(),
    );
    negative.distances[1].distance = -1;
    cases.push((
        "negative_distance".into(),
        negative,
        MetricStatus::Inconsistent,
    ));
    let mut identity = request(
        record,
        MetricOperation::ValidateMetric,
        valid_points.clone(),
        valid_distances.clone(),
    );
    identity.distances[0].distance = 1;
    cases.push((
        "identity_violation".into(),
        identity,
        MetricStatus::Inconsistent,
    ));
    let mut duplicate = request(
        record,
        MetricOperation::ValidateMetric,
        valid_points.clone(),
        valid_distances.clone(),
    );
    duplicate.points.push("p0".into());
    cases.push((
        "duplicate_point".into(),
        duplicate,
        MetricStatus::Inconsistent,
    ));
    let mut too_large = request(
        record,
        MetricOperation::ValidateMetric,
        points(9),
        Vec::new(),
    );
    too_large.distances = line_distances(&too_large.points, 1);
    cases.push(("over_bound".into(), too_large, MetricStatus::Inconsistent));
    let mut outside = request(
        record,
        MetricOperation::Distance,
        valid_points,
        valid_distances,
    );
    outside.target = Some("outside".into());
    cases.push((
        "target_outside_carrier".into(),
        outside,
        MetricStatus::Inconsistent,
    ));
    cases
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_bytes = fs::read(SOURCE)?;
    let source_text = std::str::from_utf8(&source_bytes)?;
    let discovered = discover_source_catalog(SourceCatalogDocument {
        path: SOURCE,
        document: source_text,
    })
    .map_err(|errors| errors.join("; "))?;
    assert_eq!(discovered.kind, SourceCatalogKind::Metric);
    let SourceCatalogRecords::Metric(records) = discovered.records else {
        unreachable!("metric source must produce metric records")
    };
    let metric_records =
        extract_metric_definitions(source_text).map_err(|errors| errors.join("; "))?;
    assert_eq!(records, metric_records);
    let record = records.first().expect("source metric record");
    let manifest_before = breadth_first_manifest().replay_hash();
    let mut receipts = Vec::new();
    for (id, request, expected) in cases(record) {
        let result = evaluate_metric(&request, &records);
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        receipts.push(CaseReceipt {
            id,
            expected,
            actual: result.status,
            exact: result.status == expected,
            replay_verified: result.replay_verified(),
            tamper_rejected: !tampered.replay_verified(),
        });
    }
    let manifest_unchanged = manifest_before == breadth_first_manifest().replay_hash();
    let cases_count = receipts.len();
    let exact_decisions = receipts.iter().filter(|receipt| receipt.exact).count();
    let replay_verified = receipts
        .iter()
        .filter(|receipt| receipt.replay_verified)
        .count();
    let tamper_rejected = receipts
        .iter()
        .filter(|receipt| receipt.tamper_rejected)
        .count();
    let false_authorizations = receipts
        .iter()
        .filter(|receipt| {
            receipt.expected != MetricStatus::Complete && receipt.actual == MetricStatus::Complete
        })
        .count();
    let false_denials = receipts
        .iter()
        .filter(|receipt| {
            receipt.expected == MetricStatus::Complete && receipt.actual != MetricStatus::Complete
        })
        .count();
    assert_eq!(cases_count, exact_decisions);
    assert_eq!(cases_count, replay_verified);
    assert_eq!(cases_count, tamper_rejected);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    assert!(manifest_unchanged);
    let mut report = Report {
        schema: "stage355-goal7-metric-pressure-v1",
        source_sha256: digest_bytes(&source_bytes),
        corpus_sha256: digest(&receipts),
        cases: cases_count,
        complete_cases: receipts
            .iter()
            .filter(|receipt| receipt.actual == MetricStatus::Complete)
            .count(),
        ambiguous_cases: receipts
            .iter()
            .filter(|receipt| receipt.actual == MetricStatus::Ambiguous)
            .count(),
        missing_cases: receipts
            .iter()
            .filter(|receipt| receipt.actual == MetricStatus::Missing)
            .count(),
        unsupported_or_inconsistent_cases: receipts
            .iter()
            .filter(|receipt| {
                matches!(
                    receipt.actual,
                    MetricStatus::Unsupported
                        | MetricStatus::Inconsistent
                        | MetricStatus::InvalidDomain
                )
            })
            .count(),
        exact_decisions,
        replay_verified,
        tamper_rejected,
        false_authorizations,
        false_denials,
        production_mutations: 0,
        manifest_unchanged,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{json}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 355 — source-derived metric pressure\n\n\
* cases: {}\n\
* complete / ambiguous / missing / unsupported-or-inconsistent: {} / {} / {} / {}\n\
* exact decisions / replay / tamper rejection: {} / {} / {}\n\
* false authorizations / denials: {} / {}\n\
* production mutations: {}\n\
* manifest unchanged: {}\n\n\
The metric executor consumed an independently authored finite-distance corpus after generic source discovery. Valid finite metrics, malformed axioms, ambiguous identities, missing targets, invalid domains, and bound violations were separated without weakening authorization.\n",
            report.cases,
            report.complete_cases,
            report.ambiguous_cases,
            report.missing_cases,
            report.unsupported_or_inconsistent_cases,
            report.exact_decisions,
            report.replay_verified,
            report.tamper_rejected,
            report.false_authorizations,
            report.false_denials,
            report.production_mutations,
            report.manifest_unchanged,
        ),
    )?;
    println!("{json}");
    Ok(())
}
