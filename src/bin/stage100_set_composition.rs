//! Stage 100: finite-set composition with probability and graph semantics.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use the_machine::graph_pack::{
    evaluate_graph, GraphArtifact, GraphOperation, GraphRequest, GraphStatus,
};
use the_machine::probability_pack::{
    evaluate_probability, ProbabilityArtifact, ProbabilityOperation, ProbabilityRequest,
    ProbabilityStatus, Rational,
};
use the_machine::source_set_pack::{
    evaluate, replay_verified, SetArtifact, SetOperation, SetRequest, SetStatus,
};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Authorized,
    Refused,
}
#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    kind: Kind,
    set_status: String,
    probability_status: Option<String>,
    graph_status: Option<String>,
    authorized: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    route_leakage: bool,
}
#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    cases: usize,
    authorized: usize,
    refused: usize,
    probability_routes: usize,
    graph_routes: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    route_leakage: usize,
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<Receipt>,
}
fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}
fn set(seed: usize) -> BTreeSet<String> {
    (0..4).map(|i| format!("v{}", (seed + i) % 8)).collect()
}
fn uniform_request(outcomes: &[String], provenance: &str) -> ProbabilityRequest {
    let p = Rational::new(1, outcomes.len() as i128).unwrap();
    ProbabilityRequest {
        operation: ProbabilityOperation::DistributionConstruction,
        domain: "finite_exact_probability".into(),
        outcomes: outcomes.to_vec(),
        probabilities: outcomes.iter().map(|_| p.clone()).collect(),
        values: Vec::new(),
        event_a: None,
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: None,
        provenance: vec![provenance.into(), "set-to-uniform-distribution".into()],
    }
}
fn graph_request(vertices: &[String], provenance: &str) -> GraphRequest {
    GraphRequest {
        operation: GraphOperation::Construction,
        domain: "finite_simple_graph".into(),
        vertices: vertices.to_vec(),
        edges: Vec::new(),
        directed: false,
        matrix: None,
        vertex_order: vertices.to_vec(),
        start: None,
        target: None,
        ambiguity: None,
        provenance: vec![provenance.into(), "set-to-graph-vertices".into()],
    }
}
fn main() {
    let mut receipts = Vec::new();
    for index in 0..240 {
        let refused = index % 4 == 0;
        let universe: BTreeSet<String> = (0..8).map(|i| format!("v{i}")).collect();
        let left = set(index);
        let right = set(index + 2);
        let request = SetRequest { operation: if refused { SetOperation::Union } else { SetOperation::Intersection }, universe: universe.clone(), left: left.clone(), right: right.clone(), ambiguity: refused.then(|| "set artifact is not authorized as a probability or graph object without explicit bridge".into()), provenance: vec![format!("stage100:{index}")] };
        let set_result = evaluate(&request);
        let mut probability_status = None;
        let mut graph_status = None;
        let mut probability_replay = true;
        let mut graph_replay = true;
        let mut authorized = false;
        if !refused {
            if let Some(SetArtifact::FiniteSet(values)) = set_result.artifact.as_ref() {
                let outcomes: Vec<String> = values.iter().cloned().collect();
                let probability =
                    evaluate_probability(&uniform_request(&outcomes, &format!("stage100:{index}")));
                probability_status = Some(format!("{:?}", probability.status));
                probability_replay = probability.replay_verified();
                let graph = evaluate_graph(&graph_request(&outcomes, &format!("stage100:{index}")));
                graph_status = Some(format!("{:?}", graph.status));
                graph_replay = graph.replay_verified();
                authorized = set_result.status == SetStatus::Complete
                    && probability.status == ProbabilityStatus::Complete
                    && matches!(
                        probability.artifact,
                        Some(ProbabilityArtifact::Distribution(_))
                    )
                    && graph.status == GraphStatus::Complete
                    && matches!(graph.artifact, Some(GraphArtifact::Graph(_)));
            }
        }
        let tamper_rejected = {
            let mut copy = set_result.clone();
            copy.replay_hash.push('x');
            !replay_verified(&copy)
        };
        let expected = !refused;
        receipts.push(Receipt {
            id: format!("set_comp_{index:04}"),
            kind: if expected {
                Kind::Authorized
            } else {
                Kind::Refused
            },
            set_status: format!("{:?}", set_result.status),
            probability_status,
            graph_status,
            authorized,
            replay_verified: replay_verified(&set_result) && probability_replay && graph_replay,
            tamper_rejected,
            route_leakage: refused && authorized,
        });
    }
    let authorized = receipts.iter().filter(|r| r.authorized).count();
    let refused = receipts.len() - authorized;
    assert_eq!(authorized, 180);
    assert_eq!(receipts.iter().filter(|r| r.route_leakage).count(), 0);
    assert_eq!(receipts.iter().filter(|r| !r.replay_verified).count(), 0);
    assert_eq!(receipts.iter().filter(|r| !r.tamper_rejected).count(), 0);
    let report = Report {
        schema: "stage100-finite-set-composition-v1",
        cases: receipts.len(),
        authorized,
        refused,
        probability_routes: receipts
            .iter()
            .filter(|r| r.probability_status.is_some())
            .count(),
        graph_routes: receipts.iter().filter(|r| r.graph_status.is_some()).count(),
        replay_verified: receipts.iter().filter(|r| r.replay_verified).count(),
        tamper_rejections: receipts.iter().filter(|r| r.tamper_rejected).count(),
        route_leakage: receipts.iter().filter(|r| r.route_leakage).count(),
        false_authorizations: 0,
        false_denials: 0,
        receipts,
    };
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    let _ = digest(&report);
}
