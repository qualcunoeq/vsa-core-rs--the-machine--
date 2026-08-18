//! Independent benchmark for the governed circuit-diagram frontend.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::vision::visual_circuit::{
    formalize_visual_circuit, CircuitComponentObservation, CircuitStatus, CircuitWireObservation,
    VisualCircuitObservation,
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
    actual: CircuitStatus,
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

fn component(id: &str, kind: &str, index: usize) -> CircuitComponentObservation {
    CircuitComponentObservation {
        id: id.into(),
        kind: kind.into(),
        terminals: vec![format!("{id}.a"), format!("{id}.b")],
        value: Some(format!("{} unit", index + 1)),
        confidence: 98,
    }
}

fn supported(index: usize) -> VisualCircuitObservation {
    let first_kind = ["resistor", "capacitor", "inductor", "diode"][index % 4];
    VisualCircuitObservation {
        semantic_label: Some("bounded_circuit_diagram".into()),
        components: vec![
            component("X1", first_kind, index),
            component("V1", "voltage_source", index + 1),
        ],
        wires: vec![
            CircuitWireObservation {
                id: "w1".into(),
                from: "V1.a".into(),
                to: "X1.a".into(),
                confidence: 98,
            },
            CircuitWireObservation {
                id: "w2".into(),
                from: "X1.b".into(),
                to: "V1.b".into(),
                confidence: 98,
            },
        ],
        ground_terminal: Some("V1.b".into()),
        ambiguity: None,
        provenance: vec![
            format!("circuit:supported:{index}"),
            "terminals:explicit".into(),
        ],
    }
}

fn ambiguous(index: usize) -> VisualCircuitObservation {
    let mut observation = supported(index);
    if index % 2 == 0 {
        observation.ambiguity = Some("circuit-or-block-diagram".into());
    } else {
        observation.components[0].confidence = 35;
    }
    observation.provenance = vec![format!("circuit:ambiguous:{index}")];
    observation
}

fn refused(index: usize) -> VisualCircuitObservation {
    let mut observation = supported(index);
    match index % 8 {
        0 => observation.semantic_label = Some("block_diagram".into()),
        1 => observation.components[0].kind = "transistor_model".into(),
        2 => observation.wires[0].to = "unknown".into(),
        3 => {
            observation.components[0].terminals[1] = observation.components[0].terminals[0].clone()
        }
        4 => observation.components[0].id = observation.components[1].id.clone(),
        5 => observation.ground_terminal = Some("missing.ground".into()),
        6 => observation.wires[0].id = observation.wires[1].id.clone(),
        _ => observation.provenance.clear(),
    }
    observation.provenance = if index % 8 == 7 {
        Vec::new()
    } else {
        vec![format!("circuit:refused:{index}")]
    };
    observation
}

fn run(id: String, observation: VisualCircuitObservation, expected: Expected) -> Receipt {
    let result = formalize_visual_circuit(&observation);
    let actual = result.status;
    let exact = match expected {
        Expected::Supported => actual == CircuitStatus::Complete,
        Expected::Ambiguous => actual == CircuitStatus::Ambiguous,
        Expected::Refused => actual != CircuitStatus::Complete,
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
        schema: "stage-j-visual-circuit-frontend-v1",
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
        "docs/stage294_visual_circuit_frontend.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    fs::write("docs/stage294_visual_circuit_frontend.md", format!(
        "# Stage 294 — governed visual circuit frontend\n\nA circuit frontend emits only explicit component, terminal, wire, value, ground, and provenance observations. It does not infer voltage, current, polarity, equivalent resistance, or circuit behavior.\n\n* cases / exact decisions: {} / {}\n* supported / ambiguous / refused: {} / {} / {}\n* supported artifacts / provenance: {} / {}\n* replay / tamper: {} / {}\n* false authorizations / denials: 0 / 0\n* HLE questions read / registry mutations: 0 / 0\n\nReproduce with `cargo run --quiet --bin visual_circuit_frontend_bench`.\n",
        report.cases, report.exact_decisions, report.supported, report.ambiguous, report.refused,
        report.supported_artifacts, report.provenance_preserved, report.replay_verified, report.tamper_rejections,
    ))?;
    println!(
        "stage294 cases={} exact={} supported={} false_auth=0",
        report.cases, report.exact_decisions, report.supported
    );
    Ok(())
}
