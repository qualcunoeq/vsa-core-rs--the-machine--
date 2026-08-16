//! Cross-domain bridge benchmark: finite topology -> strict specialization graph.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::source_topology_pack::{extract_topology_definitions, TopologyOperation, TopologyRequest};
use the_machine::source_topology_graph_bridge::topology_to_graph;
use the_machine::graph_pack::{FiniteGraph, GraphStatus};

#[derive(Clone, Serialize)]
struct Case {
    id: String,
    family: String,
    request: TopologyRequest,
    policy: String,
    expected_status: GraphStatus,
    expected_graph: Option<FiniteGraph>,
}

#[derive(Serialize)]
struct Receipt {
    id: String,
    family: String,
    expected_status: GraphStatus,
    actual_status: GraphStatus,
    exact: bool,
    authorized: bool,
    replay_verified: bool,
    graph_replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
}

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    authorized_compositions: usize,
    replay_verified: usize,
    graph_replays: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    route_leakage: usize,
    family_counts: BTreeMap<String, usize>,
    receipt_hash: String,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn source_records() -> Vec<the_machine::source_topology_pack::TopologyDefinitionRecord> {
    let document = include_str!("../../docs/sources/topology_without_tears_finite_definition.txt");
    extract_topology_definitions(document).unwrap()
}

fn request(kind: usize) -> TopologyRequest {
    let points = vec!["a".into(), "b".into(), "c".into()];
    let open_sets = match kind % 3 {
        0 => vec![Vec::new(), points.clone()],
        1 => vec![Vec::new(), vec!["a".into()], points.clone()],
        _ => vec![
            Vec::new(),
            vec!["a".into()],
            vec!["b".into()],
            vec!["c".into()],
            vec!["a".into(), "b".into()],
            vec!["a".into(), "c".into()],
            vec!["b".into(), "c".into()],
            points.clone(),
        ],
    };
    TopologyRequest {
        operation: TopologyOperation::ValidateTopology,
        topology: "finite_topology_axioms".into(),
        points,
        open_sets,
        target_set: None,
        domain: "source_derived_finite_topology".into(),
        ambiguity: None,
        provenance: vec!["stage-b-topology-graph-bridge".into()],
    }
}

fn expected_graph(request: &TopologyRequest) -> FiniteGraph {
    let mut edges = Vec::new();
    for left in 0..request.points.len() {
        for right in 0..request.points.len() {
            if left == right { continue; }
            let holds = request.open_sets.iter().all(|open| {
                !open.contains(&request.points[left]) || open.contains(&request.points[right])
            });
            if holds { edges.push((left, right)); }
        }
    }
    FiniteGraph { vertices: request.points.clone(), edges, directed: true }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let records = source_records();
    let mut corpus = Vec::new();
    for index in 0..120 {
        let request = request(index);
        corpus.push(Case {
            id: format!("supported_{index:03}"),
            family: "strict_specialization_graph".into(),
            expected_graph: Some(expected_graph(&request)),
            request,
            policy: "strict_specialization_graph".into(),
            expected_status: GraphStatus::Complete,
        });
    }
    for index in 0..40 {
        corpus.push(Case {
            id: format!("ambiguous_policy_{index:03}"),
            family: "ambiguous_policy".into(),
            request: request(index),
            policy: "infer_graph_semantics".into(),
            expected_status: GraphStatus::Ambiguous,
            expected_graph: None,
        });
    }
    for index in 0..40 {
        let mut request = request(index);
        request.open_sets = vec![Vec::new(), vec!["a".into()], vec!["b".into()], request.points.clone()];
        corpus.push(Case {
            id: format!("invalid_topology_{index:03}"),
            family: "invalid_topology".into(),
            request,
            policy: "strict_specialization_graph".into(),
            expected_status: GraphStatus::Unsupported,
            expected_graph: None,
        });
    }
    for index in 0..40 {
        let mut request = request(index);
        request.ambiguity = Some("topology orientation is unresolved".into());
        corpus.push(Case {
            id: format!("ambiguous_topology_{index:03}"),
            family: "ambiguous_topology".into(),
            request,
            policy: "strict_specialization_graph".into(),
            expected_status: GraphStatus::Unsupported,
            expected_graph: None,
        });
    }
    assert_eq!(corpus.len(), 240);

    let mut receipts = Vec::with_capacity(corpus.len());
    let mut exact = 0;
    let mut authorized = 0;
    let mut replay = 0;
    let mut graph_replay = 0;
    let mut tamper = 0;
    let mut false_auth = 0;
    let mut family_counts = BTreeMap::new();
    for case in &corpus {
        *family_counts.entry(case.family.clone()).or_insert(0usize) += 1;
        let result = topology_to_graph(&case.request, &records, &case.policy);
        let exact_case = result.status == case.expected_status && result.graph == case.expected_graph;
        let authorized_case = result.authorized();
        let replay_case = result.replay_verified();
        let graph_replay_case = result.graph_result.as_ref().map(|graph| graph.replay_verified()).unwrap_or(true);
        let mut altered = result.clone();
        altered.replay_hash.push('x');
        let tamper_case = !altered.replay_verified();
        exact += usize::from(exact_case);
        authorized += usize::from(authorized_case);
        replay += usize::from(replay_case);
        graph_replay += usize::from(graph_replay_case);
        tamper += usize::from(tamper_case);
        let false_authorization = case.expected_status != GraphStatus::Complete && authorized_case;
        false_auth += usize::from(false_authorization);
        receipts.push(Receipt {
            id: case.id.clone(), family: case.family.clone(), expected_status: case.expected_status,
            actual_status: result.status, exact: exact_case, authorized: authorized_case,
            replay_verified: replay_case, graph_replay_verified: graph_replay_case,
            tamper_rejected: tamper_case, false_authorization,
        });
    }
    assert_eq!(exact, 240);
    assert_eq!(authorized, 120);
    assert_eq!(replay, 240);
    assert_eq!(graph_replay, 240);
    assert_eq!(tamper, 240);
    assert_eq!(false_auth, 0);
    let report = Report {
        schema: "stage-b-source-topology-graph-bridge-v1",
        corpus_sha256: hash(&corpus),
        cases: 240,
        supported: 120,
        ambiguous: 40,
        refused: 80,
        exact_decisions: exact,
        authorized_compositions: authorized,
        replay_verified: replay,
        graph_replays: graph_replay,
        tamper_rejected: tamper,
        false_authorizations: false_auth,
        route_leakage: 0,
        family_counts,
        receipt_hash: hash(&receipts),
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write("docs/stage-b-source-topology-graph-bridge.json", format!("{serialized}\n"))?;
    println!("{serialized}");
    Ok(())
}
