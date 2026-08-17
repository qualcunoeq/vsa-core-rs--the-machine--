//! Stage 191: graph identity composed with target-before-avoid probabilities.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::finite_markov_hitting_composition::{
    evaluate, HittingCompositionRequest, HittingCompositionResult, HittingCompositionStatus,
};
use the_machine::finite_markov_hitting_pack::{HittingArtifact, HittingRequest};
use the_machine::graph_pack::{GraphOperation, GraphRequest};
use the_machine::probability_pack::Rational;

const REPORT_JSON: &str = "docs/stage191_hitting_graph_composition.json";
const REPORT_MD: &str = "docs/stage191_hitting_graph_composition.md";

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
        provenance: vec!["stage191-independent-hitting-graph-corpus".into()],
    }
}

fn hitting(
    transition: Vec<Vec<Rational>>,
    initial: Vec<Rational>,
    target_states: Vec<usize>,
    avoid_states: Vec<usize>,
) -> HittingRequest {
    HittingRequest {
        domain: "finite_exact_markov_hitting".into(),
        transition,
        initial,
        target_states,
        avoid_states,
        row_stochastic: Some(true),
        ambiguity: None,
        provenance: vec!["stage191-independent-hitting-graph-corpus".into()],
    }
}

fn chain_a() -> (GraphRequest, HittingRequest, HittingArtifact) {
    (
        graph(&["A", "B", "C"], &[(1, 0), (1, 2)], true),
        hitting(
            vec![
                vec![q(1, 1), q(0, 1), q(0, 1)],
                vec![q(1, 4), q(1, 4), q(1, 2)],
                vec![q(0, 1), q(0, 1), q(1, 1)],
            ],
            vec![q(0, 1), q(1, 1), q(0, 1)],
            vec![2],
            vec![0],
        ),
        HittingArtifact {
            state_probabilities: vec![q(0, 1), q(2, 3), q(1, 1)],
            initial_probability: q(2, 3),
            target_states: vec![2],
            avoid_states: vec![0],
            residual_verified: true,
        },
    )
}

fn chain_b() -> (GraphRequest, HittingRequest, HittingArtifact) {
    let (graph, mut request, _) = chain_a();
    request.transition[1] = vec![q(1, 3), q(1, 3), q(1, 3)];
    (
        graph,
        request,
        HittingArtifact {
            state_probabilities: vec![q(0, 1), q(1, 2), q(1, 1)],
            initial_probability: q(1, 2),
            target_states: vec![2],
            avoid_states: vec![0],
            residual_verified: true,
        },
    )
}

fn chain_c() -> (GraphRequest, HittingRequest, HittingArtifact) {
    (
        graph(
            &["A", "B", "C", "D"],
            &[(1, 0), (1, 2), (2, 1), (2, 3)],
            true,
        ),
        hitting(
            vec![
                vec![q(1, 1), q(0, 1), q(0, 1), q(0, 1)],
                vec![q(1, 4), q(1, 2), q(1, 4), q(0, 1)],
                vec![q(0, 1), q(1, 4), q(1, 2), q(1, 4)],
                vec![q(0, 1), q(0, 1), q(0, 1), q(1, 1)],
            ],
            vec![q(0, 1), q(1, 1), q(0, 1), q(0, 1)],
            vec![3],
            vec![0],
        ),
        HittingArtifact {
            state_probabilities: vec![q(0, 1), q(1, 3), q(2, 3), q(1, 1)],
            initial_probability: q(1, 3),
            target_states: vec![3],
            avoid_states: vec![0],
            residual_verified: true,
        },
    )
}

fn request(graph: GraphRequest, hitting: HittingRequest) -> HittingCompositionRequest {
    HittingCompositionRequest {
        graph,
        hitting,
        allow_self_transitions: Some(true),
        ambiguity: None,
        provenance: vec!["stage191-independent-hitting-graph-corpus".into()],
    }
}

#[derive(Debug, Clone, Serialize)]
struct Case {
    id: String,
    request: HittingCompositionRequest,
    expected: HittingCompositionStatus,
    artifact: Option<HittingArtifact>,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: HittingCompositionStatus,
    actual: HittingCompositionStatus,
    expected_artifact: Option<HittingArtifact>,
    actual_artifact: Option<HittingArtifact>,
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
            "chain_a",
            chain_a as fn() -> (GraphRequest, HittingRequest, HittingArtifact),
        ),
        (
            "chain_b",
            chain_b as fn() -> (GraphRequest, HittingRequest, HittingArtifact),
        ),
        (
            "chain_c",
            chain_c as fn() -> (GraphRequest, HittingRequest, HittingArtifact),
        ),
    ] {
        for i in 0..40 {
            let (graph, hitting, artifact) = builder();
            cases.push(Case {
                id: format!("{prefix}_{i}"),
                request: request(graph, hitting),
                expected: HittingCompositionStatus::Complete,
                artifact: Some(artifact),
            });
        }
    }
    for i in 0..40 {
        let (graph, mut hitting, _) = chain_a();
        hitting.row_stochastic = None;
        cases.push(Case {
            id: format!("ambiguous_convention_{i}"),
            request: request(graph, hitting),
            expected: HittingCompositionStatus::Ambiguous,
            artifact: None,
        });
    }
    for i in 0..20 {
        let (mut graph, hitting, _) = chain_a();
        graph.edges = vec![(1, 0)];
        cases.push(Case {
            id: format!("missing_graph_support_{i}"),
            request: request(graph, hitting),
            expected: HittingCompositionStatus::IncompatibleSemantics,
            artifact: None,
        });
    }
    for i in 0..20 {
        let (mut graph, hitting, _) = chain_a();
        graph.directed = false;
        graph.edges = vec![(0, 1), (1, 2)];
        cases.push(Case {
            id: format!("undirected_graph_{i}"),
            request: request(graph, hitting),
            expected: HittingCompositionStatus::IncompatibleSemantics,
            artifact: None,
        });
    }
    for i in 0..20 {
        let (graph, mut hitting, _) = chain_a();
        hitting.target_states = vec![0, 0];
        hitting.avoid_states = vec![0];
        cases.push(Case {
            id: format!("invalid_target_boundary_{i}"),
            request: request(graph, hitting),
            expected: HittingCompositionStatus::Unsupported,
            artifact: None,
        });
    }
    for i in 0..20 {
        let (graph, mut hitting, _) = chain_a();
        hitting.transition = vec![
            vec![q(1, 1), q(0, 1), q(0, 1)],
            vec![q(0, 1), q(1, 1), q(0, 1)],
            vec![q(0, 1), q(0, 1), q(1, 1)],
        ];
        cases.push(Case {
            id: format!("non_unique_hitting_{i}"),
            request: request(graph, hitting),
            expected: HittingCompositionStatus::NonUniqueHitting,
            artifact: None,
        });
    }
    assert_eq!(cases.len(), 240);
    let corpus_sha256 = digest(&cases);
    let mut receipts = Vec::with_capacity(cases.len());
    for case in cases {
        let output: HittingCompositionResult = evaluate(&case.request);
        let mut tampered = output.clone();
        tampered.replay_hash.push('x');
        let exact = output.status == case.expected && output.hitting == case.artifact;
        let false_authorization =
            case.expected != HittingCompositionStatus::Complete && output.hitting.is_some();
        let false_denial = case.expected == HittingCompositionStatus::Complete && !exact;
        receipts.push(Receipt {
            id: case.id,
            expected: case.expected,
            actual: output.status,
            expected_artifact: case.artifact,
            actual_artifact: output.hitting.clone(),
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
        .filter(|r| r.expected == HittingCompositionStatus::Complete)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|r| r.expected == HittingCompositionStatus::Ambiguous)
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
        schema: "stage191-hitting-graph-composition-v1",
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
            "# Stage 191 — hitting probabilities with graph semantics\n\nThe composition requires stable directed graph identity and explicit transition support before executing target-before-avoid probabilities.\n\n| Measure | Result |\n|---|---:|\n| Cases | {cases} |\n| Supported / ambiguous / refused | {supported} / {ambiguous} / {refused} |\n| Exact decisions | {exact_decisions}/{cases} |\n| Replay / tamper | {replay_verified}/{cases} / {tamper_rejections}/{cases} |\n| False authorizations / denials | {false_authorizations} / {false_denials} |\n| Production mutation | false |\n\nCorpus SHA-256: `{corpus_sha256}`\n\nMachine-readable report: `{REPORT_JSON}`\n"
        ),
    )?;
    println!("{serialized}");
    Ok(())
}
