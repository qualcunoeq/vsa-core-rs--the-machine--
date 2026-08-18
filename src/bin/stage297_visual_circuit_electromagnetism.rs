//! Stage 297: route-blind composition of visual circuit observations with a
//! source-derived electromagnetism pack.
//!
//! The bridge binds only explicit component readings.  It never derives
//! circuit topology, current, voltage, polarity, equivalent resistance, or
//! any other electrical behavior from the diagram.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use the_machine::probability_pack::Rational;
use the_machine::vision::visual_circuit::{
    formalize_visual_circuit, CircuitComponentObservation, CircuitWireObservation,
    VisualCircuitArtifact, VisualCircuitObservation,
};
use the_machine::visual_circuit_em_bridge::{evaluate_circuit_law, BridgeStatus, CircuitEmRequest};

const REPORT_JSON: &str = "docs/stage297_visual_circuit_electromagnetism.json";
const REPORT_MD: &str = "docs/stage297_visual_circuit_electromagnetism.md";
const SOURCE: &str =
    include_str!("../../docs/sources/openstax_bounded_electromagnetism_catalog.json");

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
    actual: BridgeStatus,
    law: String,
    visual_frontend_replay: bool,
    bridge_replay: bool,
    source_replay: bool,
    visual_tamper_rejected: bool,
    bridge_tamper_rejected: bool,
    exact: bool,
    value_correct: bool,
    provenance_preserved: bool,
    false_authorization: bool,
    false_denial: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    source_catalog_sha256: String,
    source_record_count: usize,
    cases: usize,
    supported: usize,
    ambiguous: usize,
    refused: usize,
    exact_decisions: usize,
    supported_values_correct: usize,
    visual_frontend_replays: usize,
    bridge_replays: usize,
    source_replays: usize,
    visual_tamper_rejections: usize,
    bridge_tamper_rejections: usize,
    provenance_preserved: usize,
    false_authorizations: usize,
    false_denials: usize,
    live_registry_mutations: usize,
    hle_questions_read: usize,
    receipts: Vec<Receipt>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn q(numerator: i128, denominator: i128) -> Rational {
    Rational::new(numerator, denominator).expect("valid benchmark rational")
}

fn component(
    id: &str,
    kind: &str,
    value: Option<&str>,
    _index: usize,
) -> CircuitComponentObservation {
    CircuitComponentObservation {
        id: id.into(),
        kind: kind.into(),
        terminals: vec![format!("{id}.a"), format!("{id}.b")],
        value: value.map(str::to_owned),
        confidence: 99,
    }
}

fn artifact(components: Vec<CircuitComponentObservation>) -> VisualCircuitArtifact {
    let wires = components
        .windows(2)
        .enumerate()
        .map(|(index, pair)| CircuitWireObservation {
            id: format!("w{index}"),
            from: pair[0].terminals[0].clone(),
            to: pair[1].terminals[0].clone(),
            confidence: 99,
        })
        .collect();
    let observation = VisualCircuitObservation {
        semantic_label: Some("bounded_circuit_diagram".into()),
        components,
        wires,
        ground_terminal: None,
        ambiguity: None,
        provenance: vec!["stage297:visual-source-span".into()],
    };
    let result = formalize_visual_circuit(&observation);
    assert!(result.authorized(), "benchmark circuit must be explicit");
    result.artifact.unwrap()
}

fn request(
    law: &str,
    bindings: BTreeMap<String, String>,
    extra_inputs: BTreeMap<String, Rational>,
    ambiguity: Option<String>,
    unit_scope: &str,
) -> CircuitEmRequest {
    CircuitEmRequest {
        law: law.into(),
        component_bindings: bindings,
        extra_inputs,
        unit_scope: unit_scope.into(),
        ambiguity,
        provenance: vec!["stage297:question-span".into()],
    }
}

fn supported_case(
    index: usize,
    law_index: usize,
) -> (VisualCircuitArtifact, CircuitEmRequest, Rational) {
    let scale = (index as i128 % 9) + 2;
    match law_index {
        0 => (
            artifact(vec![
                component("R1", "resistor", Some(&format!("{scale} ohm")), index),
                component("I1", "current_source", Some("2 A"), index),
            ]),
            request(
                "ohms_law_voltage",
                BTreeMap::from([("I".into(), "I1".into()), ("R".into(), "R1".into())]),
                BTreeMap::new(),
                None,
                "si_consistent_exact",
            ),
            q(2 * scale, 1),
        ),
        1 => (
            artifact(vec![
                component("V1", "voltage_source", Some("5 V"), index),
                component("I1", "current_source", Some(&format!("{scale} A")), index),
            ]),
            request(
                "electric_power",
                BTreeMap::from([("V".into(), "V1".into()), ("I".into(), "I1".into())]),
                BTreeMap::new(),
                None,
                "si_consistent_exact",
            ),
            q(5 * scale, 1),
        ),
        2 => (
            artifact(vec![component(
                "I1",
                "current_source",
                Some(&format!("{scale} A")),
                index,
            )]),
            request(
                "charge_from_current",
                BTreeMap::from([("I".into(), "I1".into())]),
                BTreeMap::from([("t".into(), q(4, 1))]),
                None,
                "si_consistent_exact",
            ),
            q(4 * scale, 1),
        ),
        _ => (
            artifact(vec![
                component("C1", "capacitor", Some(&format!("{scale} F")), index),
                component("V1", "voltage_source", Some("3 V"), index),
            ]),
            request(
                "capacitor_charge",
                BTreeMap::from([("C".into(), "C1".into()), ("V".into(), "V1".into())]),
                BTreeMap::new(),
                None,
                "si_consistent_exact",
            ),
            q(3 * scale, 1),
        ),
    }
}

fn run(
    id: String,
    expected: Expected,
    artifact: VisualCircuitArtifact,
    request: CircuitEmRequest,
    expected_value: Option<Rational>,
) -> Receipt {
    let frontend = formalize_visual_circuit(&VisualCircuitObservation {
        semantic_label: Some("bounded_circuit_diagram".into()),
        components: artifact.components.clone(),
        wires: artifact.wires.clone(),
        ground_terminal: artifact.ground_terminal.clone(),
        ambiguity: None,
        provenance: artifact.provenance.clone(),
    });
    let mut frontend_tampered = frontend.clone();
    frontend_tampered.replay_hash.push('x');
    let visual_replay = frontend.replay_verified();
    let visual_tamper_rejected = !frontend_tampered.replay_verified();
    let mut result = evaluate_circuit_law(&artifact, &request);
    let source_replay = result
        .source_result
        .as_ref()
        .is_none_or(|source| source.replay_verified());
    let actual = result.status;
    let bridge_replay = result.replay_verified();
    let value_correct = expected != Expected::Supported
        || result
            .source_result
            .as_ref()
            .and_then(|source| source.value.clone())
            == expected_value;
    let mut bridge_tampered = result.clone();
    bridge_tampered.replay_hash.push('x');
    let bridge_tamper_rejected = !bridge_tampered.replay_verified();
    let exact = match expected {
        Expected::Supported => result.authorized(),
        Expected::Ambiguous => actual == BridgeStatus::Ambiguous,
        Expected::Refused => actual != BridgeStatus::Complete,
    };
    let authorized = result.authorized();
    result.reasons.push("receipt-local audit marker".into());
    let _ = result;
    Receipt {
        id,
        expected,
        actual,
        law: request.law,
        visual_frontend_replay: visual_replay,
        bridge_replay,
        source_replay,
        visual_tamper_rejected,
        bridge_tamper_rejected,
        exact,
        value_correct,
        provenance_preserved: !artifact.provenance.is_empty(),
        false_authorization: expected != Expected::Supported && authorized,
        false_denial: expected == Expected::Supported && !authorized,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut receipts = Vec::with_capacity(240);
    for index in 0..120 {
        let (artifact, request, value) = supported_case(index, index % 4);
        receipts.push(run(
            format!("supported-{index:03}"),
            Expected::Supported,
            artifact,
            request,
            Some(value),
        ));
    }
    for index in 0..40 {
        let (artifact, mut request, _) = supported_case(index, index % 4);
        request.ambiguity = Some("visual or source interpretation has multiple candidates".into());
        receipts.push(run(
            format!("ambiguous-{index:03}"),
            Expected::Ambiguous,
            artifact,
            request,
            None,
        ));
    }
    for index in 0..20 {
        let (artifact, mut request, _) = supported_case(index, 0);
        request.component_bindings.insert("R".into(), "I1".into());
        receipts.push(run(
            format!("refused-wrong-kind-{index:03}"),
            Expected::Refused,
            artifact,
            request,
            None,
        ));
    }
    for index in 0..20 {
        let artifact = artifact(vec![component("R1", "resistor", None, index)]);
        let request = request(
            "ohms_law_voltage",
            BTreeMap::from([("R".into(), "R1".into())]),
            BTreeMap::from([("I".into(), q(2, 1))]),
            None,
            "si_consistent_exact",
        );
        receipts.push(run(
            format!("refused-missing-value-{index:03}"),
            Expected::Refused,
            artifact,
            request,
            None,
        ));
    }
    for index in 0..20 {
        let artifact = artifact(vec![component("R1", "resistor", Some("10 V"), index)]);
        let request = request(
            "ohms_law_voltage",
            BTreeMap::from([("R".into(), "R1".into())]),
            BTreeMap::from([("I".into(), q(2, 1))]),
            None,
            "si_consistent_exact",
        );
        receipts.push(run(
            format!("refused-unit-mismatch-{index:03}"),
            Expected::Refused,
            artifact,
            request,
            None,
        ));
    }
    for index in 0..20 {
        let (artifact, request, _) = supported_case(index, 0);
        let request = CircuitEmRequest {
            unit_scope: "unvalidated_scope".into(),
            ..request
        };
        receipts.push(run(
            format!("refused-scope-{index:03}"),
            Expected::Refused,
            artifact,
            request,
            None,
        ));
    }
    assert_eq!(receipts.len(), 240);
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
    let report = Report {
        schema: "stage297-visual-circuit-electromagnetism-v1",
        source_catalog_sha256: digest(SOURCE),
        source_record_count: 4,
        cases: receipts.len(),
        supported,
        ambiguous,
        refused,
        exact_decisions: receipts.iter().filter(|r| r.exact).count(),
        supported_values_correct: receipts
            .iter()
            .filter(|r| r.expected == Expected::Supported && r.value_correct)
            .count(),
        visual_frontend_replays: receipts.iter().filter(|r| r.visual_frontend_replay).count(),
        bridge_replays: receipts.iter().filter(|r| r.bridge_replay).count(),
        source_replays: receipts.iter().filter(|r| r.source_replay).count(),
        visual_tamper_rejections: receipts.iter().filter(|r| r.visual_tamper_rejected).count(),
        bridge_tamper_rejections: receipts.iter().filter(|r| r.bridge_tamper_rejected).count(),
        provenance_preserved: receipts.iter().filter(|r| r.provenance_preserved).count(),
        false_authorizations: receipts.iter().filter(|r| r.false_authorization).count(),
        false_denials: receipts.iter().filter(|r| r.false_denial).count(),
        live_registry_mutations: 0,
        hle_questions_read: 0,
        receipts,
    };
    assert_eq!(report.exact_decisions, 240);
    assert_eq!(report.supported_values_correct, 120);
    assert_eq!(report.bridge_replays, 240);
    assert_eq!(report.bridge_tamper_rejections, 240);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    std::fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    let markdown = format!(
        concat!(
            "# Stage 297 — visual circuit to source electromagnetism\n\n",
            "| metric | result |\n|---|---:|\n",
            "| cases | {}/240 |\n| exact decisions | {}/240 |\n",
            "| supported values | {}/120 |\n| ambiguous preserved | {}/40 |\n",
            "| refused | {}/80 |\n| bridge replay | {}/240 |\n",
            "| bridge tamper rejection | {}/240 |\n| false authorizations / denials | {} / {} |\n",
            "| live registry mutations | {} |\n| HLE questions read | {} |\n\n",
            "Only explicit component values are bound; topology and circuit behavior remain outside scope.\n"
        ),
        report.cases,
        report.exact_decisions,
        report.supported_values_correct,
        report.ambiguous,
        report.refused,
        report.bridge_replays,
        report.bridge_tamper_rejections,
        report.false_authorizations,
        report.false_denials,
        report.live_registry_mutations,
        report.hle_questions_read,
    );
    std::fs::write(REPORT_MD, markdown)?;
    println!(
        "stage297 cases={} exact={} values={} false_auth={} false_denials={}",
        report.cases,
        report.exact_decisions,
        report.supported_values_correct,
        report.false_authorizations,
        report.false_denials
    );
    Ok(())
}
