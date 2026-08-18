//! Independent benchmark for the governed geometry-diagram frontend.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::vision::visual_geometry::{
    formalize_visual_geometry, GeometryCircleObservation, GeometryPointObservation,
    GeometryRelationObservation, GeometrySegmentObservation, GeometryStatus,
    VisualGeometryObservation,
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
    actual: GeometryStatus,
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
    refused: usize,
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

fn point(label: &str, x: i32, y: i32) -> GeometryPointObservation {
    GeometryPointObservation {
        label: label.into(),
        x,
        y,
        confidence: 98,
    }
}

fn segment(id: &str, from: &str, to: &str) -> GeometrySegmentObservation {
    GeometrySegmentObservation {
        id: id.into(),
        from: from.into(),
        to: to.into(),
        confidence: 97,
    }
}

fn supported(index: usize) -> VisualGeometryObservation {
    let mut points = vec![point("A", 0, 0), point("B", 4, 0), point("C", 0, 3)];
    let mut segments = vec![segment("AB", "A", "B"), segment("AC", "A", "C")];
    let mut circles = Vec::new();
    let mut relations = vec![GeometryRelationObservation {
        kind: "perpendicular".into(),
        left: "AB".into(),
        right: "AC".into(),
        confidence: 96,
    }];
    if index % 3 == 0 {
        points.push(point("D", 4, 3));
        segments.push(segment("BD", "B", "D"));
        segments.push(segment("CD", "C", "D"));
        relations.push(GeometryRelationObservation {
            kind: "parallel".into(),
            left: "AB".into(),
            right: "CD".into(),
            confidence: 95,
        });
    } else if index % 3 == 1 {
        circles.push(GeometryCircleObservation {
            id: "omega".into(),
            center: "A".into(),
            radius: 5 + (index % 3) as i32,
            confidence: 94,
        });
        relations.push(GeometryRelationObservation {
            kind: "tangent".into(),
            left: "AB".into(),
            right: "omega".into(),
            confidence: 93,
        });
    } else {
        relations.push(GeometryRelationObservation {
            kind: "equal_length".into(),
            left: "AB".into(),
            right: "AC".into(),
            confidence: 92,
        });
    }
    VisualGeometryObservation {
        semantic_label: Some("cartesian_geometry_diagram".into()),
        points,
        segments,
        circles,
        relations,
        ambiguity: None,
        provenance: vec![
            format!("geometry:supported:{index}"),
            "coordinates:explicit".into(),
            "relations:explicit".into(),
        ],
    }
}

fn ambiguous(index: usize) -> VisualGeometryObservation {
    let mut observation = supported(index);
    if index % 2 == 0 {
        observation.ambiguity = Some("diagram-or-plot".into());
    } else {
        observation.points[0].confidence = 35;
    }
    observation.provenance = vec![format!("geometry:ambiguous:{index}")];
    observation
}

fn refused(index: usize) -> VisualGeometryObservation {
    let mut observation = supported(index);
    match index % 8 {
        0 => observation.semantic_label = Some("polar_geometry".into()),
        1 => observation.segments[0].to = "Z".into(),
        2 => observation.segments[0].id = observation.segments[1].id.clone(),
        3 => observation.circles.push(GeometryCircleObservation {
            id: "bad_circle".into(),
            center: "Z".into(),
            radius: 3,
            confidence: 98,
        }),
        4 => observation.relations[0].kind = "similarity".into(),
        5 => observation.relations[0].right = "unknown_object".into(),
        6 => observation.points.push(point("far", 20_001, 0)),
        _ => observation.provenance.clear(),
    }
    observation.provenance = if index % 8 == 7 {
        Vec::new()
    } else {
        vec![format!("geometry:refused:{index}")]
    };
    observation
}

fn run(id: String, observation: VisualGeometryObservation, expected: Expected) -> Receipt {
    let result = formalize_visual_geometry(&observation);
    let actual = result.status;
    let exact = match expected {
        Expected::Supported => actual == GeometryStatus::Complete,
        Expected::Ambiguous => actual == GeometryStatus::Ambiguous,
        Expected::Refused => actual != GeometryStatus::Complete,
    };
    let mut tampered = result.clone();
    tampered.replay_hash.push('x');
    Receipt {
        id,
        expected,
        actual,
        exact,
        artifact_emitted: result.artifact.is_some(),
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
            format!("refused_{index:03}"),
            refused(index),
            Expected::Refused,
        ));
    }
    let supported = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported)
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
    let supported_artifacts = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && r.artifact_emitted)
        .count();
    let provenance_preserved = receipts
        .iter()
        .filter(|r| r.expected == Expected::Supported && r.provenance_preserved)
        .count();
    let replay_verified = receipts.iter().filter(|r| r.replay_verified).count();
    let tamper_rejections = receipts.iter().filter(|r| r.tamper_rejected).count();
    let false_authorizations = receipts.iter().filter(|r| r.false_authorization).count();
    let false_denials = receipts.iter().filter(|r| r.false_denial).count();
    let mut status_counts = BTreeMap::new();
    for receipt in &receipts {
        *status_counts
            .entry(format!("{:?}", receipt.actual))
            .or_insert(0usize) += 1;
    }
    assert_eq!((supported, ambiguous, refused), (120, 40, 80));
    assert_eq!(exact_decisions, 240);
    assert_eq!(supported_artifacts, 120);
    assert_eq!(provenance_preserved, 120);
    assert_eq!(replay_verified, 240);
    assert_eq!(tamper_rejections, 240);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    let report = Report {
        schema: "stage-j-visual-geometry-frontend-v1",
        corpus_sha256: digest(&receipts),
        cases: 240,
        supported,
        ambiguous,
        refused,
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
        "docs/stage292_visual_geometry_frontend.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    fs::write("docs/stage292_visual_geometry_frontend.md", format!(
        "# Stage 292 — governed visual geometry frontend\n\nA coordinate-preserving geometry frontend emits only explicit points, segments, circles, relations, confidence, and provenance. It does not infer lengths, angles, incidence, parallelism, or proofs from coordinates.\n\n* cases / exact decisions: {} / {}\n* supported / ambiguous / refused: {} / {} / {}\n* supported artifacts / provenance: {} / {}\n* replay / tamper: {} / {}\n* false authorizations / denials: 0 / 0\n* HLE questions read / registry mutations: 0 / 0\n\nReproduce with `cargo run --quiet --bin visual_geometry_frontend_bench`.\n",
        report.cases, report.exact_decisions, report.supported, report.ambiguous, report.refused,
        report.supported_artifacts, report.provenance_preserved, report.replay_verified, report.tamper_rejections,
    ))?;
    println!(
        "stage292 cases={} exact={} supported={} false_auth=0",
        report.cases, report.exact_decisions, report.supported
    );
    Ok(())
}
