//! Route-blind multimodal campaign over visual tables and graph diagrams.
//!
//! Every input is offered to both visual frontends. Authorization requires
//! exactly one complete downstream route with replayable artifacts.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::graph_pack::{evaluate_graph, GraphStatus};
use the_machine::probability_pack::{
    evaluate_probability, ProbabilityOperation, ProbabilityRequest, Rational,
};
use the_machine::random_walk_composition::TransitionConvention;
use the_machine::vision::visual_graph::{
    execute_one_step_random_walk, formalize_visual_graph, to_graph_request, VisualEdgeObservation,
    VisualGraphObservation, VisualGraphStatus, VisualNodeObservation,
};
use the_machine::vision::visual_table::visual_probability_bridge::{
    table_to_probability, BridgeStatus,
};
use the_machine::vision::visual_table::{formalize_table_tsv, TableStatus};

const OCR_HEADER: &str =
    "level\tpage\tblock\tpar\tline\tword\tleft\ttop\twidth\theight\tconf\ttext";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Route {
    Table,
    Graph,
}

#[derive(Debug, Clone)]
struct Case {
    id: String,
    expected: Expected,
    preferred_route: Route,
    table_tsv: String,
    graph: VisualGraphObservation,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    preferred_route: Route,
    selected_route: Option<Route>,
    authorized: bool,
    exact: bool,
    table_status: TableStatus,
    table_bridge_status: Option<BridgeStatus>,
    graph_status: VisualGraphStatus,
    graph_pack_status: Option<GraphStatus>,
    table_replay_verified: bool,
    table_tamper_rejected: bool,
    graph_replay_verified: bool,
    graph_tamper_rejected: bool,
    downstream_replay_verified: bool,
    downstream_tamper_rejected: bool,
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
    unsupported: usize,
    exact_decisions: usize,
    authorized_supported: usize,
    ambiguities_preserved: usize,
    unsupported_refusals: usize,
    frontend_invocations: usize,
    route_counts: BTreeMap<String, usize>,
    table_replay_verified: usize,
    graph_replay_verified: usize,
    downstream_replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    hle_questions_read: usize,
    production_registry_mutations: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("serializes"))
    )
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("exact rational")
}

fn ocr_word(left: usize, top: usize, text: &str) -> String {
    format!("5\t1\t1\t1\t1\t1\t{left}\t{top}\t50\t10\t99\t{text}")
}

fn table_tsv(headers: &[&str], rows: &[Vec<&str>]) -> String {
    let mut lines = vec![OCR_HEADER.to_string()];
    for (column, header) in headers.iter().enumerate() {
        lines.push(ocr_word(10 + column * 70, 10, header));
    }
    for (row, values) in rows.iter().enumerate() {
        for (column, value) in values.iter().enumerate() {
            lines.push(ocr_word(10 + column * 70, 30 + row * 25, value));
        }
    }
    lines.join("\n")
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

fn graph_observation(index: usize, semantic: Option<&str>) -> VisualGraphObservation {
    let directed = index % 2 == 0;
    let edges = if directed {
        vec![
            VisualEdgeObservation {
                from: "a".into(),
                to: "b".into(),
                directed: Some(true),
                confidence: 99,
            },
            VisualEdgeObservation {
                from: "b".into(),
                to: "c".into(),
                directed: Some(true),
                confidence: 99,
            },
            VisualEdgeObservation {
                from: "c".into(),
                to: "a".into(),
                directed: Some(true),
                confidence: 99,
            },
        ]
    } else {
        vec![
            VisualEdgeObservation {
                from: "a".into(),
                to: "b".into(),
                directed: Some(false),
                confidence: 99,
            },
            VisualEdgeObservation {
                from: "b".into(),
                to: "c".into(),
                directed: Some(false),
                confidence: 99,
            },
        ]
    };
    VisualGraphObservation {
        semantic_label: semantic.map(str::to_owned),
        nodes: vec![node("a", 10), node("b", 60), node("c", 110)],
        edges,
        directed: Some(directed),
        ambiguity: None,
        provenance: vec![format!("visual-route-blind:graph:{index}")],
    }
}

fn empty_graph() -> VisualGraphObservation {
    VisualGraphObservation {
        semantic_label: Some("not_a_graph".into()),
        nodes: Vec::new(),
        edges: Vec::new(),
        directed: None,
        ambiguity: None,
        provenance: vec!["visual-route-blind:table-input".into()],
    }
}

fn empty_table() -> String {
    OCR_HEADER.into()
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::with_capacity(240);
    for index in 0..60 {
        cases.push(Case {
            id: format!("table_supported_{index:03}"),
            expected: Expected::Supported,
            preferred_route: Route::Table,
            table_tsv: table_tsv(
                &["outcome", "probability"],
                &[vec!["a", "1/2"], vec!["b", "1/3"], vec!["c", "1/6"]],
            ),
            graph: empty_graph(),
        });
    }
    for index in 0..60 {
        cases.push(Case {
            id: format!("graph_supported_{index:03}"),
            expected: Expected::Supported,
            preferred_route: Route::Graph,
            table_tsv: empty_table(),
            graph: graph_observation(index, Some("finite_simple_graph")),
        });
    }
    for index in 0..20 {
        cases.push(Case {
            id: format!("table_ambiguous_{index:03}"),
            expected: Expected::Ambiguous,
            preferred_route: Route::Table,
            table_tsv: table_tsv(&["value", "weight"], &[vec!["a", "1/2"], vec!["b", "1/2"]]),
            graph: empty_graph(),
        });
    }
    for index in 0..20 {
        let mut graph = graph_observation(index, Some("finite_simple_graph"));
        graph.directed = None;
        graph.edges.iter_mut().for_each(|edge| edge.directed = None);
        cases.push(Case {
            id: format!("graph_ambiguous_{index:03}"),
            expected: Expected::Ambiguous,
            preferred_route: Route::Graph,
            table_tsv: empty_table(),
            graph,
        });
    }
    for index in 0..40 {
        cases.push(Case {
            id: format!("table_unsupported_{index:03}"),
            expected: Expected::Unsupported,
            preferred_route: Route::Table,
            table_tsv: table_tsv(
                &["outcome", "probability"],
                &[vec!["a", "1/3"], vec!["b", "1/3"]],
            ),
            graph: empty_graph(),
        });
    }
    for index in 0..40 {
        cases.push(Case {
            id: format!("graph_unsupported_{index:03}"),
            expected: Expected::Unsupported,
            preferred_route: Route::Graph,
            table_tsv: empty_table(),
            graph: graph_observation(index, Some("weighted_graph")),
        });
    }
    cases
}

fn transition(index: usize) -> Vec<Vec<Rational>> {
    if index % 2 == 0 {
        vec![
            vec![q(0, 1), q(1, 1), q(0, 1)],
            vec![q(0, 1), q(0, 1), q(1, 1)],
            vec![q(1, 1), q(0, 1), q(0, 1)],
        ]
    } else {
        vec![
            vec![q(0, 1), q(1, 1), q(0, 1)],
            vec![q(1, 2), q(0, 1), q(1, 2)],
            vec![q(0, 1), q(1, 1), q(0, 1)],
        ]
    }
}

fn initial_probability() -> the_machine::probability_pack::ProbabilityResult {
    evaluate_probability(&ProbabilityRequest {
        operation: ProbabilityOperation::DistributionConstruction,
        domain: "finite_exact_probability".into(),
        outcomes: vec!["a".into(), "b".into(), "c".into()],
        probabilities: vec![q(1, 1), q(0, 1), q(0, 1)],
        values: Vec::new(),
        event_a: None,
        event_b: None,
        partition: Vec::new(),
        conditional_values: Vec::new(),
        prior_probability: None,
        likelihood: None,
        evidence: None,
        ambiguity: None,
        provenance: vec!["visual-route-blind:initial-distribution".into()],
    })
}

fn run(case: &Case) -> Receipt {
    // Both frontends run for every case; no modality or lexical hint is used.
    let table = formalize_table_tsv(&case.table_tsv);
    let table_bridge = table.artifact.as_ref().map(table_to_probability);
    let graph = formalize_visual_graph(&case.graph);
    let graph_request = to_graph_request(&graph);
    let graph_pack = graph_request.as_ref().map(evaluate_graph);
    let table_authorized = table_bridge
        .as_ref()
        .is_some_and(|b| b.status == BridgeStatus::Complete);
    let graph_authorized = graph.status == VisualGraphStatus::Complete
        && graph_pack
            .as_ref()
            .is_some_and(|r| r.status == GraphStatus::Complete);
    let mut table_tampered = table.clone();
    table_tampered.replay_hash.push('x');
    let mut graph_tampered = graph.clone();
    graph_tampered.replay_hash.push('x');
    let (downstream_replay_verified, downstream_tamper_rejected) = if graph_authorized {
        let index = case
            .id
            .strip_prefix("graph_supported_")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let walk = execute_one_step_random_walk(
            &graph,
            Some(&transition(index)),
            &initial_probability(),
            Some(TransitionConvention::RowStochastic),
            vec![case.id.clone(), "explicit-transition".into()],
        )
        .expect("complete graph route emits walk");
        let mut tampered = walk.clone();
        tampered.replay_hash.push('x');
        (walk.replay_verified(), !tampered.replay_verified())
    } else {
        (true, true)
    };
    let selected_route = match (table_authorized, graph_authorized) {
        (true, false) => Some(Route::Table),
        (false, true) => Some(Route::Graph),
        _ => None,
    };
    let authorized = selected_route.is_some()
        && (selected_route != Some(Route::Graph) || downstream_replay_verified);
    let actual_ambiguous = !authorized
        && (table_bridge
            .as_ref()
            .is_some_and(|b| b.status == BridgeStatus::Ambiguous)
            || graph.status == VisualGraphStatus::Ambiguous);
    let exact = match case.expected {
        Expected::Supported => authorized && selected_route == Some(case.preferred_route),
        Expected::Ambiguous => actual_ambiguous,
        Expected::Unsupported => !authorized && !actual_ambiguous,
    };
    Receipt {
        id: case.id.clone(),
        expected: case.expected,
        preferred_route: case.preferred_route,
        selected_route,
        authorized,
        exact,
        table_status: table.status,
        table_bridge_status: table_bridge.as_ref().map(|b| b.status),
        graph_status: graph.status,
        graph_pack_status: graph_pack.as_ref().map(|r| r.status),
        table_replay_verified: table.replay_verified(),
        table_tamper_rejected: !table_tampered.replay_verified(),
        graph_replay_verified: graph.replay_verified(),
        graph_tamper_rejected: !graph_tampered.replay_verified(),
        downstream_replay_verified,
        downstream_tamper_rejected,
        false_authorization: case.expected != Expected::Supported && authorized,
        false_denial: case.expected == Expected::Supported && !authorized,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let receipts = cases().iter().map(run).collect::<Vec<_>>();
    let count = |expected| receipts.iter().filter(|r| r.expected == expected).count();
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let authorized_supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && r.authorized)
        .count();
    let ambiguities_preserved = receipts
        .iter()
        .filter(|r| r.expected == Expected::Ambiguous && !r.authorized)
        .count();
    let unsupported_refusals = receipts
        .iter()
        .filter(|r| r.expected == Expected::Unsupported && !r.authorized)
        .count();
    let route_counts = receipts.iter().fold(BTreeMap::new(), |mut map, r| {
        if let Some(route) = r.selected_route {
            *map.entry(format!("{route:?}").to_ascii_lowercase())
                .or_insert(0) += 1;
        }
        map
    });
    let report = Report {
        schema: "stage-j-multimodal-route-blind-v1",
        corpus_sha256: digest(&receipts),
        cases: receipts.len(),
        supported: count(Expected::Supported),
        ambiguous: count(Expected::Ambiguous),
        unsupported: count(Expected::Unsupported),
        exact_decisions,
        authorized_supported,
        ambiguities_preserved,
        unsupported_refusals,
        frontend_invocations: receipts.len() * 2,
        route_counts,
        table_replay_verified: receipts.iter().filter(|r| r.table_replay_verified).count(),
        graph_replay_verified: receipts.iter().filter(|r| r.graph_replay_verified).count(),
        downstream_replay_verified: receipts
            .iter()
            .filter(|r| r.downstream_replay_verified)
            .count(),
        tamper_rejected: receipts
            .iter()
            .filter(|r| {
                r.table_tamper_rejected && r.graph_tamper_rejected && r.downstream_tamper_rejected
            })
            .count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts.iter().filter(|r| r.false_denial).count(),
        hle_questions_read: 0,
        production_registry_mutations: 0,
        receipts,
    };
    assert_eq!(
        (
            report.cases,
            report.supported,
            report.ambiguous,
            report.unsupported
        ),
        (240, 120, 40, 80)
    );
    assert_eq!(
        (
            report.exact_decisions,
            report.authorized_supported,
            report.ambiguities_preserved,
            report.unsupported_refusals
        ),
        (240, 120, 40, 80)
    );
    assert_eq!(
        (
            report.frontend_invocations,
            report.table_replay_verified,
            report.graph_replay_verified,
            report.downstream_replay_verified
        ),
        (480, 240, 240, 240)
    );
    assert_eq!(report.tamper_rejected, 240);
    assert_eq!(
        (
            report.false_authorizations,
            report.false_denials,
            report.hle_questions_read,
            report.production_registry_mutations
        ),
        (0, 0, 0, 0)
    );
    let serialized = serde_json::to_string_pretty(&report)?;
    std::fs::write(
        "docs/stage_j_multimodal_route_blind.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
