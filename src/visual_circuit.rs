//! Conservative coordinate/provenance-preserving circuit-diagram frontend.
//!
//! This module recognizes only explicit component and terminal observations.
//! It never infers voltage, current, equivalent resistance, polarity, or
//! circuit behavior from a drawing.  A later electrical solver must consume
//! this artifact under its own typed assumptions.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const DOMAIN: &str = "visual_bounded_circuit";
const MAX_COMPONENTS: usize = 32;
const MAX_WIRES: usize = 64;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CircuitStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CircuitComponentObservation {
    pub id: String,
    pub kind: String,
    pub terminals: Vec<String>,
    pub value: Option<String>,
    pub confidence: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CircuitWireObservation {
    pub id: String,
    pub from: String,
    pub to: String,
    pub confidence: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualCircuitObservation {
    pub semantic_label: Option<String>,
    pub components: Vec<CircuitComponentObservation>,
    pub wires: Vec<CircuitWireObservation>,
    pub ground_terminal: Option<String>,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualCircuitArtifact {
    pub components: Vec<CircuitComponentObservation>,
    pub wires: Vec<CircuitWireObservation>,
    pub ground_terminal: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualCircuitResult {
    pub status: CircuitStatus,
    pub artifact: Option<VisualCircuitArtifact>,
    pub alternatives: Vec<String>,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("visual circuit serializes"))
    )
}

fn payload(result: &VisualCircuitResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.artifact,
        &result.alternatives,
        &result.reasons,
    )
}

fn result(
    status: CircuitStatus,
    artifact: Option<VisualCircuitArtifact>,
    alternatives: Vec<String>,
    reasons: Vec<String>,
) -> VisualCircuitResult {
    let mut output = VisualCircuitResult {
        status,
        artifact,
        alternatives,
        reasons,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&(
        output.status,
        &output.artifact,
        &output.alternatives,
        &output.reasons,
    ));
    output.replay_hash = replay_hash;
    output
}

fn supported_kind(kind: &str) -> bool {
    matches!(
        kind,
        "resistor"
            | "capacitor"
            | "inductor"
            | "voltage_source"
            | "current_source"
            | "switch"
            | "diode"
    )
}

/// Formalize a bounded circuit diagram from explicit component/terminal
/// observations.  No electrical law or value is inferred here.
pub fn formalize_visual_circuit(input: &VisualCircuitObservation) -> VisualCircuitResult {
    if let Some(ambiguity) = &input.ambiguity {
        return result(
            CircuitStatus::Ambiguous,
            None,
            vec![ambiguity.clone()],
            vec!["visual extractor reported unresolved circuit alternatives".into()],
        );
    }
    if input.provenance.is_empty() {
        return result(
            CircuitStatus::Missing,
            None,
            Vec::new(),
            vec!["circuit observations need provenance".into()],
        );
    }
    if input.semantic_label.as_deref() != Some("bounded_circuit_diagram") {
        return result(
            CircuitStatus::Unsupported,
            None,
            Vec::new(),
            vec!["visual structure does not establish bounded circuit semantics".into()],
        );
    }
    if input.components.is_empty() {
        return result(
            CircuitStatus::Missing,
            None,
            Vec::new(),
            vec!["at least one explicit circuit component is required".into()],
        );
    }
    if input.components.len() > MAX_COMPONENTS || input.wires.len() > MAX_WIRES {
        return result(
            CircuitStatus::Unsupported,
            None,
            Vec::new(),
            vec!["circuit exceeds the bounded component or wire budget".into()],
        );
    }
    let mut components = BTreeSet::new();
    let mut terminals = BTreeSet::new();
    for component in &input.components {
        if component.id.trim().is_empty() || component.terminals.len() != 2 {
            return result(
                CircuitStatus::Missing,
                None,
                Vec::new(),
                vec!["each component needs an id and exactly two explicit terminals".into()],
            );
        }
        if !supported_kind(&component.kind) {
            return result(
                CircuitStatus::Unsupported,
                None,
                Vec::new(),
                vec![format!(
                    "component kind '{}' is outside the bounded vocabulary",
                    component.kind
                )],
            );
        }
        if !components.insert(component.id.clone()) {
            return result(
                CircuitStatus::Invalid,
                None,
                Vec::new(),
                vec!["duplicate component ids are not identity-safe".into()],
            );
        }
        if component.terminals[0] == component.terminals[1]
            || component
                .terminals
                .iter()
                .any(|terminal| terminal.trim().is_empty())
        {
            return result(
                CircuitStatus::Invalid,
                None,
                Vec::new(),
                vec!["component terminals must be distinct and nonempty".into()],
            );
        }
        if component
            .terminals
            .iter()
            .any(|terminal| !terminals.insert(terminal.clone()))
        {
            return result(
                CircuitStatus::Invalid,
                None,
                Vec::new(),
                vec!["terminal identities must be unique across components".into()],
            );
        }
        if component
            .value
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return result(
                CircuitStatus::Missing,
                None,
                Vec::new(),
                vec!["declared component values must be nonempty".into()],
            );
        }
        if component.confidence < 80 {
            return result(
                CircuitStatus::Ambiguous,
                None,
                vec![component.id.clone()],
                vec!["component confidence is below the semantic boundary".into()],
            );
        }
    }
    if let Some(ground) = &input.ground_terminal {
        if !terminals.contains(ground) {
            return result(
                CircuitStatus::Invalid,
                None,
                Vec::new(),
                vec!["ground terminal is not an explicit component terminal".into()],
            );
        }
    }
    let mut wires = BTreeSet::new();
    let mut connections = BTreeSet::new();
    for wire in &input.wires {
        if wire.id.trim().is_empty() {
            return result(
                CircuitStatus::Missing,
                None,
                Vec::new(),
                vec!["wire identifiers must be explicit".into()],
            );
        }
        if !wires.insert(wire.id.clone()) {
            return result(
                CircuitStatus::Invalid,
                None,
                Vec::new(),
                vec!["duplicate wire ids are not identity-safe".into()],
            );
        }
        if !terminals.contains(&wire.from) || !terminals.contains(&wire.to) {
            return result(
                CircuitStatus::Invalid,
                None,
                Vec::new(),
                vec!["wire endpoint is not an explicit terminal".into()],
            );
        }
        if wire.from == wire.to {
            return result(
                CircuitStatus::Unsupported,
                None,
                Vec::new(),
                vec!["self-connected wires are outside the bounded boundary".into()],
            );
        }
        if wire.confidence < 80 {
            return result(
                CircuitStatus::Ambiguous,
                None,
                vec![wire.id.clone()],
                vec!["wire confidence is below the semantic boundary".into()],
            );
        }
        let key = if wire.from <= wire.to {
            (wire.from.clone(), wire.to.clone())
        } else {
            (wire.to.clone(), wire.from.clone())
        };
        if !connections.insert(key) {
            return result(
                CircuitStatus::Invalid,
                None,
                Vec::new(),
                vec!["duplicate terminal connections are not identity-safe".into()],
            );
        }
    }
    result(
        CircuitStatus::Complete,
        Some(VisualCircuitArtifact {
            components: input.components.clone(),
            wires: input.wires.clone(),
            ground_terminal: input.ground_terminal.clone(),
            provenance: input.provenance.clone(),
        }),
        Vec::new(),
        Vec::new(),
    )
}

impl VisualCircuitResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
    }

    pub fn authorized(&self) -> bool {
        self.status == CircuitStatus::Complete
            && self.artifact.as_ref().is_some_and(|artifact| {
                !artifact.components.is_empty() && !artifact.provenance.is_empty()
            })
            && self.replay_verified()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation() -> VisualCircuitObservation {
        VisualCircuitObservation {
            semantic_label: Some("bounded_circuit_diagram".into()),
            components: vec![
                CircuitComponentObservation {
                    id: "R1".into(),
                    kind: "resistor".into(),
                    terminals: vec!["R1.a".into(), "R1.b".into()],
                    value: Some("10 ohm".into()),
                    confidence: 99,
                },
                CircuitComponentObservation {
                    id: "V1".into(),
                    kind: "voltage_source".into(),
                    terminals: vec!["V1.a".into(), "V1.b".into()],
                    value: Some("5 V".into()),
                    confidence: 99,
                },
            ],
            wires: vec![
                CircuitWireObservation {
                    id: "w1".into(),
                    from: "V1.a".into(),
                    to: "R1.a".into(),
                    confidence: 99,
                },
                CircuitWireObservation {
                    id: "w2".into(),
                    from: "R1.b".into(),
                    to: "V1.b".into(),
                    confidence: 99,
                },
            ],
            ground_terminal: Some("V1.b".into()),
            ambiguity: None,
            provenance: vec!["circuit:test".into()],
        }
    }

    #[test]
    fn explicit_circuit_replays_without_solving() {
        let result = formalize_visual_circuit(&observation());
        assert_eq!(result.status, CircuitStatus::Complete);
        assert!(result.authorized());
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        assert!(!tampered.replay_verified());
    }

    #[test]
    fn unsupported_component_is_refused() {
        let mut input = observation();
        input.components[0].kind = "transistor_model".into();
        assert_eq!(
            formalize_visual_circuit(&input).status,
            CircuitStatus::Unsupported
        );
    }
}
