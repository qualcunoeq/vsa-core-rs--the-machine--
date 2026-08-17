//! Stage 125: finite simplicial homology to one-skeleton graph composition.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::simplicial_homology_bridge::{one_skeleton_graph, BridgeStatus};
use the_machine::simplicial_homology_pack::{
    HomologyOperation, HomologyStatus, SimplicialComplexRequest,
};

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    expected: BridgeStatus,
    request: SimplicialComplexRequest,
    policy: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    authorized_routes: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    statuses: BTreeMap<String, usize>,
}

fn digest<T: serde::Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn request(valid: bool, coefficient_field: Option<u32>) -> SimplicialComplexRequest {
    let mut simplices = vec![
        vec![0],
        vec![1],
        vec![2],
        vec![0, 1],
        vec![0, 2],
        vec![1, 2],
    ];
    if valid {
        simplices.push(vec![0, 1, 2]);
    } else {
        // Retain a 2-simplex while removing a required face.
        simplices.push(vec![0, 1, 2]);
        simplices.retain(|simplex| simplex != &vec![0, 1]);
    }
    SimplicialComplexRequest {
        operation: HomologyOperation::ValidateComplex,
        domain: "finite_simplicial_complex".into(),
        vertices: vec!["a".into(), "b".into(), "c".into()],
        simplices,
        coefficient_field,
        provenance: vec!["stage125-independent-composition-corpus".into()],
        ambiguity: None,
    }
}

fn corpus() -> Vec<Case> {
    let mut cases = Vec::new();
    for index in 0..120 {
        cases.push(Case {
            id: format!("supported-{index:03}"),
            expected: BridgeStatus::Complete,
            request: request(true, Some(2)),
            policy: "one_skeleton_graph".into(),
        });
    }
    for index in 0..40 {
        cases.push(Case {
            id: format!("ambiguous-{index:03}"),
            expected: BridgeStatus::Ambiguous,
            request: request(true, Some(2)),
            policy: "infer_graph".into(),
        });
    }
    for index in 0..80 {
        let (request, expected) = if index % 2 == 0 {
            (request(false, Some(2)), BridgeStatus::Invalid)
        } else {
            (request(true, Some(3)), BridgeStatus::Unsupported)
        };
        cases.push(Case {
            id: format!("refused-{index:03}"),
            expected,
            request,
            policy: "one_skeleton_graph".into(),
        });
    }
    cases
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cases = corpus();
    let mut exact_decisions = 0;
    let mut authorized_routes = 0;
    let mut replay_verified = 0;
    let mut tamper_rejected = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut statuses = BTreeMap::new();
    for case in &cases {
        let output = one_skeleton_graph(&case.request, &case.policy);
        *statuses
            .entry(format!("{:?}", output.status).to_ascii_lowercase())
            .or_insert(0usize) += 1;
        if output.status == case.expected {
            exact_decisions += 1;
        }
        if output.authorized() {
            authorized_routes += 1;
        }
        if output.replay_verified() {
            replay_verified += 1;
        }
        let mut tampered = output.clone();
        tampered.replay_hash = "tampered".into();
        if !tampered.replay_verified() {
            tamper_rejected += 1;
        }
        if case.expected != BridgeStatus::Complete && output.authorized() {
            false_authorizations += 1;
        }
        if case.expected == BridgeStatus::Complete && !output.authorized() {
            false_denials += 1;
        }
    }
    let report = Report {
        schema: "stage125-simplicial-graph-composition-v1",
        corpus_sha256: digest(&cases),
        cases: cases.len(),
        supported: 120,
        ambiguous: 40,
        refused: 80,
        exact_decisions,
        authorized_routes,
        replay_verified,
        tamper_rejected,
        false_authorizations,
        false_denials,
        statuses,
    };
    assert_eq!(report.cases, 240);
    assert_eq!(report.exact_decisions, 240);
    assert_eq!(report.authorized_routes, 120);
    assert_eq!(report.replay_verified, 240);
    assert_eq!(report.tamper_rejected, 240);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
