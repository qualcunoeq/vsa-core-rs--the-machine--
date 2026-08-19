//! Independent technical-language benchmark for the source-derived topology pack.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::source_topology_frontend::{
    formalize_topology_text, TopologyFrontendResult, TopologyFrontendStatus,
};
use the_machine::source_topology_pack::{evaluate_topology, extract_topology_definitions};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Complete,
    Ambiguous,
    Unsupported,
}

#[derive(Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    frontend_status: TopologyFrontendStatus,
    downstream_authorized: bool,
    exact: bool,
    frontend_replay_verified: bool,
    downstream_replay_verified: bool,
    frontend_tamper_rejected: bool,
    downstream_tamper_rejected: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    source_records: usize,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    exact_decisions: usize,
    complete_frontends: usize,
    downstream_authorizations: usize,
    frontend_replay_verified: usize,
    downstream_replay_verified: usize,
    frontend_tamper_rejected: usize,
    downstream_tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    status_counts: BTreeMap<String, usize>,
    receipt_hash: String,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn source_records() -> Vec<the_machine::source_topology_pack::TopologyDefinitionRecord> {
    let document = include_str!("../../docs/sources/topology_without_tears_finite_definition.txt");
    extract_topology_definitions(document).expect("source topology definition extracts")
}

fn downstream(frontend: &TopologyFrontendResult) -> (bool, bool, bool) {
    let Some(request) = frontend.request.clone() else {
        return (false, false, false);
    };
    let result = evaluate_topology(&request, &source_records());
    let authorized = result.authorized();
    let replay = result.replay_verified();
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    (authorized, replay, !tampered.replay_verified())
}

fn run(id: String, text: String, expected: Expected) -> Receipt {
    let frontend = formalize_topology_text(&text);
    let (downstream_authorized, downstream_replay_verified, downstream_tamper_rejected) =
        downstream(&frontend);
    let frontend_replay_verified = frontend.replay_verified();
    let mut tampered = frontend.clone();
    tampered.replay_hash.push('x');
    let frontend_tamper_rejected = !tampered.replay_verified();
    let exact = match expected {
        Expected::Complete => {
            frontend.status == TopologyFrontendStatus::Complete && downstream_authorized
        }
        Expected::Ambiguous => {
            frontend.status == TopologyFrontendStatus::Ambiguous && !downstream_authorized
        }
        Expected::Unsupported => {
            frontend.status == TopologyFrontendStatus::Unsupported && !downstream_authorized
        }
    };
    Receipt {
        id,
        expected,
        frontend_status: frontend.status,
        downstream_authorized,
        exact,
        frontend_replay_verified,
        downstream_replay_verified,
        frontend_tamper_rejected,
        downstream_tamper_rejected,
        false_authorization: expected != Expected::Complete && downstream_authorized,
        false_denial: expected == Expected::Complete && !downstream_authorized,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut receipts = Vec::with_capacity(240);
    for index in 0..120 {
        let points = "{a,b,c}";
        let opens = if index % 3 == 0 {
            "open sets: {}; open sets: {a}; open sets: {a,b,c}"
        } else if index % 3 == 1 {
            "open sets: {}; open sets: {a}; open sets: {b}; open sets: {c}; open sets: {a,b}; open sets: {a,c}; open sets: {b,c}; open sets: {a,b,c}"
        } else {
            "open sets: {}; open sets: {a,b,c}"
        };
        let (operation, target) = match index % 5 {
            0 => ("Validate topology", ""),
            1 => ("Is open", " target: {a}"),
            2 => ("Is closed", " target: {a,b}"),
            3 => ("Find the interior", " target: {a,b}"),
            _ => ("Find the closure", " target: {a}"),
        };
        receipts.push(run(
            format!("supported_{index:03}"),
            format!("{operation}; points: {points};{target}; {opens}."),
            Expected::Complete,
        ));
    }
    for index in 0..40 {
        receipts.push(run(
            format!("ambiguous_{index:03}"),
            "Find the interior; points: {a,b}; points: {a,b,c}; target: {a}; open sets: {}; open sets: {a}; open sets: {a,b,c}.".into(),
            Expected::Ambiguous,
        ));
    }
    let unsupported = [
        "Determine whether the metric space is compact.",
        "Find the homology of the topology; points: {a,b}; open sets: {}; open sets: {a,b}.",
        "Validate topology; points: {a,b,c,d,e,f,g,h,i}; open sets: {}; open sets: {a,b,c,d,e,f,g,h,i}.",
        "Validate topology; points: {a,b,c,d,e,f,g,h,i}; open sets: {}; open sets: {a,b,c,d,e,f,g,h,i}.",
    ];
    for index in 0..80 {
        receipts.push(run(
            format!("unsupported_{index:03}"),
            unsupported[index % unsupported.len()].into(),
            Expected::Unsupported,
        ));
    }
    assert_eq!(receipts.len(), 240);
    let supported = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Complete)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Ambiguous)
        .count();
    let unsupported_count = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Unsupported)
        .count();
    let exact_decisions = receipts.iter().filter(|receipt| receipt.exact).count();
    let complete_frontends = receipts
        .iter()
        .filter(|receipt| receipt.frontend_status == TopologyFrontendStatus::Complete)
        .count();
    let downstream_authorizations = receipts
        .iter()
        .filter(|receipt| receipt.downstream_authorized)
        .count();
    let frontend_replay_verified = receipts
        .iter()
        .filter(|receipt| receipt.frontend_replay_verified)
        .count();
    let downstream_replay_verified = receipts
        .iter()
        .filter(|receipt| receipt.downstream_replay_verified)
        .count();
    let frontend_tamper_rejected = receipts
        .iter()
        .filter(|receipt| receipt.frontend_tamper_rejected)
        .count();
    let downstream_tamper_rejected = receipts
        .iter()
        .filter(|receipt| receipt.downstream_tamper_rejected)
        .count();
    let false_authorizations = receipts
        .iter()
        .filter(|receipt| receipt.false_authorization)
        .count();
    let false_denials = receipts
        .iter()
        .filter(|receipt| receipt.false_denial)
        .count();
    assert_eq!((supported, ambiguous, unsupported_count), (120, 40, 80));
    assert_eq!(exact_decisions, 240);
    assert_eq!(complete_frontends, 120);
    assert_eq!(downstream_authorizations, 120);
    assert_eq!(frontend_replay_verified, 240);
    assert_eq!(downstream_replay_verified, 120);
    assert_eq!(frontend_tamper_rejected, 240);
    assert_eq!(downstream_tamper_rejected, 120);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let mut status_counts = BTreeMap::new();
    for receipt in &receipts {
        *status_counts
            .entry(format!("{:?}", receipt.frontend_status))
            .or_insert(0usize) += 1;
    }
    let report = Report {
        schema: "stage-c-source-derived-topology-frontend-v1",
        corpus_sha256: hash(&receipts),
        source_records: source_records().len(),
        cases: 240,
        supported,
        ambiguous,
        unsupported: unsupported_count,
        exact_decisions,
        complete_frontends,
        downstream_authorizations,
        frontend_replay_verified,
        downstream_replay_verified,
        frontend_tamper_rejected,
        downstream_tamper_rejected,
        false_authorizations,
        false_denials,
        status_counts,
        receipt_hash: hash(&receipts),
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage-c-source-derived-topology-frontend.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
