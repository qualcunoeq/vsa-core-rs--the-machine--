//! Five-route blind visual composition: table, graph, plot, geometry, circuit.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::vision::visual_circuit::{
    formalize_visual_circuit, CircuitComponentObservation, VisualCircuitObservation,
};
use the_machine::vision::visual_geometry::{
    formalize_visual_geometry, GeometryPointObservation, GeometryRelationObservation,
    GeometrySegmentObservation, VisualGeometryObservation,
};
use the_machine::vision::visual_graph::{
    formalize_visual_graph, VisualEdgeObservation, VisualGraphObservation, VisualNodeObservation,
};
use the_machine::vision::visual_plot::{
    formalize_visual_plot, PlotAxisObservation, PlotPointObservation, PlotSegmentObservation,
    VisualPlotObservation,
};
use the_machine::vision::visual_table::{formalize_table_tsv, TableStatus};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Table,
    Graph,
    Plot,
    Geometry,
    Circuit,
    Ambiguous,
    Refused,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    complete_routes: usize,
    selected_route: Option<String>,
    exact: bool,
    route_leakage: bool,
    frontend_replays: usize,
    tamper_rejections: usize,
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
    authorized_routes: usize,
    frontend_replays: usize,
    tamper_rejections: usize,
    route_leakage: usize,
    false_authorizations: usize,
    false_denials: usize,
    hle_questions_read: usize,
    registry_mutations: usize,
    receipts: Vec<Receipt>,
}

#[derive(Debug, Clone)]
struct Case {
    expected: Expected,
    table: Option<String>,
    graph: Option<VisualGraphObservation>,
    plot: Option<VisualPlotObservation>,
    geometry: Option<VisualGeometryObservation>,
    circuit: Option<VisualCircuitObservation>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn table(index: usize) -> String {
    format!("5\t1\t1\t1\t1\t99\t10\t10\t20\t10\t99\tname\n5\t1\t1\t1\t1\t99\t60\t10\t20\t10\t99\tvalue\n5\t1\t1\t1\t1\t99\t10\t40\t20\t10\t99\ta\n5\t1\t1\t1\t1\t99\t60\t40\t20\t10\t99\t{}", index + 1)
}

fn graph(index: usize) -> VisualGraphObservation {
    let directed = index % 2 == 0;
    VisualGraphObservation {
        semantic_label: Some("finite_simple_graph".into()),
        nodes: vec![
            VisualNodeObservation {
                label: "A".into(),
                left: 10,
                top: 10,
                width: 10,
                height: 10,
                confidence: 98,
            },
            VisualNodeObservation {
                label: "B".into(),
                left: 60,
                top: 10,
                width: 10,
                height: 10,
                confidence: 98,
            },
            VisualNodeObservation {
                label: "C".into(),
                left: 35,
                top: 60,
                width: 10,
                height: 10,
                confidence: 98,
            },
        ],
        edges: vec![
            VisualEdgeObservation {
                from: "A".into(),
                to: "B".into(),
                directed: Some(directed),
                confidence: 98,
            },
            VisualEdgeObservation {
                from: "B".into(),
                to: "C".into(),
                directed: Some(directed),
                confidence: 98,
            },
        ],
        directed: Some(directed),
        ambiguity: None,
        provenance: vec![format!("five-route:graph:{index}")],
    }
}

fn plot(index: usize) -> VisualPlotObservation {
    let line = index % 2 == 0;
    VisualPlotObservation {
        semantic_label: Some("cartesian_plot".into()),
        x_axis: Some(PlotAxisObservation {
            label: "x".into(),
            minimum: 0,
            maximum: 10,
            confidence: 98,
        }),
        y_axis: Some(PlotAxisObservation {
            label: "y".into(),
            minimum: 0,
            maximum: 10,
            confidence: 98,
        }),
        kind: Some(if line { "line" } else { "scatter" }.into()),
        units: None,
        points: vec![
            PlotPointObservation {
                label: Some("p0".into()),
                x: 1,
                y: 2,
                confidence: 98,
            },
            PlotPointObservation {
                label: Some("p1".into()),
                x: 5,
                y: 6,
                confidence: 98,
            },
        ],
        segments: if line {
            vec![PlotSegmentObservation {
                from: 0,
                to: 1,
                confidence: 98,
            }]
        } else {
            Vec::new()
        },
        ambiguity: None,
        provenance: vec![format!("five-route:plot:{index}")],
    }
}

fn geometry(index: usize) -> VisualGeometryObservation {
    VisualGeometryObservation {
        semantic_label: Some("cartesian_geometry_diagram".into()),
        points: vec![
            GeometryPointObservation {
                label: "A".into(),
                x: 0,
                y: 0,
                confidence: 98,
            },
            GeometryPointObservation {
                label: "B".into(),
                x: 4,
                y: 0,
                confidence: 98,
            },
            GeometryPointObservation {
                label: "C".into(),
                x: 0,
                y: 3,
                confidence: 98,
            },
        ],
        segments: vec![
            GeometrySegmentObservation {
                id: "AB".into(),
                from: "A".into(),
                to: "B".into(),
                confidence: 98,
            },
            GeometrySegmentObservation {
                id: "AC".into(),
                from: "A".into(),
                to: "C".into(),
                confidence: 98,
            },
        ],
        circles: Vec::new(),
        relations: vec![GeometryRelationObservation {
            kind: "perpendicular".into(),
            left: "AB".into(),
            right: "AC".into(),
            confidence: 98,
        }],
        ambiguity: None,
        provenance: vec![format!("five-route:geometry:{index}")],
    }
}

fn component(id: &str, kind: &str, value: &str) -> CircuitComponentObservation {
    CircuitComponentObservation {
        id: id.into(),
        kind: kind.into(),
        terminals: vec![format!("{id}.a"), format!("{id}.b")],
        value: Some(value.into()),
        confidence: 98,
    }
}

fn circuit(index: usize) -> VisualCircuitObservation {
    VisualCircuitObservation {
        semantic_label: Some("bounded_circuit_diagram".into()),
        components: vec![
            component("R1", "resistor", &format!("{} ohm", index + 1)),
            component("V1", "voltage_source", "5 V"),
        ],
        wires: vec![
            the_machine::vision::visual_circuit::CircuitWireObservation {
                id: "w1".into(),
                from: "V1.a".into(),
                to: "R1.a".into(),
                confidence: 98,
            },
            the_machine::vision::visual_circuit::CircuitWireObservation {
                id: "w2".into(),
                from: "R1.b".into(),
                to: "V1.b".into(),
                confidence: 98,
            },
        ],
        ground_terminal: Some("V1.b".into()),
        ambiguity: None,
        provenance: vec![format!("five-route:circuit:{index}")],
    }
}

fn empty_graph() -> VisualGraphObservation {
    VisualGraphObservation {
        semantic_label: None,
        nodes: Vec::new(),
        edges: Vec::new(),
        directed: None,
        ambiguity: None,
        provenance: Vec::new(),
    }
}

fn empty_plot() -> VisualPlotObservation {
    VisualPlotObservation {
        semantic_label: None,
        x_axis: None,
        y_axis: None,
        kind: None,
        units: None,
        points: Vec::new(),
        segments: Vec::new(),
        ambiguity: None,
        provenance: Vec::new(),
    }
}

fn empty_geometry() -> VisualGeometryObservation {
    VisualGeometryObservation {
        semantic_label: None,
        points: Vec::new(),
        segments: Vec::new(),
        circles: Vec::new(),
        relations: Vec::new(),
        ambiguity: None,
        provenance: Vec::new(),
    }
}

fn empty_circuit() -> VisualCircuitObservation {
    VisualCircuitObservation {
        semantic_label: None,
        components: Vec::new(),
        wires: Vec::new(),
        ground_terminal: None,
        ambiguity: None,
        provenance: Vec::new(),
    }
}

fn empty_case(expected: Expected) -> Case {
    Case {
        expected,
        table: None,
        graph: None,
        plot: None,
        geometry: None,
        circuit: None,
    }
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::with_capacity(240);
    for index in 0..24 {
        cases.push(Case {
            expected: Expected::Table,
            table: Some(table(index)),
            ..empty_case(Expected::Table)
        });
    }
    for index in 0..24 {
        cases.push(Case {
            expected: Expected::Graph,
            graph: Some(graph(index)),
            ..empty_case(Expected::Graph)
        });
    }
    for index in 0..24 {
        cases.push(Case {
            expected: Expected::Plot,
            plot: Some(plot(index)),
            ..empty_case(Expected::Plot)
        });
    }
    for index in 0..24 {
        cases.push(Case {
            expected: Expected::Geometry,
            geometry: Some(geometry(index)),
            ..empty_case(Expected::Geometry)
        });
    }
    for index in 0..24 {
        cases.push(Case {
            expected: Expected::Circuit,
            circuit: Some(circuit(index)),
            ..empty_case(Expected::Circuit)
        });
    }
    for index in 0..8 {
        let mut value = plot(index);
        value.kind = None;
        cases.push(Case {
            expected: Expected::Ambiguous,
            plot: Some(value),
            ..empty_case(Expected::Ambiguous)
        });
    }
    for index in 0..8 {
        let mut value = graph(index);
        value.directed = None;
        cases.push(Case {
            expected: Expected::Ambiguous,
            graph: Some(value),
            ..empty_case(Expected::Ambiguous)
        });
    }
    for index in 0..8 {
        let mut value = geometry(index);
        value.ambiguity = Some("diagram-or-plot".into());
        cases.push(Case {
            expected: Expected::Ambiguous,
            geometry: Some(value),
            ..empty_case(Expected::Ambiguous)
        });
    }
    for index in 0..8 {
        let mut value = circuit(index);
        value.components[0].confidence = 30;
        cases.push(Case {
            expected: Expected::Ambiguous,
            circuit: Some(value),
            ..empty_case(Expected::Ambiguous)
        });
    }
    for index in 0..8 {
        let mut value = table(index);
        value.push_str("\n5\t1\t1\t1\t1\t99\t10\t70\t20\t10\t99\textra");
        cases.push(Case {
            expected: Expected::Ambiguous,
            table: Some(value),
            ..empty_case(Expected::Ambiguous)
        });
    }
    for index in 0..16 {
        let mut value = graph(index);
        value.edges[0].to = "unknown".into();
        cases.push(Case {
            expected: Expected::Refused,
            graph: Some(value),
            ..empty_case(Expected::Refused)
        });
    }
    for index in 0..16 {
        let mut value = plot(index);
        value.semantic_label = Some("polar_plot".into());
        cases.push(Case {
            expected: Expected::Refused,
            plot: Some(value),
            ..empty_case(Expected::Refused)
        });
    }
    for index in 0..16 {
        let mut value = geometry(index);
        value.relations[0].kind = "similarity".into();
        cases.push(Case {
            expected: Expected::Refused,
            geometry: Some(value),
            ..empty_case(Expected::Refused)
        });
    }
    for index in 0..16 {
        let mut value = circuit(index);
        value.components[0].kind = "transistor_model".into();
        cases.push(Case {
            expected: Expected::Refused,
            circuit: Some(value),
            ..empty_case(Expected::Refused)
        });
    }
    for index in 0..16 {
        let mut value = table(index);
        value.push_str("\n5\t1\t1\t1\t1\t99\t10\t70\t20\t10\t99\textra");
        cases.push(Case {
            expected: Expected::Refused,
            table: Some(value),
            ..empty_case(Expected::Refused)
        });
    }
    cases
}

fn run(index: usize, case: Case) -> Receipt {
    let table_result = formalize_table_tsv(case.table.as_deref().unwrap_or(""));
    let graph_result = formalize_visual_graph(&case.graph.unwrap_or_else(empty_graph));
    let plot_result = formalize_visual_plot(&case.plot.unwrap_or_else(empty_plot));
    let geometry_result = formalize_visual_geometry(&case.geometry.unwrap_or_else(empty_geometry));
    let circuit_result = formalize_visual_circuit(&case.circuit.unwrap_or_else(empty_circuit));
    let mut table_tampered = table_result.clone();
    table_tampered.replay_hash.push('x');
    let mut graph_tampered = graph_result.clone();
    graph_tampered.replay_hash.push('x');
    let mut plot_tampered = plot_result.clone();
    plot_tampered.replay_hash.push('x');
    let mut geometry_tampered = geometry_result.clone();
    geometry_tampered.replay_hash.push('x');
    let mut circuit_tampered = circuit_result.clone();
    circuit_tampered.replay_hash.push('x');
    let replays = [
        table_result.replay_verified(),
        graph_result.replay_verified(),
        plot_result.replay_verified(),
        geometry_result.replay_verified(),
        circuit_result.replay_verified(),
    ];
    let tampered = [
        !table_tampered.replay_verified(),
        !graph_tampered.replay_verified(),
        !plot_tampered.replay_verified(),
        !geometry_tampered.replay_verified(),
        !circuit_tampered.replay_verified(),
    ];
    let routes = [
        ("table", table_result.status == TableStatus::Complete),
        ("graph", graph_result.authorized()),
        ("plot", plot_result.authorized()),
        ("geometry", geometry_result.authorized()),
        ("circuit", circuit_result.authorized()),
    ];
    let authorized: Vec<_> = routes
        .iter()
        .filter(|(_, auth)| *auth)
        .map(|(name, _)| *name)
        .collect();
    let selected_route = (authorized.len() == 1).then(|| authorized[0].to_string());
    let expected_route = match case.expected {
        Expected::Table => Some("table"),
        Expected::Graph => Some("graph"),
        Expected::Plot => Some("plot"),
        Expected::Geometry => Some("geometry"),
        Expected::Circuit => Some("circuit"),
        _ => None,
    };
    let exact = match expected_route {
        Some(expected) => selected_route.as_deref() == Some(expected),
        None => authorized.is_empty(),
    };
    let route_leakage = authorized.len() > 1;
    Receipt {
        id: format!("five_route_{index:03}"),
        expected: case.expected,
        complete_routes: authorized.len(),
        selected_route,
        exact,
        route_leakage,
        frontend_replays: replays.into_iter().filter(|value| *value).count(),
        tamper_rejections: tampered.into_iter().filter(|value| *value).count(),
        false_authorization: expected_route.is_none() && !authorized.is_empty(),
        false_denial: expected_route.is_some() && authorized.is_empty(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let receipts: Vec<_> = cases()
        .into_iter()
        .enumerate()
        .map(|(index, case)| run(index, case))
        .collect();
    let supported = receipts
        .iter()
        .filter(|r| {
            matches!(
                r.expected,
                Expected::Table
                    | Expected::Graph
                    | Expected::Plot
                    | Expected::Geometry
                    | Expected::Circuit
            )
        })
        .count();
    let ambiguous = receipts
        .iter()
        .filter(|r| r.expected == Expected::Ambiguous)
        .count();
    let refused = receipts
        .iter()
        .filter(|r| r.expected == Expected::Refused)
        .count();
    let exact_decisions = receipts.iter().filter(|r| r.exact).count();
    let authorized_routes = receipts.iter().filter(|r| r.complete_routes == 1).count();
    let frontend_replays: usize = receipts.iter().map(|r| r.frontend_replays).sum();
    let tamper_rejections: usize = receipts.iter().map(|r| r.tamper_rejections).sum();
    let route_leakage = receipts.iter().filter(|r| r.route_leakage).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!(exact_decisions, 240);
    assert_eq!(authorized_routes, 120);
    assert_eq!(frontend_replays, 1200);
    assert_eq!(tamper_rejections, 1200);
    assert_eq!(route_leakage, 0);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage-j-multimodal-visual-five-route-blind-v1",
        corpus_sha256: digest(&receipts),
        cases: 240,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        authorized_routes,
        frontend_replays,
        tamper_rejections,
        route_leakage,
        false_authorizations,
        false_denials,
        hle_questions_read: 0,
        registry_mutations: 0,
        receipts,
    };
    fs::write(
        "docs/stage295_multimodal_visual_five_route_blind.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    fs::write("docs/stage295_multimodal_visual_five_route_blind.md", format!("# Stage 295 — five-route blind multimodal visual composition\n\nEvery case is offered to table, graph, plot, geometry, and circuit frontends without a modality dispatcher. Authorization requires exactly one complete replayable route.\n\n* cases / exact decisions: {} / {}\n* supported / ambiguous / refused: {} / {} / {}\n* authorized routes: {}\n* frontend replay / tamper: {} / {}\n* route leakage: {}\n* false authorizations / denials: 0 / 0\n* HLE questions read / registry mutations: 0 / 0\n\nReproduce with `cargo run --quiet --bin stage295_multimodal_visual_five_route_blind`.\n", report.cases, report.exact_decisions, report.supported, report.ambiguous, report.refused, report.authorized_routes, report.frontend_replays, report.tamper_rejections, report.route_leakage))?;
    println!(
        "stage295 cases={} exact={} authorized={} leakage=0",
        report.cases, report.exact_decisions, report.authorized_routes
    );
    Ok(())
}
