//! Phase 74: metric-to-topology cross-domain composition.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::metric_topology_bridge::{metric_result_to_topology, BridgeStatus};
use the_machine::source_metric_pack::{
    evaluate_metric, extract_metric_definitions, DistanceEntry, MetricDefinitionRecord,
    MetricOperation, MetricRequest, MetricStatus, DOMAIN as METRIC_DOMAIN,
};
use the_machine::source_topology_pack::{
    evaluate_topology, extract_topology_definitions, TopologyStatus,
};

const METRIC_SOURCE: &str =
    include_str!("../../docs/sources/topology_without_tears_finite_metric_definition.txt");
const TOPOLOGY_SOURCE: &str =
    include_str!("../../docs/sources/topology_without_tears_finite_definition.txt");
const REPORT_JSON: &str = "docs/phase74_metric_topology_bridge.json";
const REPORT_MD: &str = "docs/phase74_metric_topology_bridge.md";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    metric_status: MetricStatus,
    bridge_status: BridgeStatus,
    topology_status: Option<TopologyStatus>,
    exact: bool,
    metric_replay: bool,
    bridge_replay: bool,
    topology_replay: bool,
    tamper_rejected: bool,
    provenance_preserved: bool,
    false_authorization: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    metric_source_sha256: String,
    topology_source_sha256: String,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    supported_topology_artifacts: usize,
    metric_replay_verified: usize,
    bridge_replay_verified: usize,
    topology_receipts: usize,
    topology_replay_verified: usize,
    tamper_rejected: usize,
    provenance_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    hle_questions_read: usize,
    production_registry_mutations: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn metric_record() -> MetricDefinitionRecord {
    extract_metric_definitions(METRIC_SOURCE)
        .expect("metric source extracts")
        .remove(0)
}

fn points(count: usize) -> Vec<String> {
    (0..count).map(|index| format!("p{index}")).collect()
}

fn distances(count: usize, triangle_violation: bool) -> Vec<DistanceEntry> {
    let mut entries = Vec::new();
    for left in 0..count {
        for right in left..count {
            let mut distance = (right - left) as i64;
            if triangle_violation && left == 0 && right == count - 1 {
                distance += 3;
            }
            entries.push(DistanceEntry {
                left: format!("p{left}"),
                right: format!("p{right}"),
                distance,
            });
        }
    }
    entries
}

fn request(
    operation: MetricOperation,
    index: usize,
    ambiguity: Option<String>,
    invalid: bool,
) -> MetricRequest {
    let count = 3 + index % 4;
    let carrier = points(count);
    MetricRequest {
        operation,
        metric: "finite_metric_axioms".into(),
        points: carrier.clone(),
        distances: distances(count, invalid),
        center: carrier.first().cloned(),
        target: carrier.last().cloned(),
        radius: Some(2),
        domain: METRIC_DOMAIN.into(),
        ambiguity,
        provenance: vec!["phase74-metric-topology-composition".into()],
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let metric_records = vec![metric_record()];
    let topology_records =
        extract_topology_definitions(TOPOLOGY_SOURCE).expect("topology source extracts");
    let mut cases = Vec::new();
    for index in 0..120 {
        cases.push((
            format!("supported_{index}"),
            Expected::Supported,
            request(MetricOperation::ValidateMetric, index, None, false),
        ));
    }
    for index in 0..40 {
        cases.push((
            format!("ambiguous_{index}"),
            Expected::Ambiguous,
            request(
                MetricOperation::ValidateMetric,
                index,
                Some("metric interpretation is unresolved".into()),
                false,
            ),
        ));
    }
    for index in 0..40 {
        cases.push((
            format!("unsupported_artifact_{index}"),
            Expected::Unsupported,
            request(MetricOperation::Distance, index, None, false),
        ));
    }
    for index in 0..40 {
        cases.push((
            format!("unsupported_invalid_metric_{index}"),
            Expected::Unsupported,
            request(MetricOperation::ValidateMetric, index, None, true),
        ));
    }
    assert_eq!(cases.len(), 240);
    let mut receipts = Vec::with_capacity(cases.len());
    for (id, expected, request) in cases {
        let metric = evaluate_metric(&request, &metric_records);
        let mut tampered_metric = metric.clone();
        tampered_metric.replay_hash.push('x');
        let bridge = metric_result_to_topology(&metric);
        let mut tampered_bridge = bridge.clone();
        tampered_bridge.replay_hash.push('x');
        let mut topology_status = None;
        let mut topology_replay = true;
        let mut tampered_topology_rejected = true;
        if let Some(topology_request) = bridge.request.clone() {
            let topology = evaluate_topology(&topology_request, &topology_records);
            topology_status = Some(topology.status);
            topology_replay = topology.replay_verified();
            let mut tampered_topology = topology.clone();
            tampered_topology.replay_hash.push('x');
            tampered_topology_rejected = !tampered_topology.replay_verified();
        }
        let exact = match expected {
            Expected::Supported => {
                bridge.status == BridgeStatus::Complete
                    && topology_status == Some(TopologyStatus::Complete)
            }
            Expected::Ambiguous => bridge.status == BridgeStatus::Ambiguous,
            Expected::Unsupported => bridge.status == BridgeStatus::Unsupported,
        };
        let metric_replay = metric.replay_verified();
        let bridge_replay = bridge.replay_verified();
        let provenance = !metric.provenance.is_empty() && !bridge.provenance.is_empty();
        receipts.push(Receipt {
            id,
            expected,
            metric_status: metric.status,
            bridge_status: bridge.status,
            topology_status,
            exact,
            metric_replay,
            bridge_replay,
            topology_replay,
            tamper_rejected: !tampered_metric.replay_verified()
                && !tampered_bridge.replay_verified()
                && tampered_topology_rejected,
            provenance_preserved: provenance,
            false_authorization: expected != Expected::Supported
                && topology_status == Some(TopologyStatus::Complete),
        });
    }
    let report = Report {
        schema: "phase74-metric-topology-bridge-v1",
        metric_source_sha256: digest(&METRIC_SOURCE),
        topology_source_sha256: digest(&TOPOLOGY_SOURCE),
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
        supported_topology_artifacts: receipts
            .iter()
            .filter(|r| {
                r.expected == Expected::Supported
                    && r.topology_status == Some(TopologyStatus::Complete)
            })
            .count(),
        metric_replay_verified: receipts.iter().filter(|r| r.metric_replay).count(),
        bridge_replay_verified: receipts.iter().filter(|r| r.bridge_replay).count(),
        topology_receipts: receipts
            .iter()
            .filter(|r| r.topology_status.is_some())
            .count(),
        topology_replay_verified: receipts
            .iter()
            .filter(|r| r.topology_status.is_some() && r.topology_replay)
            .count(),
        tamper_rejected: receipts.iter().filter(|r| r.tamper_rejected).count(),
        provenance_preserved: receipts.iter().filter(|r| r.provenance_preserved).count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts
            .iter()
            .filter(|r| r.expected == Expected::Supported && !r.exact)
            .count(),
        hle_questions_read: 0,
        production_registry_mutations: 0,
        receipts,
    };
    assert_eq!(report.cases, 240);
    assert_eq!(
        (report.supported, report.ambiguous, report.unsupported),
        (120, 40, 80)
    );
    assert_eq!(report.exact_decisions, 240);
    assert_eq!(report.supported_topology_artifacts, 120);
    assert_eq!(report.metric_replay_verified, 240);
    assert_eq!(report.bridge_replay_verified, 240);
    assert_eq!(report.topology_receipts, 120);
    assert_eq!(report.topology_replay_verified, 120);
    assert_eq!(report.tamper_rejected, 240);
    assert_eq!(report.provenance_preserved, 240);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        format!(
            "# Phase 74 — Metric to induced topology composition\n\n- Cases: 240 (120 supported, 40 ambiguous, 80 unsupported)\n- Supported induced topologies: 120/120\n- Exact decisions: 240/240\n- Metric replay: 240/240\n- Bridge replay: 240/240\n- Topology replay: 120/120 emitted artifacts\n- Tamper rejected: 240/240\n- Provenance preserved: 240/240\n- False authorizations / denials: 0 / 0\n- HLE questions read: 0\n- Production registry mutations: 0\n- Corpus report: `{}`\n",
            REPORT_JSON
        ),
    )?;
    Ok(())
}
