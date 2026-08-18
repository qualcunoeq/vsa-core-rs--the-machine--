//! Independent benchmark for the governed Cartesian-plot visual frontend.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::vision::visual_plot::{
    formalize_visual_plot, PlotAxisObservation, PlotPointObservation, PlotSegmentObservation,
    PlotStatus, VisualPlotObservation,
};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Serialize)]
struct Receipt {
    id: String,
    expected: Expected,
    actual: PlotStatus,
    artifact_emitted: bool,
    exact: bool,
    provenance_preserved: bool,
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
    unsupported: usize,
    exact_decisions: usize,
    supported_artifacts: usize,
    provenance_preserved: usize,
    replay_verified: usize,
    tamper_rejections: usize,
    false_authorizations: usize,
    false_denials: usize,
    status_counts: BTreeMap<String, usize>,
    hle_questions_read: usize,
    registry_mutations: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn axis(label: &str, minimum: i32, maximum: i32) -> PlotAxisObservation {
    PlotAxisObservation {
        label: label.into(),
        minimum,
        maximum,
        confidence: 99,
    }
}

fn supported(index: usize) -> VisualPlotObservation {
    let line = index % 2 == 0;
    let points = vec![
        PlotPointObservation {
            label: Some("p0".into()),
            x: 1,
            y: 2 + (index % 3) as i32,
            confidence: 98,
        },
        PlotPointObservation {
            label: Some("p1".into()),
            x: 4,
            y: 5 + (index % 4) as i32,
            confidence: 97,
        },
        PlotPointObservation {
            label: Some("p2".into()),
            x: 8,
            y: 7 + (index % 2) as i32,
            confidence: 96,
        },
    ];
    VisualPlotObservation {
        semantic_label: Some("cartesian_plot".into()),
        x_axis: Some(axis("time", 0, 10)),
        y_axis: Some(axis("value", 0, 10)),
        kind: Some(if line { "line" } else { "scatter" }.into()),
        units: if index % 3 == 0 {
            Some(("s".into(), "m".into()))
        } else {
            None
        },
        points,
        segments: if line {
            vec![
                PlotSegmentObservation {
                    from: 0,
                    to: 1,
                    confidence: 98,
                },
                PlotSegmentObservation {
                    from: 1,
                    to: 2,
                    confidence: 97,
                },
            ]
        } else {
            Vec::new()
        },
        ambiguity: None,
        provenance: vec![
            format!("plot:supported:{index}"),
            "coordinates:explicit".into(),
        ],
    }
}

fn ambiguous(index: usize) -> VisualPlotObservation {
    let mut observation = supported(index);
    if index % 2 == 0 {
        observation.kind = None;
    } else {
        observation.ambiguity = Some("scatter-or-line".into());
    }
    observation.provenance = vec![format!("plot:ambiguous:{index}")];
    observation
}

fn unsupported(index: usize) -> VisualPlotObservation {
    let mut observation = supported(index);
    match index % 6 {
        0 => observation.semantic_label = Some("polar_plot".into()),
        1 => observation.points[0].x = 99,
        2 => observation.units = Some((String::new(), "m".into())),
        3 => {
            observation.kind = Some("line".into());
            if let Some(segment) = observation.segments.first_mut() {
                segment.to = 99;
            } else {
                observation.segments = vec![PlotSegmentObservation {
                    from: 0,
                    to: 99,
                    confidence: 99,
                }];
            }
        }
        4 => observation.segments.clear(),
        _ => observation.provenance.clear(),
    }
    if index % 6 == 4 {
        observation.kind = Some("line".into());
    }
    observation.provenance = if index % 6 == 5 {
        Vec::new()
    } else {
        vec![format!("plot:unsupported:{index}")]
    };
    observation
}

fn run(id: String, observation: VisualPlotObservation, expected: Expected) -> Receipt {
    let result = formalize_visual_plot(&observation);
    let actual = result.status;
    let exact = match expected {
        Expected::Supported => actual == PlotStatus::Complete,
        Expected::Ambiguous => actual == PlotStatus::Ambiguous,
        Expected::Unsupported => matches!(
            actual,
            PlotStatus::Unsupported | PlotStatus::Invalid | PlotStatus::Missing
        ),
    };
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    Receipt {
        id,
        expected,
        actual,
        artifact_emitted: result.artifact.is_some(),
        exact,
        provenance_preserved: result
            .artifact
            .as_ref()
            .is_some_and(|artifact| !artifact.provenance.is_empty()),
        replay_verified: result.replay_verified(),
        tamper_rejected: !tampered.replay_verified(),
        false_authorization: expected != Expected::Supported && result.authorized(),
        false_denial: expected == Expected::Supported && !result.authorized(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut receipts = Vec::with_capacity(240);
    for index in 0..120 {
        receipts.push(run(
            format!("supported_{index:03}"),
            supported(index),
            Expected::Supported,
        ));
    }
    for index in 0..40 {
        receipts.push(run(
            format!("ambiguous_{index:03}"),
            ambiguous(index),
            Expected::Ambiguous,
        ));
    }
    for index in 0..80 {
        receipts.push(run(
            format!("unsupported_{index:03}"),
            unsupported(index),
            Expected::Unsupported,
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
    let unsupported = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Unsupported)
        .count();
    let exact_decisions = receipts.iter().filter(|receipt| receipt.exact).count();
    let supported_artifacts = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Supported && receipt.artifact_emitted)
        .count();
    let provenance_preserved = receipts
        .iter()
        .filter(|receipt| receipt.expected == Expected::Supported && receipt.provenance_preserved)
        .count();
    let replay_verified = receipts
        .iter()
        .filter(|receipt| receipt.replay_verified)
        .count();
    let tamper_rejections = receipts
        .iter()
        .filter(|receipt| receipt.tamper_rejected)
        .count();
    let false_authorizations = receipts
        .iter()
        .filter(|receipt| receipt.false_authorization)
        .count();
    let false_denials = receipts
        .iter()
        .filter(|receipt| receipt.false_denial)
        .count();
    let mut status_counts = BTreeMap::new();
    for receipt in &receipts {
        *status_counts
            .entry(format!("{:?}", receipt.actual))
            .or_insert(0usize) += 1;
    }
    assert_eq!((supported, ambiguous, unsupported), (120, 40, 80));
    assert_eq!(exact_decisions, 240);
    assert_eq!(supported_artifacts, 120);
    assert_eq!(provenance_preserved, 120);
    assert_eq!(replay_verified, 240);
    assert_eq!(tamper_rejections, 240);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage-j-visual-plot-frontend-v1",
        corpus_sha256: digest(&receipts),
        cases: 240,
        supported,
        ambiguous,
        unsupported,
        exact_decisions,
        supported_artifacts,
        provenance_preserved,
        replay_verified,
        tamper_rejections,
        false_authorizations,
        false_denials,
        status_counts,
        hle_questions_read: 0,
        registry_mutations: 0,
        receipts,
    };
    fs::write(
        "docs/stage291_visual_plot_frontend.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    fs::write(
        "docs/stage291_visual_plot_frontend.md",
        format!(
            "# Stage 291 — governed visual plot frontend\n\nA coordinate-preserving Cartesian-plot frontend emits only explicit axis, point, segment, kind, confidence, unit, and provenance artifacts. It does not infer functions, interpolation, monotonicity, or downstream answers.\n\n* cases / exact decisions: {} / {}\n* supported / ambiguous / unsupported: {} / {} / {}\n* supported artifacts / provenance: {} / {}\n* replay / tamper: {} / {}\n* false authorizations / denials: 0 / 0\n* HLE questions read / registry mutations: 0 / 0\n\nReproduce with `cargo run --quiet --bin visual_plot_frontend_bench`.\n",
            report.cases,
            report.exact_decisions,
            report.supported,
            report.ambiguous,
            report.unsupported,
            report.supported_artifacts,
            report.provenance_preserved,
            report.replay_verified,
            report.tamper_rejections,
        ),
    )?;
    println!(
        "stage291 cases={} exact={} supported={} false_auth=0",
        report.cases, report.exact_decisions, report.supported
    );
    Ok(())
}
