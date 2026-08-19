//! Stage 357: execute a metric request only after exact-version memory retrieval.
//!
//! This is the end-to-end source path: attributed document, generic catalog,
//! cloned exact-version memory, typed retrieval, then bounded execution.
//! Retrieval does not promote the catalog or mutate the live curriculum.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::curriculum_memory::{AppendStatus, CurriculumMemory};
use the_machine::source_metric_pack::{
    evaluate_metric, DistanceEntry, MetricOperation, MetricRequest, MetricStatus,
};
use the_machine::source_multiformat_discovery::{
    discover_source_catalog, SourceCatalogDocument, SourceCatalogKind, SourceCatalogRecords,
};
use the_machine::source_multiformat_memory::{
    append_catalog, replay_verified, retrieve_catalog, CatalogMemoryStatus,
};

const SOURCE: &str = "docs/sources/topology_without_tears_finite_metric_definition.txt";
const REPORT_JSON: &str = "docs/stage357_goal7_metric_memory_execution.json";
const REPORT_MD: &str = "docs/stage357_goal7_metric_memory_execution.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_sha256: String,
    catalog_kind: SourceCatalogKind,
    retrieved_status: CatalogMemoryStatus,
    retrieved_replay_verified: bool,
    execution_cases: usize,
    exact_execution_decisions: usize,
    execution_replay_verified: usize,
    execution_tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    append_status: AppendStatus,
    duplicate_status: AppendStatus,
    production_mutations: usize,
    manifest_unchanged: bool,
    report_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("report serializes"))
    )
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn line_distances(points: &[&str]) -> Vec<DistanceEntry> {
    let mut entries = Vec::new();
    for (left_index, left) in points.iter().enumerate() {
        for (right_index, right) in points.iter().enumerate().skip(left_index) {
            entries.push(DistanceEntry {
                left: (*left).into(),
                right: (*right).into(),
                distance: (right_index - left_index) as i64,
            });
        }
    }
    entries
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source_bytes = fs::read(SOURCE)?;
    let source_text = std::str::from_utf8(&source_bytes)?;
    let catalog = discover_source_catalog(SourceCatalogDocument {
        path: SOURCE,
        document: source_text,
    })
    .map_err(|errors| errors.join("; "))?;
    assert_eq!(catalog.kind, SourceCatalogKind::Metric);

    let manifest_before = breadth_first_manifest().replay_hash();
    let mut memory = CurriculumMemory::new();
    let append_status = append_catalog(&mut memory, &catalog);
    let duplicate_status = append_catalog(&mut memory, &catalog);
    let retrieved = retrieve_catalog(
        &memory,
        SourceCatalogKind::Metric,
        &catalog.candidate.domain,
        &catalog.source_hash,
    );
    assert_eq!(retrieved.status, CatalogMemoryStatus::Unique);
    assert!(replay_verified(&retrieved));
    let SourceCatalogRecords::Metric(records) = retrieved.records.as_ref().unwrap() else {
        unreachable!("exact metric retrieval must retain metric records")
    };
    let record = records.first().expect("metric record");

    let base = MetricRequest {
        operation: MetricOperation::Distance,
        metric: record.metric_id.clone(),
        points: vec!["a".into(), "b".into(), "c".into()],
        distances: line_distances(&["a", "b", "c"]),
        center: Some("a".into()),
        target: Some("c".into()),
        radius: Some(2),
        domain: record.domain.clone(),
        ambiguity: None,
        provenance: vec!["stage357:retrieved-metric-catalog".into()],
    };
    let mut requests = vec![(base.clone(), MetricStatus::Complete)];
    let mut ball = base.clone();
    ball.operation = MetricOperation::OpenBall;
    requests.push((ball, MetricStatus::Complete));
    let mut diameter = base.clone();
    diameter.operation = MetricOperation::Diameter;
    requests.push((diameter, MetricStatus::Complete));
    let mut ambiguous = base.clone();
    ambiguous.ambiguity = Some("metric interpretation is unresolved".into());
    requests.push((ambiguous, MetricStatus::Ambiguous));
    let mut wrong_domain = base.clone();
    wrong_domain.domain = "unbounded_metric".into();
    requests.push((wrong_domain, MetricStatus::InvalidDomain));
    let mut missing_target = base;
    missing_target.target = None;
    requests.push((missing_target, MetricStatus::Missing));

    let mut exact = 0;
    let mut replay = 0;
    let mut tamper = 0;
    let mut false_auth = 0;
    let mut false_denial = 0;
    for (request, expected) in &requests {
        let result = evaluate_metric(request, records);
        exact += usize::from(result.status == *expected);
        replay += usize::from(result.replay_verified());
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        tamper += usize::from(!tampered.replay_verified());
        false_auth += usize::from(*expected != MetricStatus::Complete && result.authorized());
        false_denial += usize::from(*expected == MetricStatus::Complete && !result.authorized());
    }
    let manifest_unchanged = manifest_before == breadth_first_manifest().replay_hash();
    assert_eq!(append_status, AppendStatus::Appended);
    assert_eq!(duplicate_status, AppendStatus::Duplicate);
    assert_eq!(exact, requests.len());
    assert_eq!(replay, requests.len());
    assert_eq!(tamper, requests.len());
    assert_eq!(false_auth, 0);
    assert_eq!(false_denial, 0);
    assert!(manifest_unchanged);

    let mut report = Report {
        schema: "stage357-goal7-metric-memory-execution-v1",
        source_sha256: digest_bytes(&source_bytes),
        catalog_kind: catalog.kind,
        retrieved_status: retrieved.status,
        retrieved_replay_verified: replay_verified(&retrieved),
        execution_cases: requests.len(),
        exact_execution_decisions: exact,
        execution_replay_verified: replay,
        execution_tamper_rejected: tamper,
        false_authorizations: false_auth,
        false_denials: false_denial,
        append_status,
        duplicate_status,
        production_mutations: 0,
        manifest_unchanged,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 357 — execute an exact-version retrieved metric catalog\n\n\
* catalog retrieval / replay: {:?} / {}\n\
* execution cases / exact decisions: {} / {}\n\
* execution replay / tamper rejection: {} / {}\n\
* false authorizations / denials: {} / {}\n\
* append / duplicate refusal: {:?} / {:?}\n\
* production mutations: {}\n\
* manifest unchanged: {}\n\n\
Execution consumed only the typed metric records returned by exact kind-and-version retrieval from cloned memory. Supported requests and ambiguous, invalid-domain, and missing-target requests retained their boundaries; no live curriculum or registry was changed.\n",
            report.retrieved_status,
            report.retrieved_replay_verified,
            report.execution_cases,
            report.exact_execution_decisions,
            report.execution_replay_verified,
            report.execution_tamper_rejected,
            report.false_authorizations,
            report.false_denials,
            report.append_status,
            report.duplicate_status,
            report.production_mutations,
            report.manifest_unchanged,
        ),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
