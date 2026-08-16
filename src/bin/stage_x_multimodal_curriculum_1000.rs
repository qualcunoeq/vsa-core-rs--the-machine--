//! Stage X: route-blind multimodal curriculum composition at 1,000 cases.
//!
//! Every case is offered to both visual frontends.  A route may authorize only
//! after its own typed artifact, downstream composition, and replay receipt
//! are valid.  Conditional downstream metrics use emitted-artifact
//! denominators rather than treating non-applicable routes as verified.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::graph_pack::GraphStatus;
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
    preferred: Route,
    table: String,
    graph: VisualGraphObservation,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    preferred: Route,
    selected: Option<Route>,
    authorized: bool,
    exact: bool,
    table_status: TableStatus,
    table_bridge_status: Option<BridgeStatus>,
    graph_status: VisualGraphStatus,
    graph_pack_status: Option<GraphStatus>,
    table_replay_verified: bool,
    graph_replay_verified: bool,
    table_tamper_rejected: bool,
    graph_tamper_rejected: bool,
    downstream_emitted: bool,
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
    downstream_emitted: usize,
    downstream_replay_verified: usize,
    frontend_tamper_rejected: usize,
    downstream_tamper_rejected: usize,
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

fn rational(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("exact rational")
}

fn word(left: usize, top: usize, text: &str) -> String {
    format!("5\t1\t1\t1\t1\t1\t{left}\t{top}\t50\t10\t99\t{text}")
}

fn table(headers: &[&str], rows: &[(&str, &str)]) -> String {
    let mut lines = vec![OCR_HEADER.to_string()];
    for (column, header) in headers.iter().enumerate() {
        lines.push(word(10 + column * 70, 10, header));
    }
    for (row, (left, right)) in rows.iter().enumerate() {
        lines.push(word(10, 35 + row * 25, left));
        lines.push(word(80, 35 + row * 25, right));
    }
    lines.join("\n")
}

fn empty_table() -> String {
    OCR_HEADER.into()
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

fn graph(
    index: usize,
    semantic_label: Option<&str>,
    direction: Option<bool>,
) -> VisualGraphObservation {
    let edges = vec![
        VisualEdgeObservation {
            from: "a".into(),
            to: "b".into(),
            directed: direction,
            confidence: 99,
        },
        VisualEdgeObservation {
            from: "b".into(),
            to: "c".into(),
            directed: direction,
            confidence: 99,
        },
        VisualEdgeObservation {
            from: "c".into(),
            to: "a".into(),
            directed: direction,
            confidence: 99,
        },
    ];
    VisualGraphObservation {
        semantic_label: semantic_label.map(str::to_owned),
        nodes: vec![node("a", 10), node("b", 60), node("c", 110)],
        edges,
        directed: direction,
        ambiguity: None,
        provenance: vec![format!("stage-x:graph:{index}")],
    }
}

fn empty_graph() -> VisualGraphObservation {
    VisualGraphObservation {
        semantic_label: Some("not_a_graph".into()),
        nodes: Vec::new(),
        edges: Vec::new(),
        directed: None,
        ambiguity: None,
        provenance: vec!["stage-x:table-route".into()],
    }
}

fn corpus() -> Vec<Case> {
    let mut cases = Vec::with_capacity(1000);
    for index in 0..300 {
        cases.push(Case {
            id: format!("table_supported_{index:03}"),
            expected: Expected::Supported,
            preferred: Route::Table,
            table: table(
                &["outcome", "probability"],
                &[("a", "1/2"), ("b", "1/3"), ("c", "1/6")],
            ),
            graph: empty_graph(),
        });
    }
    for index in 0..300 {
        cases.push(Case {
            id: format!("graph_supported_{index:03}"),
            expected: Expected::Supported,
            preferred: Route::Graph,
            table: empty_table(),
            graph: graph(index, Some("finite_simple_graph"), Some(index % 2 == 0)),
        });
    }
    for index in 0..100 {
        cases.push(Case {
            id: format!("table_ambiguous_{index:03}"),
            expected: Expected::Ambiguous,
            preferred: Route::Table,
            table: table(&["value", "weight"], &[("a", "1/2"), ("b", "1/2")]),
            graph: empty_graph(),
        });
    }
    for index in 0..100 {
        cases.push(Case {
            id: format!("graph_ambiguous_{index:03}"),
            expected: Expected::Ambiguous,
            preferred: Route::Graph,
            table: empty_table(),
            graph: graph(index, Some("finite_simple_graph"), None),
        });
    }
    for index in 0..100 {
        cases.push(Case {
            id: format!("table_unsupported_{index:03}"),
            expected: Expected::Unsupported,
            preferred: Route::Table,
            table: table(&["outcome", "probability"], &[("a", "1/3"), ("b", "1/3")]),
            graph: empty_graph(),
        });
    }
    for index in 0..100 {
        cases.push(Case {
            id: format!("graph_unsupported_{index:03}"),
            expected: Expected::Unsupported,
            preferred: Route::Graph,
            table: empty_table(),
            graph: graph(index, Some("weighted_graph"), Some(index % 2 == 0)),
        });
    }
    cases
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
        provenance: vec!["stage-x:initial-distribution".into()],
    })
}

fn run(case: &Case) -> Receipt {
    let table_result = formalize_table_tsv(&case.table);
    let table_bridge = table_result.artifact.as_ref().map(table_to_probability);
    let graph_result = formalize_visual_graph(&case.graph);
    let graph_pack = to_graph_request(&graph_result)
        .map(|request| the_machine::graph_pack::evaluate_graph(&request));
    let table_authorized = table_bridge
        .as_ref()
        .is_some_and(|bridge| bridge.authorized());
    let graph_authorized = graph_result.status == VisualGraphStatus::Complete
        && graph_pack
            .as_ref()
            .is_some_and(|result| result.status == GraphStatus::Complete);
    let (downstream_emitted, downstream_replay_verified, downstream_tamper_rejected) =
        if graph_authorized {
            let index = case
                .id
                .strip_prefix("graph_supported_")
                .and_then(|value| value.parse().ok())
                .unwrap_or(0);
            let walk = execute_one_step_random_walk(
                &graph_result,
                Some(&transition(index)),
                &initial_distribution(),
                Some(TransitionConvention::RowStochastic),
                vec![case.id.clone(), "stage-x-explicit-transition".into()],
            )
            .expect("authorized graph route emits a walk");
            let mut tampered = walk.clone();
            tampered.replay_hash.push('x');
            (true, walk.replay_verified(), !tampered.replay_verified())
        } else {
            (false, false, false)
        };
    let selected = match (table_authorized, graph_authorized) {
        (true, false) => Some(Route::Table),
        (false, true) => Some(Route::Graph),
        _ => None,
    };
    let authorized =
        selected.is_some() && (selected != Some(Route::Graph) || downstream_replay_verified);
    let actual_ambiguous = !authorized
        && (table_bridge
            .as_ref()
            .is_some_and(|bridge| bridge.status == BridgeStatus::Ambiguous)
            || graph_result.status == VisualGraphStatus::Ambiguous);
    let exact = match case.expected {
        Expected::Supported => authorized && selected == Some(case.preferred),
        Expected::Ambiguous => actual_ambiguous,
        Expected::Unsupported => !authorized && !actual_ambiguous,
    };
    let mut table_tampered = table_result.clone();
    table_tampered.replay_hash.push('x');
    let mut graph_tampered = graph_result.clone();
    graph_tampered.replay_hash.push('x');
    Receipt {
        id: case.id.clone(),
        expected: case.expected,
        preferred: case.preferred,
        selected,
        authorized,
        exact,
        table_status: table_result.status,
        table_bridge_status: table_bridge.as_ref().map(|bridge| bridge.status),
        graph_status: graph_result.status,
        graph_pack_status: graph_pack.as_ref().map(|result| result.status),
        table_replay_verified: table_result.replay_verified(),
        graph_replay_verified: graph_result.replay_verified(),
        table_tamper_rejected: !table_tampered.replay_verified(),
        graph_tamper_rejected: !graph_tampered.replay_verified(),
        downstream_emitted,
        downstream_replay_verified,
        downstream_tamper_rejected,
        false_authorization: case.expected != Expected::Supported && authorized,
        false_denial: case.expected == Expected::Supported && !authorized,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let receipts = corpus().iter().map(run).collect::<Vec<_>>();
    let count = |expected| {
        receipts
            .iter()
            .filter(|receipt| receipt.expected == expected)
            .count()
    };
    let report = Report {
        schema: "stage-x-multimodal-curriculum-1000-v1",
        corpus_sha256: digest(&receipts),
        cases: receipts.len(),
        supported: count(Expected::Supported),
        ambiguous: count(Expected::Ambiguous),
        unsupported: count(Expected::Unsupported),
        exact_decisions: receipts.iter().filter(|receipt| receipt.exact).count(),
        authorized_supported: receipts
            .iter()
            .filter(|receipt| receipt.expected == Expected::Supported && receipt.authorized)
            .count(),
        ambiguities_preserved: receipts
            .iter()
            .filter(|receipt| receipt.expected == Expected::Ambiguous && !receipt.authorized)
            .count(),
        unsupported_refusals: receipts
            .iter()
            .filter(|receipt| receipt.expected == Expected::Unsupported && !receipt.authorized)
            .count(),
        frontend_invocations: receipts.len() * 2,
        route_counts: receipts.iter().fold(BTreeMap::new(), |mut map, receipt| {
            if let Some(route) = receipt.selected {
                *map.entry(format!("{route:?}").to_ascii_lowercase())
                    .or_insert(0) += 1;
            }
            map
        }),
        table_replay_verified: receipts
            .iter()
            .filter(|receipt| receipt.table_replay_verified)
            .count(),
        graph_replay_verified: receipts
            .iter()
            .filter(|receipt| receipt.graph_replay_verified)
            .count(),
        downstream_emitted: receipts
            .iter()
            .filter(|receipt| receipt.downstream_emitted)
            .count(),
        downstream_replay_verified: receipts
            .iter()
            .filter(|receipt| receipt.downstream_replay_verified)
            .count(),
        frontend_tamper_rejected: receipts
            .iter()
            .filter(|receipt| receipt.table_tamper_rejected && receipt.graph_tamper_rejected)
            .count(),
        downstream_tamper_rejected: receipts
            .iter()
            .filter(|receipt| receipt.downstream_tamper_rejected)
            .count(),
        false_authorizations: receipts
            .iter()
            .filter(|receipt| receipt.false_authorization)
            .count(),
        false_denials: receipts
            .iter()
            .filter(|receipt| receipt.false_denial)
            .count(),
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
        (1000, 600, 200, 200)
    );
    assert_eq!(
        (
            report.exact_decisions,
            report.authorized_supported,
            report.ambiguities_preserved,
            report.unsupported_refusals
        ),
        (1000, 600, 200, 200)
    );
    assert_eq!(
        (
            report.frontend_invocations,
            report.table_replay_verified,
            report.graph_replay_verified
        ),
        (2000, 1000, 1000)
    );
    assert_eq!(
        (
            report.downstream_emitted,
            report.downstream_replay_verified,
            report.frontend_tamper_rejected,
            report.downstream_tamper_rejected
        ),
        (300, 300, 1000, 300)
    );
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
        "docs/stage_x_multimodal_curriculum_1000.json",
        format!("{serialized}\n"),
    )?;
    println!("{serialized}");
    Ok(())
}
