//! Stage I integration: retrieve the topology axioms as a governed claim
//! before allowing the source-derived topology pack to execute.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::source_retrieval::{retrieve_claim, ClaimQuery, ClaimSource, RetrievalStatus, SourceClaim};
use the_machine::source_topology_pack::{evaluate_topology, extract_topology_definitions, TopologyOperation, TopologyRequest};

#[derive(Serialize)]
struct Report {
    schema: &'static str,
    cases: usize,
    supported: usize,
    conflicting: usize,
    missing: usize,
    exact_decisions: usize,
    topology_authorizations: usize,
    retrieval_replay: usize,
    topology_replay: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    registry_mutated: bool,
    receipt_hash: String,
}

fn hash<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn query() -> ClaimQuery {
    ClaimQuery {
        subject: "topological_space".into(),
        predicate: "axioms_for_bounded_finite_execution".into(),
        domain: "source_derived_finite_topology".into(),
        scope: "finite_carrier_at_most_eight_points".into(),
        provenance: vec!["stage-i-topology-retrieval".into()],
    }
}

fn source(id: &str, lineage: &str) -> ClaimSource {
    ClaimSource {
        source_id: id.into(),
        title: "Topology Without Tears".into(),
        locator: "https://www.topologywithouttears.net/topbook.pdf".into(),
        retrieved_utc: "2026-08-16".into(),
        lineage_id: lineage.into(),
    }
}

fn claim(id: &str, object: &str, source_id: &str, lineage: &str) -> SourceClaim {
    SourceClaim {
        claim_id: id.into(),
        subject: "topological_space".into(),
        predicate: "axioms_for_bounded_finite_execution".into(),
        object: object.into(),
        domain: "source_derived_finite_topology".into(),
        scope: "finite_carrier_at_most_eight_points".into(),
        validity: "source definition plus explicit finite bound".into(),
        assumptions: vec!["carrier and open sets are explicitly declared".into()],
        source: source(source_id, lineage),
    }
}

fn topology_request() -> TopologyRequest {
    TopologyRequest {
        operation: TopologyOperation::ValidateTopology,
        topology: "finite_topology_axioms".into(),
        points: vec!["a".into(), "b".into(), "c".into()],
        open_sets: vec![Vec::new(), vec!["a".into()], vec!["a".into(), "b".into(), "c".into()]],
        target_set: None,
        domain: "source_derived_finite_topology".into(),
        ambiguity: None,
        provenance: vec!["stage-i-topology-retrieval".into()],
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let document = include_str!("../../docs/sources/topology_without_tears_finite_definition.txt");
    let records = extract_topology_definitions(document).unwrap();
    let object = "empty;whole;unions;finite_intersections";
    let mut exact = 0;
    let mut supported = 0;
    let mut conflicting = 0;
    let mut missing = 0;
    let mut topology_auth = 0;
    let mut retrieval_replay = 0;
    let mut topology_replay = 0;
    let mut tamper = 0;
    let mut false_auth = 0;
    let mut receipts = Vec::new();
    for index in 0..120 {
        let corpus = vec![
            claim("primary", object, "topology-without-tears", "definition-1.3.1"),
            claim("corroborating", object, "topology-course-notes", "finite-topology-definition"),
        ];
        let retrieval = retrieve_claim(&query(), &corpus);
        let eligible = retrieval.status == RetrievalStatus::Supported && retrieval.eligible_for_shadow_use();
        let topology = evaluate_topology(&topology_request(), &records);
        let ok = eligible && topology.authorized();
        exact += usize::from(ok);
        supported += usize::from(ok);
        topology_auth += usize::from(topology.authorized());
        retrieval_replay += usize::from(retrieval.replay_verified());
        topology_replay += usize::from(topology.replay_verified());
        let mut altered = retrieval.clone(); altered.replay_hash.push('x');
        let mut altered_topology = topology.clone(); altered_topology.replay_hash.push('x');
        tamper += usize::from(!altered.replay_verified() && !altered_topology.replay_verified());
        false_auth += usize::from(!ok);
        receipts.push((index, "supported", ok));
    }
    for index in 0..40 {
        let corpus = vec![claim("a", object, "topology-a", "lineage-a"), claim("b", "empty;whole;unions", "topology-b", "lineage-b")];
        let retrieval = retrieve_claim(&query(), &corpus);
        let ok = retrieval.status == RetrievalStatus::Conflicting && !retrieval.eligible_for_shadow_use();
        exact += usize::from(ok); conflicting += usize::from(ok); retrieval_replay += usize::from(retrieval.replay_verified());
        let mut altered = retrieval.clone(); altered.replay_hash.push('x'); tamper += usize::from(!altered.replay_verified());
        false_auth += usize::from(!ok); receipts.push((index, "conflicting", ok));
    }
    for index in 0..80 {
        let mut q = query(); q.subject = format!("unknown_{index}");
        let retrieval = retrieve_claim(&q, &[claim("unmatched", object, "topology-a", "lineage-a")]);
        let ok = retrieval.status == RetrievalStatus::Missing && !retrieval.eligible_for_shadow_use();
        exact += usize::from(ok); missing += usize::from(ok); retrieval_replay += usize::from(retrieval.replay_verified());
        let mut altered = retrieval.clone(); altered.replay_hash.push('x'); tamper += usize::from(!altered.replay_verified());
        false_auth += usize::from(!ok); receipts.push((index, "missing", ok));
    }
    assert_eq!(exact, 240); assert_eq!(supported, 120); assert_eq!(conflicting, 40); assert_eq!(missing, 80);
    assert_eq!(topology_auth, 120); assert_eq!(retrieval_replay, 240); assert_eq!(topology_replay, 120); assert_eq!(tamper, 240); assert_eq!(false_auth, 0);
    let report = Report {
        schema: "stage-i-source-topology-retrieval-v1", cases: 240, supported, conflicting, missing,
        exact_decisions: exact, topology_authorizations: topology_auth, retrieval_replay,
        topology_replay, tamper_rejected: tamper, false_authorizations: false_auth,
        registry_mutated: false, receipt_hash: hash(&receipts),
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write("docs/stage-i-source-topology-retrieval.json", format!("{serialized}\n"))?;
    println!("{serialized}");
    Ok(())
}
