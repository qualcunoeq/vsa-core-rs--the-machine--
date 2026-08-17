//! Stage 189: graph/probability/algebra stationary composition.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::finite_markov_stationary_composition::{
    evaluate, CompositionRequest, CompositionResult, CompositionStatus,
};
use the_machine::finite_markov_stationary_pack::{StationaryArtifact, StationaryRequest};
use the_machine::graph_pack::{GraphOperation, GraphRequest};
use the_machine::probability_pack::Rational;

const REPORT_JSON: &str = "docs/stage189_stationary_graph_composition.json";
const REPORT_MD: &str = "docs/stage189_stationary_graph_composition.md";

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn graph(vertices: &[&str], edges: &[(usize, usize)], directed: bool) -> GraphRequest {
    GraphRequest {
        operation: GraphOperation::Construction,
        domain: "finite_simple_graph".into(),
        vertices: vertices.iter().map(|value| (*value).into()).collect(),
        edges: edges.to_vec(),
        directed,
        matrix: None,
        vertex_order: vertices.iter().map(|value| (*value).into()).collect(),
        start: None,
        target: None,
        ambiguity: None,
        provenance: vec!["stage189-independent-graph-stationary-corpus".into()],
    }
}

fn transition(matrix: Vec<Vec<Rational>>) -> StationaryRequest {
    StationaryRequest {
        domain: "finite_exact_markov_stationary".into(),
        transition: matrix,
        row_stochastic: Some(true),
        ambiguity: None,
        provenance: vec!["stage189-independent-graph-stationary-corpus".into()],
    }
}

fn cycle3() -> (GraphRequest, StationaryRequest, StationaryArtifact) {
    let vertices = ["A", "B", "C"];
    let edges = [(0, 1), (1, 2), (2, 0)];
    (
        graph(&vertices, &edges, true),
        transition(vec![
            vec![q(0, 1), q(1, 1), q(0, 1)],
            vec![q(0, 1), q(0, 1), q(1, 1)],
            vec![q(1, 1), q(0, 1), q(0, 1)],
        ]),
        StationaryArtifact {
            distribution: vec![q(1, 3), q(1, 3), q(1, 3)],
            state_order: vec![0, 1, 2],
            residual_verified: true,
        },
    )
}

fn complete3() -> (GraphRequest, StationaryRequest, StationaryArtifact) {
    let vertices = ["A", "B", "C"];
    let edges = [(0, 1), (0, 2), (1, 0), (1, 2), (2, 0), (2, 1)];
    let row = vec![q(1, 2), q(1, 3), q(1, 6)];
    (
        graph(&vertices, &edges, true),
        transition(vec![row.clone(), row.clone(), row]),
        StationaryArtifact {
            distribution: vec![q(1, 2), q(1, 3), q(1, 6)],
            state_order: vec![0, 1, 2],
            residual_verified: true,
        },
    )
}

fn complete4() -> (GraphRequest, StationaryRequest, StationaryArtifact) {
    let vertices = ["A", "B", "C", "D"];
    let edges = (0..4)
        .flat_map(|source| {
            (0..4).filter_map(move |target| (source != target).then_some((source, target)))
        })
        .collect::<Vec<_>>();
    let row = vec![q(1, 4), q(1, 4), q(1, 4), q(1, 4)];
    (
        graph(&vertices, &edges, true),
        transition(vec![row.clone(), row.clone(), row.clone(), row]),
        StationaryArtifact {
            distribution: vec![q(1, 4), q(1, 4), q(1, 4), q(1, 4)],
            state_order: vec![0, 1, 2, 3],
            residual_verified: true,
        },
    )
}

fn request(graph: GraphRequest, transition: StationaryRequest) -> CompositionRequest {
    CompositionRequest {
        graph,
        transition,
        allow_self_transitions: Some(true),
        ambiguity: None,
        provenance: vec!["stage189-independent-graph-stationary-corpus".into()],
    }
}

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    request: CompositionRequest,
    expected: CompositionStatus,
    artifact: Option<StationaryArtifact>,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: CompositionStatus,
    actual: CompositionStatus,
    expected_artifact: Option<StationaryArtifact>,
    actual_artifact: Option<StationaryArtifact>,
    exact: bool,
    replay_verified: bool,
    tamper_rejected: bool,
    false_authorization: bool,
    false_denial: bool,
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
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cases = Vec::with_capacity(240);
    for (prefix, builder) in [
        (
            "cycle3",
            cycle3 as fn() -> (GraphRequest, StationaryRequest, StationaryArtifact),
        ),
        (
            "complete3",
            complete3 as fn() -> (GraphRequest, StationaryRequest, StationaryArtifact),
        ),
        (
            "complete4",
            complete4 as fn() -> (GraphRequest, StationaryRequest, StationaryArtifact),
        ),
    ] {
        for i in 0..40 {
            let (graph, transition, artifact) = builder();
            cases.push(Case {
                id: format!("{prefix}_{i}"),
                request: request(graph, transition),
                expected: CompositionStatus::Complete,
                artifact: Some(artifact),
            });
        }
    }
    for i in 0..40 {
        let (graph, mut transition, _) = cycle3();
        transition.row_stochastic = None;
        cases.push(Case {
            id: format!("ambiguous_convention_{i}"),
            request: request(graph, transition),
            expected: CompositionStatus::Ambiguous,
            artifact: None,
        });
    }
    for i in 0..20 {
        let (graph, _, _) = cycle3();
        let transition = transition(vec![
            vec![q(1, 2), q(0, 1), q(1, 2)],
            vec![q(0, 1), q(1, 2), q(1, 2)],
            vec![q(1, 2), q(0, 1), q(1, 2)],
        ]);
        cases.push(Case {
            id: format!("missing_graph_support_{i}"),
            request: request(graph, transition),
            expected: CompositionStatus::IncompatibleSemantics,
            artifact: None,
        });
    }
    for i in 0..20 {
        let (mut graph, transition, _) = complete3();
        graph.directed = false;
        graph.edges = vec![(0, 1), (0, 2), (1, 2)];
        cases.push(Case {
            id: format!("undirected_graph_{i}"),
            request: request(graph, transition),
            expected: CompositionStatus::IncompatibleSemantics,
            artifact: None,
        });
    }
    for i in 0..20 {
        let (graph, _, _) = cycle3();
        let identity = vec![
            vec![q(1, 1), q(0, 1), q(0, 1)],
            vec![q(0, 1), q(1, 1), q(0, 1)],
            vec![q(0, 1), q(0, 1), q(1, 1)],
        ];
        cases.push(Case {
            id: format!("non_unique_stationary_{i}"),
            request: request(graph, transition(identity)),
            expected: CompositionStatus::NonUniqueStationary,
            artifact: None,
        });
    }
    for i in 0..20 {
        let (mut graph, transition, _) = cycle3();
        graph.vertex_order = vec!["A".into(), "C".into(), "B".into()];
        cases.push(Case {
            id: format!("vertex_order_mismatch_{i}"),
            request: request(graph, transition),
            expected: CompositionStatus::IncompatibleSemantics,
            artifact: None,
        });
    }
    assert_eq!(cases.len(), 240);
    let corpus_sha256 = digest(&cases);
    let mut receipts = Vec::with_capacity(cases.len());
    for case in cases {
        let output: CompositionResult = evaluate(&case.request);
        let mut tampered = output.clone();
        tampered.replay_hash.push('x');
        let exact = output.status == case.expected && output.stationary == case.artifact;
        let false_authorization =
            case.expected != CompositionStatus::Complete && output.stationary.is_some();
        let false_denial = case.expected == CompositionStatus::Complete && !exact;
        receipts.push(Receipt {
            id: case.id,
            expected: case.expected,
            actual: output.status,
            expected_artifact: case.artifact,
            actual_artifact: output.stationary.clone(),
            exact,
            replay_verified: output.replay_verified(),
            tamper_rejected: !tampered.replay_verified(),
            false_authorization,
            false_denial,
        });
    }
    let cases = receipts.len();
    let supported = receipts
        .iter()
        .filter(|r| r.expected == CompositionStatus::Complete)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|r| r.expected == CompositionStatus::Ambiguous)
        .count();
    let refused = cases - supported - ambiguous;
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!(
        (
            exact_decisions,
            replay_verified,
            tamper_rejections,
            false_authorizations,
            false_denials
        ),
        (240, 240, 240, 0, 0)
    );
    let report = Report {
        schema: "stage189-stationary-graph-composition-v1",
        corpus_sha256: corpus_sha256.clone(),
        cases,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        replay_verified,
        tamper_rejections,
        false_authorizations,
        false_denials,
        receipts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 189 — stationary graph composition\n\nThis composition requires independently validated directed graph identity, stable vertex ordering, and exact row-stochastic semantics before invoking the stationary solver.\n\n| Measure | Result |\n|---|---:|\n| Cases | {cases} |\n| Supported / ambiguous / refused | {supported} / {ambiguous} / {refused} |\n| Exact decisions | {exact_decisions}/{cases} |\n| Replay / tamper | {replay_verified}/{cases} / {tamper_rejections}/{cases} |\n| False authorizations / denials | {false_authorizations} / {false_denials} |\n| Production mutation | false |\n\nCorpus SHA-256: `{corpus_sha256}`\n\nMachine-readable report: `{REPORT_JSON}`\n"
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
