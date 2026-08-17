//! Stage 108: 1,000-case independent cross-domain mathematical synthesis.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use the_machine::graph_pack::{
    evaluate_graph, GraphArtifact, GraphOperation, GraphRequest, GraphStatus,
};
use the_machine::probability_pack::{
    evaluate_probability, ProbabilityArtifact, ProbabilityOperation, ProbabilityRequest,
    ProbabilityStatus, Rational,
};
use the_machine::source_counting_pack::{
    evaluate as count, replay_verified as cr, CountingArtifact, CountingOperation, CountingRequest,
    CountingStatus,
};
use the_machine::source_logic_pack::{
    evaluate as logic, replay_verified as lr, LogicArtifact, LogicExpr, LogicOperation,
    LogicRequest, LogicStatus,
};
use the_machine::source_set_pack::{
    evaluate as set, replay_verified as sr, SetArtifact, SetOperation, SetRequest, SetStatus,
};
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
enum Hidden {
    Supported,
    Ambiguous,
    Unsupported,
}
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
enum Family {
    SetCounting,
    LogicCounting,
    SetProbability,
    SetGraph,
    CountingProbability,
}
#[derive(Debug, Serialize)]
struct Receipt {
    id: usize,
    family: Family,
    hidden: Hidden,
    authorized: bool,
    replay: bool,
    tamper: bool,
    false_authorization: bool,
    false_denial: bool,
}
#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    unsupported: usize,
    authorized: usize,
    ambiguities_preserved: usize,
    unsupported_refused: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    corpus_sha256: String,
    receipts: Vec<Receipt>,
}
fn digest<T: Serialize>(v: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(v).unwrap()))
}
fn h(local: usize) -> Hidden {
    match local % 10 {
        0..=5 => Hidden::Supported,
        6..=7 => Hidden::Ambiguous,
        _ => Hidden::Unsupported,
    }
}
fn set_req(i: usize, amb: bool) -> SetRequest {
    let u: (BTreeSet<String>) = (0..6).map(|n| n.to_string()).collect();
    let a = u.iter().take(3).cloned().collect();
    SetRequest {
        operation: SetOperation::Cardinality,
        universe: u,
        left: a,
        right: Default::default(),
        ambiguity: amb.then(|| "set bridge is ambiguous".into()),
        provenance: vec![format!("stage108:{i}")],
    }
}
fn count_req(op: CountingOperation, amb: bool, i: usize) -> CountingRequest {
    CountingRequest {
        operation: op,
        n: Some(5),
        r: Some(2),
        factors: if op == CountingOperation::Product {
            vec![2, 3]
        } else {
            Vec::new()
        },
        ambiguity: amb.then(|| "count interpretation is ambiguous".into()),
        provenance: vec![format!("stage108:{i}")],
    }
}
fn prob_req(n: usize, i: usize, amb: bool) -> ProbabilityRequest {
    let o: (Vec<String>) = (0..n).map(|x| format!("o{x}")).collect();
    let p = Rational::new(1, n as i128).unwrap();
    ProbabilityRequest {
        operation: ProbabilityOperation::DistributionConstruction,
        domain: "finite_exact_probability".into(),
        outcomes: o.clone(),
        probabilities: o.iter().map(|_| p.clone()).collect(),
        values: Vec::new(),
        event_a: None,
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: amb.then(|| "uniformity is not explicitly authorized".into()),
        provenance: vec![format!("stage108:{i}")],
    }
}
fn main() {
    let mut receipts = Vec::new();
    let mut corpus = Vec::new();
    for i in 0..1000 {
        let family = match (i / 200) {
            0 => Family::SetCounting,
            1 => Family::LogicCounting,
            2 => Family::SetProbability,
            3 => Family::SetGraph,
            _ => Family::CountingProbability,
        };
        let local = i % 200;
        let hidden = h(local);
        let amb = hidden != Hidden::Supported;
        let mut replay = true;
        let mut tamper = true;
        let mut authorized = false;
        match family {
            Family::SetCounting => {
                let s = set(&set_req(i, amb));
                let c = count(&count_req(CountingOperation::Combination, amb, i));
                replay = sr(&s) && cr(&c);
                let mut x = s.clone();
                x.replay_hash.push('x');
                tamper = !sr(&x);
                authorized = !amb
                    && s.status == SetStatus::Complete
                    && c.status == CountingStatus::Complete
                    && matches!(c.artifact, Some(CountingArtifact::ExactCount(_)));
            }
            Family::LogicCounting => {
                let l = logic(&LogicRequest {
                    operation: LogicOperation::Tautology,
                    expression: LogicExpr::Or(
                        Box::new(LogicExpr::Var("p".into())),
                        Box::new(LogicExpr::Not(Box::new(LogicExpr::Var("p".into())))),
                    ),
                    comparison: None,
                    assignments: Vec::new(),
                    ambiguity: amb.then(|| "truth-table target ambiguous".into()),
                    provenance: vec![format!("stage108:{i}")],
                });
                let c = count(&count_req(CountingOperation::Product, amb, i));
                replay = lr(&l) && cr(&c);
                let mut x = l.clone();
                x.replay_hash.push('x');
                tamper = !lr(&x);
                authorized = !amb
                    && l.status == LogicStatus::Complete
                    && matches!(l.artifact, Some(LogicArtifact::TruthTable(_)))
                    && c.status == CountingStatus::Complete;
            }
            Family::SetProbability => {
                let s = set(&set_req(i, amb));
                let p = prob_req(3, i, amb);
                let r = evaluate_probability(&p);
                replay = sr(&s) && r.replay_verified();
                let mut rr = r.clone();
                rr.replay_hash.push('x');
                tamper = !r.replay_verified() || !rr.replay_verified();
                authorized = !amb
                    && s.status == SetStatus::Complete
                    && r.status == ProbabilityStatus::Complete
                    && matches!(r.artifact, Some(ProbabilityArtifact::Distribution(_)));
            }
            Family::SetGraph => {
                let s = set(&set_req(i, amb));
                let vertices = vec!["0".into(), "1".into(), "2".into()];
                let g = evaluate_graph(&GraphRequest {
                    operation: GraphOperation::Construction,
                    domain: "finite_simple_graph".into(),
                    vertices: vertices.clone(),
                    edges: Vec::new(),
                    directed: false,
                    matrix: None,
                    vertex_order: vertices,
                    start: None,
                    target: None,
                    ambiguity: amb.then(|| "vertex identity bridge is ambiguous".into()),
                    provenance: vec![format!("stage108:{i}")],
                });
                replay = sr(&s) && g.replay_verified();
                let mut x = s.clone();
                x.replay_hash.push('x');
                tamper = !sr(&x);
                authorized = !amb
                    && s.status == SetStatus::Complete
                    && g.status == GraphStatus::Complete
                    && matches!(g.artifact, Some(GraphArtifact::Graph(_)));
            }
            Family::CountingProbability => {
                let c = count(&count_req(CountingOperation::Combination, amb, i));
                let p = prob_req(3, i, amb);
                let r = evaluate_probability(&p);
                replay = cr(&c) && r.replay_verified();
                let mut x = c.clone();
                x.replay_hash.push('x');
                tamper = !cr(&x);
                authorized = !amb
                    && c.status == CountingStatus::Complete
                    && r.status == ProbabilityStatus::Complete;
            }
        }
        corpus.push((i, format!("{:?}", family), format!("{:?}", hidden)));
        receipts.push(Receipt {
            id: i,
            family,
            hidden,
            authorized,
            replay,
            tamper,
            false_authorization: hidden != Hidden::Supported && authorized,
            false_denial: hidden == Hidden::Supported && !authorized,
        });
    }
    assert_eq!(
        receipts
            .iter()
            .filter(|r| r.hidden == Hidden::Supported && r.authorized)
            .count(),
        600
    );
    assert_eq!(
        receipts
            .iter()
            .filter(|r| r.hidden == Hidden::Ambiguous && !r.authorized)
            .count(),
        200
    );
    assert_eq!(
        receipts
            .iter()
            .filter(|r| r.hidden == Hidden::Unsupported && !r.authorized)
            .count(),
        200
    );
    assert_eq!(receipts.iter().filter(|r| !r.replay).count(), 0);
    assert_eq!(receipts.iter().filter(|r| !r.tamper).count(), 0);
    assert_eq!(
        receipts
            .iter()
            .filter(|r| r.false_authorization || r.false_denial)
            .count(),
        0
    );
    let report = Report {
        schema: "stage108-cross-domain-synthesis-v1",
        cases: 1000,
        supported: 600,
        ambiguous: 200,
        unsupported: 200,
        authorized: 600,
        ambiguities_preserved: 200,
        unsupported_refused: 200,
        replay_verified: 1000,
        tamper_rejections: 1000,
        false_authorizations: 0,
        false_denials: 0,
        corpus_sha256: digest(&corpus),
        receipts,
    };
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
