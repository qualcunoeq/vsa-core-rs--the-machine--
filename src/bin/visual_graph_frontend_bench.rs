//! Independent campaign for the governed coordinate-bearing visual graph
//! frontend and its handoff to the bounded finite-graph pack.

use serde::Serialize;
use sha2::{Digest, Sha256};
use the_machine::graph_pack::evaluate_graph;
use the_machine::probability_pack::{
    evaluate_probability, ProbabilityOperation, ProbabilityRequest, Rational,
};
use the_machine::random_walk_composition::TransitionConvention;
use the_machine::vision::visual_graph::{
    execute_one_step_random_walk, formalize_visual_graph, to_graph_request, VisualEdgeObservation,
    VisualGraphObservation, VisualGraphStatus, VisualNodeObservation,
};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    visual_status: VisualGraphStatus,
    authorized: bool,
    exact: bool,
    visual_replay_verified: bool,
    visual_tamper_rejected: bool,
    bridge_emitted: bool,
    graph_replay_verified: bool,
    graph_tamper_rejected: bool,
    walk_emitted: bool,
    walk_replay_verified: bool,
    walk_tamper_rejected: bool,
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
    supported_authorizations: usize,
    visual_replay_verified: usize,
    visual_tamper_rejections: usize,
    bridge_emitted: usize,
    graph_replay_verified: usize,
    graph_tamper_rejections: usize,
    walk_emitted: usize,
    walk_replay_verified: usize,
    walk_tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("visual graph benchmark serializes"))
    )
}

fn node(label: &str, left: u32) -> VisualNodeObservation {
    VisualNodeObservation {
        label: label.into(),
        left,
        top: 20,
        width: 24,
        height: 24,
        confidence: 99,
    }
}

fn supported_observation(index: usize) -> VisualGraphObservation {
    let directed = index % 2 == 0;
    VisualGraphObservation {
        semantic_label: Some("finite_simple_graph".into()),
        nodes: vec![node("a", 10), node("b", 60), node("c", 110)],
        edges: if directed {
            vec![
                VisualEdgeObservation {
                    from: "a".into(),
                    to: "b".into(),
                    directed: Some(true),
                    confidence: 98,
                },
                VisualEdgeObservation {
                    from: "b".into(),
                    to: "c".into(),
                    directed: Some(true),
                    confidence: 97,
                },
                VisualEdgeObservation {
                    from: "c".into(),
                    to: "a".into(),
                    directed: Some(true),
                    confidence: 96,
                },
            ]
        } else {
            vec![
                VisualEdgeObservation {
                    from: "a".into(),
                    to: "b".into(),
                    directed: Some(false),
                    confidence: 98,
                },
                VisualEdgeObservation {
                    from: "b".into(),
                    to: "c".into(),
                    directed: Some(false),
                    confidence: 97,
                },
            ]
        },
        directed: Some(directed),
        ambiguity: None,
        provenance: vec![
            format!("diagram:supported:{index}"),
            "coordinates:fixed".into(),
        ],
    }
}

fn rational(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("exact rational")
}

fn initial_distribution() -> the_machine::probability_pack::ProbabilityResult {
    evaluate_probability(&ProbabilityRequest {
        operation: ProbabilityOperation::DistributionConstruction,
        domain: "finite_exact_probability".into(),
        outcomes: vec!["a".into(), "b".into(), "c".into()],
        probabilities: vec![rational(1, 1), rational(0, 1), rational(0, 1)],
        values: Vec::new(),
        event_a: None,
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: None,
        provenance: vec!["visual-graph-benchmark:initial-distribution".into()],
    })
}

fn transition(index: usize) -> Vec<Vec<Rational>> {
    if index % 2 == 0 {
        vec![
            vec![rational(0, 1), rational(1, 1), rational(0, 1)],
            vec![rational(0, 1), rational(0, 1), rational(1, 1)],
            vec![rational(1, 1), rational(0, 1), rational(0, 1)],
        ]
    } else {
        vec![
            vec![rational(0, 1), rational(1, 1), rational(0, 1)],
            vec![rational(1, 2), rational(0, 1), rational(1, 2)],
            vec![rational(0, 1), rational(1, 1), rational(0, 1)],
        ]
    }
}

fn ambiguous_observation(index: usize) -> VisualGraphObservation {
    let mut observation = supported_observation(index);
    observation.directed = None;
    observation
        .edges
        .iter_mut()
        .for_each(|edge| edge.directed = None);
    observation.provenance = vec![format!("diagram:ambiguous:{index}")];
    observation
}

fn refused_observation(index: usize) -> VisualGraphObservation {
    let mut observation = supported_observation(index);
    match index % 4 {
        0 => observation.semantic_label = Some("weighted_graph".into()),
        1 => observation.edges[0].to = "unknown".into(),
        2 => observation.edges.push(VisualEdgeObservation {
            from: "a".into(),
            to: "b".into(),
            directed: Some(false),
            confidence: 99,
        }),
        _ => observation.edges[0].from = "a".into(),
    }
    if index % 4 == 3 {
        observation.edges[0].to = "a".into();
    }
    observation.provenance = vec![format!("diagram:refused:{index}")];
    observation
}

fn run(id: String, observation: VisualGraphObservation, expected: Expected) -> Receipt {
    let visual = formalize_visual_graph(&observation);
    let authorized = visual.authorized();
    let mut visual_tampered = visual.clone();
    visual_tampered.replay_hash.push('x');
    let request = to_graph_request(&visual);
    let bridge_emitted = request.is_some();
    let (graph_replay_verified, graph_tamper_rejected) = if let Some(request) = request {
        let graph = evaluate_graph(&request);
        let mut tampered = graph.clone();
        tampered.replay_hash.push('x');
        (graph.replay_verified(), !tampered.replay_verified())
    } else {
        (false, false)
    };
    let (walk_emitted, walk_replay_verified, walk_tamper_rejected) =
        if expected == Expected::Supported {
            let initial = initial_distribution();
            let walk = execute_one_step_random_walk(
                &visual,
                Some(&transition(
                    id.strip_prefix("supported_").unwrap().parse().unwrap_or(0),
                )),
                &initial,
                Some(TransitionConvention::RowStochastic),
                vec!["visual-graph-benchmark:explicit-transition".into()],
            )
            .expect("complete visual graph emits walk route");
            let mut tampered = walk.clone();
            tampered.replay_hash.push('x');
            (true, walk.replay_verified(), !tampered.replay_verified())
        } else {
            (false, false, false)
        };
    let exact = match expected {
        Expected::Supported => {
            authorized
                && visual.status == VisualGraphStatus::Complete
                && bridge_emitted
                && graph_replay_verified
                && walk_emitted
                && walk_replay_verified
        }
        Expected::Ambiguous => !authorized && visual.status == VisualGraphStatus::Ambiguous,
        Expected::Refused => !authorized,
    };
    Receipt {
        id,
        expected,
        visual_status: visual.status,
        authorized,
        exact,
        visual_replay_verified: visual.replay_verified(),
        visual_tamper_rejected: !visual_tampered.replay_verified(),
        bridge_emitted,
        graph_replay_verified,
        graph_tamper_rejected,
        walk_emitted,
        walk_replay_verified,
        walk_tamper_rejected,
        false_authorization: expected != Expected::Supported && authorized,
        false_denial: expected == Expected::Supported && !authorized,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut receipts = Vec::with_capacity(240);
    for index in 0..120 {
        receipts.push(run(
            format!("supported_{index:03}"),
            supported_observation(index),
            Expected::Supported,
        ));
    }
    for index in 0..40 {
        receipts.push(run(
            format!("ambiguous_{index:03}"),
            ambiguous_observation(index),
            Expected::Ambiguous,
        ));
    }
    for index in 0..80 {
        receipts.push(run(
            format!("refused_{index:03}"),
            refused_observation(index),
            Expected::Refused,
        ));
    }

    let supported = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Supported)
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Ambiguous)
        .count();
    let refused = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Refused)
        .count();
    let exact_decisions = receipts.iter().filter(|receipt| receipt.exact).count();
    let supported_authorizations = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Supported && receipt.authorized)
        .count();
    let visual_replay_verified = receipts
        .iter()
        .filter(|receipt| receipt.visual_replay_verified)
        .count();
    let visual_tamper_rejections = receipts
        .iter()
        .filter(|receipt| receipt.visual_tamper_rejected)
        .count();
    let bridge_emitted = receipts
        .iter()
        .filter(|receipt| receipt.bridge_emitted)
        .count();
    let graph_replay_verified = receipts
        .iter()
        .filter(|receipt| receipt.graph_replay_verified)
        .count();
    let graph_tamper_rejections = receipts
        .iter()
        .filter(|receipt| receipt.graph_tamper_rejected)
        .count();
    let walk_emitted = receipts
        .iter()
        .filter(|receipt| receipt.walk_emitted)
        .count();
    let walk_replay_verified = receipts
        .iter()
        .filter(|receipt| receipt.walk_replay_verified)
        .count();
    let walk_tamper_rejections = receipts
        .iter()
        .filter(|receipt| receipt.walk_tamper_rejected)
        .count();
    let false_authorizations = receipts
        .iter()
        .filter(|receipt| receipt.false_authorization)
        .count();
    let false_denials = receipts
        .iter()
        .filter(|receipt| receipt.false_denial)
        .count();
    let report = Report {
        schema: "stage-j-visual-graph-composition-v2",
        corpus_sha256: digest(&receipts),
        cases: receipts.len(),
        supported,
        ambiguous,
        refused,
        exact_decisions,
        supported_authorizations,
        visual_replay_verified,
        visual_tamper_rejections,
        bridge_emitted,
        graph_replay_verified,
        graph_tamper_rejections,
        walk_emitted,
        walk_replay_verified,
        walk_tamper_rejections,
        false_authorizations,
        false_denials,
        receipts,
    };
    assert_eq!(report.cases, 240);
    assert_eq!(report.supported, 120);
    assert_eq!(report.ambiguous, 40);
    assert_eq!(report.refused, 80);
    assert_eq!(report.exact_decisions, 240);
    assert_eq!(report.supported_authorizations, 120);
    assert_eq!(report.visual_replay_verified, 240);
    assert_eq!(report.visual_tamper_rejections, 240);
    assert_eq!(report.bridge_emitted, 120);
    assert_eq!(report.graph_replay_verified, 120);
    assert_eq!(report.graph_tamper_rejections, 120);
    assert_eq!(report.walk_emitted, 120);
    assert_eq!(report.walk_replay_verified, 120);
    assert_eq!(report.walk_tamper_rejections, 120);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);

    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage_j_visual_graph_frontend.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
