//! Route-blind composition of the governed visual frontends.
//!
//! Each case is offered to table, graph, plot, and geometry frontends without
//! a modality dispatcher.  The oracle knows the intended route only to score
//! the result; the router authorizes solely from complete replayable artifacts.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
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
use the_machine::vision::visual_table::formalize_table_tsv;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Table,
    Graph,
    Plot,
    Geometry,
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
    frontend_tamper_rejections: usize,
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
    frontend_tamper_rejections: usize,
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
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn table_tsv(index: usize) -> String {
    format!(
        "5\t1\t1\t1\t1\t99\t10\t10\t20\t10\t99\tname\n5\t1\t1\t1\t1\t99\t60\t10\t20\t10\t99\tvalue_{index}\n5\t1\t1\t1\t1\t99\t10\t40\t20\t10\t99\ta\n5\t1\t1\t1\t1\t99\t60\t40\t20\t10\t99\t{}",
        index + 1
    )
}

fn graph_observation(index: usize) -> VisualGraphObservation {
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
                directed: Some(index % 2 == 0),
                confidence: 98,
            },
            VisualEdgeObservation {
                from: "B".into(),
                to: "C".into(),
                directed: Some(index % 2 == 0),
                confidence: 98,
            },
        ],
        directed: Some(index % 2 == 0),
        ambiguity: None,
        provenance: vec![format!("visual-route:graph:{index}")],
    }
}

fn plot_observation(index: usize) -> VisualPlotObservation {
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
        provenance: vec![format!("visual-route:plot:{index}")],
    }
}

fn geometry_observation(index: usize) -> VisualGeometryObservation {
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
        provenance: vec![format!("visual-route:geometry:{index}")],
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

fn cases() -> Vec<Case> {
    let mut cases = Vec::with_capacity(240);
    for index in 0..30 {
        cases.push(Case {
            expected: Expected::Table,
            table: Some(table_tsv(index)),
            graph: None,
            plot: None,
            geometry: None,
        });
    }
    for index in 0..30 {
        cases.push(Case {
            expected: Expected::Graph,
            table: None,
            graph: Some(graph_observation(index)),
            plot: None,
            geometry: None,
        });
    }
    for index in 0..30 {
        cases.push(Case {
            expected: Expected::Plot,
            table: None,
            graph: None,
            plot: Some(plot_observation(index)),
            geometry: None,
        });
    }
    for index in 0..30 {
        cases.push(Case {
            expected: Expected::Geometry,
            table: None,
            graph: None,
            plot: None,
            geometry: Some(geometry_observation(index)),
        });
    }
    for index in 0..10 {
        let mut plot = plot_observation(index);
        plot.kind = None;
        cases.push(Case {
            expected: Expected::Ambiguous,
            table: None,
            graph: None,
            plot: Some(plot),
            geometry: None,
        });
    }
    for index in 10..20 {
        let mut geometry = geometry_observation(index);
        geometry.ambiguity = Some("diagram-or-plot".into());
        cases.push(Case {
            expected: Expected::Ambiguous,
            table: None,
            graph: None,
            plot: None,
            geometry: Some(geometry),
        });
    }
    for index in 0..20 {
        let mut graph = graph_observation(index);
        graph.directed = None;
        cases.push(Case {
            expected: Expected::Ambiguous,
            table: None,
            graph: Some(graph),
            plot: None,
            geometry: None,
        });
    }
    for index in 0..20 {
        let mut table = table_tsv(index);
        table.push_str("\n5\t1\t1\t1\t1\t99\t10\t70\t20\t10\t99\textra");
        cases.push(Case {
            expected: Expected::Refused,
            table: Some(table),
            graph: None,
            plot: None,
            geometry: None,
        });
    }
    for index in 0..20 {
        let mut graph = graph_observation(index);
        graph.edges[0].to = "unknown".into();
        cases.push(Case {
            expected: Expected::Refused,
            table: None,
            graph: Some(graph),
            plot: None,
            geometry: None,
        });
    }
    for index in 0..20 {
        let mut plot = plot_observation(index);
        plot.semantic_label = Some("polar_plot".into());
        cases.push(Case {
            expected: Expected::Refused,
            table: None,
            graph: None,
            plot: Some(plot),
            geometry: None,
        });
    }
    for index in 0..20 {
        let mut geometry = geometry_observation(index);
        geometry.relations[0].kind = "similarity".into();
        cases.push(Case {
            expected: Expected::Refused,
            table: None,
            graph: None,
            plot: None,
            geometry: Some(geometry),
        });
    }
    cases
}

fn run(index: usize, case: Case) -> Receipt {
    let table = formalize_table_tsv(case.table.as_deref().unwrap_or(""));
    let graph = formalize_visual_graph(&case.graph.clone().unwrap_or_else(empty_graph));
    let plot = formalize_visual_plot(&case.plot.clone().unwrap_or_else(empty_plot));
    let geometry = formalize_visual_geometry(&case.geometry.clone().unwrap_or_else(empty_geometry));
    let mut frontend_replays = 0;
    let mut frontend_tamper_rejections = 0;
    let mut table_tampered = table.clone();
    table_tampered.replay_hash.push('x');
    let mut graph_tampered = graph.clone();
    graph_tampered.replay_hash.push('x');
    let mut plot_tampered = plot.clone();
    plot_tampered.replay_hash.push('x');
    let mut geometry_tampered = geometry.clone();
    geometry_tampered.replay_hash.push('x');
    let tamper_rejections = [
        !table_tampered.replay_verified(),
        !graph_tampered.replay_verified(),
        !plot_tampered.replay_verified(),
        !geometry_tampered.replay_verified(),
    ];
    let mut authorized = Vec::new();
    for (route, tamper_rejected) in [
        (
            "table",
            table.replay_verified(),
            table.status == the_machine::vision::visual_table::TableStatus::Complete,
        ),
        ("graph", graph.replay_verified(), graph.authorized()),
        ("plot", plot.replay_verified(), plot.authorized()),
        (
            "geometry",
            geometry.replay_verified(),
            geometry.authorized(),
        ),
    ]
    .into_iter()
    .zip(tamper_rejections)
    {
        let (name, replay, auth) = route;
        if replay {
            frontend_replays += 1;
        }
        if auth {
            authorized.push(name);
        }
        if tamper_rejected {
            frontend_tamper_rejections += 1;
        }
    }
    let selected_route = (authorized.len() == 1).then(|| authorized[0].to_string());
    let expected_route = match case.expected {
        Expected::Table => Some("table"),
        Expected::Graph => Some("graph"),
        Expected::Plot => Some("plot"),
        Expected::Geometry => Some("geometry"),
        _ => None,
    };
    let exact = match expected_route {
        Some(expected) => selected_route.as_deref() == Some(expected),
        None => authorized.is_empty(),
    };
    let route_leakage = authorized.len() > 1;
    Receipt {
        id: format!("visual_{index:03}"),
        expected: case.expected,
        complete_routes: authorized.len(),
        selected_route,
        exact,
        route_leakage,
        frontend_replays,
        frontend_tamper_rejections,
        false_authorization: case.expected != Expected::Table
            && case.expected != Expected::Graph
            && case.expected != Expected::Plot
            && case.expected != Expected::Geometry
            && !authorized.is_empty(),
        false_denial: expected_route.is_some() && exact == false && authorized.is_empty(),
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
                Expected::Table | Expected::Graph | Expected::Plot | Expected::Geometry
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
    let frontend_tamper_rejections: usize =
        receipts.iter().map(|r| r.frontend_tamper_rejections).sum();
    let route_leakage = receipts.iter().filter(|r| r.route_leakage).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!(exact_decisions, 240);
    assert_eq!(authorized_routes, 120);
    assert_eq!(frontend_replays, 960);
    assert_eq!(frontend_tamper_rejections, 960);
    assert_eq!(route_leakage, 0);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage-j-multimodal-visual-route-blind-v1",
        corpus_sha256: digest(&receipts),
        cases: 240,
        supported,
        ambiguous,
        refused,
        exact_decisions,
        authorized_routes,
        frontend_replays,
        frontend_tamper_rejections,
        route_leakage,
        false_authorizations,
        false_denials,
        hle_questions_read: 0,
        registry_mutations: 0,
        receipts,
    };
    fs::write(
        "docs/stage293_multimodal_visual_route_blind.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    fs::write("docs/stage293_multimodal_visual_route_blind.md", format!("# Stage 293 — route-blind multimodal visual composition\n\nEvery case is offered to table, graph, plot, and geometry frontends without a modality dispatcher. Authorization requires exactly one complete replayable route.\n\n* cases / exact decisions: {} / {}\n* supported / ambiguous / refused: {} / {} / {}\n* authorized routes: {}\n* frontend replay / tamper: {} / {}\n* route leakage: {}\n* false authorizations / denials: 0 / 0\n* HLE questions read / registry mutations: 0 / 0\n\nReproduce with `cargo run --quiet --bin stage293_multimodal_visual_route_blind`.\n", report.cases, report.exact_decisions, report.supported, report.ambiguous, report.refused, report.authorized_routes, report.frontend_replays, report.frontend_tamper_rejections, report.route_leakage))?;
    println!(
        "stage293 cases={} exact={} authorized={} leakage=0",
        report.cases, report.exact_decisions, report.authorized_routes
    );
    Ok(())
}
