//! Governed bridge from explicit circuit observations to source-derived laws.
//!
//! This bridge binds only component values that are explicitly present in a
//! validated visual-circuit artifact.  It never infers topology, current,
//! voltage, polarity, equivalent resistance, or circuit behavior.

use crate::electromagnetism_pack::{evaluate, EmRequest, EmResult, EmStatus};
use crate::probability_pack::Rational;
use crate::vision::visual_circuit::VisualCircuitArtifact;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const DOMAIN: &str = "visual_circuit_to_source_electromagnetism";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BridgeStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CircuitEmRequest {
    pub law: String,
    /// Maps source-law input names (`I`, `R`, `V`, or `C`) to explicit
    /// component identities in the visual artifact.  `t` must be supplied in
    /// `extra_inputs`, never inferred from a diagram.
    pub component_bindings: BTreeMap<String, String>,
    pub extra_inputs: BTreeMap<String, Rational>,
    pub unit_scope: String,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CircuitEmResult {
    pub status: BridgeStatus,
    pub bound_inputs: BTreeMap<String, Rational>,
    pub source_result: Option<EmResult>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("circuit bridge serializes"))
    )
}

fn payload(result: &CircuitEmResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.bound_inputs,
        &result.source_result,
        &result.reasons,
        &result.provenance,
    )
}

fn finish(
    status: BridgeStatus,
    bound_inputs: BTreeMap<String, Rational>,
    source_result: Option<EmResult>,
    reasons: Vec<String>,
    provenance: Vec<String>,
) -> CircuitEmResult {
    let mut result = CircuitEmResult {
        status,
        bound_inputs,
        source_result,
        reasons,
        provenance,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&(
        result.status,
        &result.bound_inputs,
        &result.source_result,
        &result.reasons,
        &result.provenance,
    ));
    result.replay_hash = replay_hash;
    result
}

fn expected_component_kind(input: &str) -> Option<&'static str> {
    match input {
        "I" => Some("current_source"),
        "R" => Some("resistor"),
        "V" => Some("voltage_source"),
        "C" => Some("capacitor"),
        _ => None,
    }
}

fn expected_unit(input: &str) -> Option<&'static str> {
    match input {
        "I" => Some("A"),
        "R" => Some("ohm"),
        "V" => Some("V"),
        "C" => Some("F"),
        _ => None,
    }
}

fn parse_exact_value(raw: &str, expected_unit: &str) -> Result<Rational, String> {
    let mut parts = raw.split_whitespace();
    let number = parts.next().ok_or("component value is empty")?;
    let unit = parts
        .next()
        .ok_or("component value needs an explicit unit")?;
    if parts.next().is_some() || unit != expected_unit {
        return Err(format!(
            "component value must be an exact {expected_unit} reading"
        ));
    }
    let (numerator, denominator) = number.split_once('/').map_or_else(
        || {
            number.parse::<i128>().map(|n| (n, 1)).map_err(|_| {
                "only exact integer or numerator/denominator values are supported".to_string()
            })
        },
        |(n, d)| {
            let numerator = n
                .parse::<i128>()
                .map_err(|_| "invalid exact numerator".to_string())?;
            let denominator = d
                .parse::<i128>()
                .map_err(|_| "invalid exact denominator".to_string())?;
            Ok((numerator, denominator))
        },
    )?;
    Rational::new(numerator, denominator).ok_or_else(|| "invalid rational component value".into())
}

/// Bind explicit component readings to a source-derived electromagnetism law.
/// The bridge intentionally has no topology or circuit-solving operation.
pub fn evaluate_circuit_law(
    artifact: &VisualCircuitArtifact,
    request: &CircuitEmRequest,
) -> CircuitEmResult {
    let mut provenance = artifact.provenance.clone();
    provenance.extend(request.provenance.clone());
    if artifact.provenance.is_empty() || request.provenance.is_empty() {
        return finish(
            BridgeStatus::Missing,
            BTreeMap::new(),
            None,
            vec!["visual and request provenance are both required".into()],
            provenance,
        );
    }
    if request.unit_scope != "si_consistent_exact" {
        return finish(
            BridgeStatus::Unsupported,
            BTreeMap::new(),
            None,
            vec!["only the source pack's exact SI scope is supported".into()],
            provenance,
        );
    }
    if let Some(ambiguity) = &request.ambiguity {
        return finish(
            BridgeStatus::Ambiguous,
            BTreeMap::new(),
            None,
            vec![ambiguity.clone()],
            provenance,
        );
    }
    let components: BTreeMap<_, _> = artifact
        .components
        .iter()
        .map(|component| (component.id.as_str(), component))
        .collect();
    if components.len() != artifact.components.len() {
        return finish(
            BridgeStatus::Invalid,
            BTreeMap::new(),
            None,
            vec!["component identities are not unique".into()],
            provenance,
        );
    }
    let mut used_components = BTreeSet::new();
    let mut bound_inputs = request.extra_inputs.clone();
    for (input, component_id) in &request.component_bindings {
        let expected_kind = match expected_component_kind(input) {
            Some(kind) => kind,
            None => {
                return finish(
                    BridgeStatus::Unsupported,
                    bound_inputs,
                    None,
                    vec![format!(
                        "input {input} has no visual-component binding contract"
                    )],
                    provenance,
                )
            }
        };
        if !used_components.insert(component_id.clone()) {
            return finish(
                BridgeStatus::Invalid,
                bound_inputs,
                None,
                vec!["one explicit component cannot supply two distinct law inputs".into()],
                provenance,
            );
        }
        let component = match components.get(component_id.as_str()) {
            Some(component) => *component,
            None => {
                return finish(
                    BridgeStatus::Missing,
                    bound_inputs,
                    None,
                    vec![format!("component {component_id} is not present")],
                    provenance,
                )
            }
        };
        if component.kind != expected_kind {
            return finish(
                BridgeStatus::Unsupported,
                bound_inputs,
                None,
                vec![format!(
                    "{input} requires an explicit {expected_kind} component"
                )],
                provenance,
            );
        }
        let raw = match &component.value {
            Some(raw) => raw,
            None => {
                return finish(
                    BridgeStatus::Missing,
                    bound_inputs,
                    None,
                    vec![format!("component {component_id} has no explicit value")],
                    provenance,
                )
            }
        };
        let unit = expected_unit(input).expect("component input has a unit");
        match parse_exact_value(raw, unit) {
            Ok(value) => {
                bound_inputs.insert(input.clone(), value);
            }
            Err(reason) => {
                return finish(
                    BridgeStatus::Unsupported,
                    bound_inputs,
                    None,
                    vec![format!("component {component_id}: {reason}")],
                    provenance,
                )
            }
        }
    }
    if request.component_bindings.is_empty() {
        return finish(
            BridgeStatus::Missing,
            bound_inputs,
            None,
            vec!["at least one explicit visual component binding is required".into()],
            provenance,
        );
    }
    let source_request = EmRequest {
        law: request.law.clone(),
        inputs: bound_inputs.clone(),
        domain: "source_derived_bounded_electromagnetism".into(),
        unit_scope: request.unit_scope.clone(),
        ambiguity: None,
        provenance: provenance.clone(),
    };
    let source_result = evaluate(&source_request);
    let status = match source_result.status {
        EmStatus::Complete if source_result.authorized() => BridgeStatus::Complete,
        EmStatus::Ambiguous => BridgeStatus::Ambiguous,
        EmStatus::InvalidDomain | EmStatus::Unsupported => BridgeStatus::Unsupported,
        EmStatus::Missing => BridgeStatus::Missing,
        EmStatus::Complete => BridgeStatus::Invalid,
    };
    finish(
        status,
        bound_inputs,
        Some(source_result),
        Vec::new(),
        provenance,
    )
}

impl CircuitEmResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
    }

    pub fn authorized(&self) -> bool {
        self.status == BridgeStatus::Complete
            && self.replay_verified()
            && self
                .source_result
                .as_ref()
                .is_some_and(|result| result.authorized())
            && !self.provenance.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vision::visual_circuit::{
        formalize_visual_circuit, CircuitComponentObservation, CircuitWireObservation,
        VisualCircuitObservation,
    };

    fn artifact() -> VisualCircuitArtifact {
        let observation = VisualCircuitObservation {
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
                    id: "I1".into(),
                    kind: "current_source".into(),
                    terminals: vec!["I1.a".into(), "I1.b".into()],
                    value: Some("2 A".into()),
                    confidence: 99,
                },
            ],
            wires: vec![CircuitWireObservation {
                id: "w1".into(),
                from: "R1.a".into(),
                to: "I1.a".into(),
                confidence: 99,
            }],
            ground_terminal: None,
            ambiguity: None,
            provenance: vec!["visual:test".into()],
        };
        formalize_visual_circuit(&observation)
            .artifact
            .expect("explicit circuit artifact")
    }

    #[test]
    fn explicit_values_reach_source_law_without_solving_topology() {
        let result = evaluate_circuit_law(
            &artifact(),
            &CircuitEmRequest {
                law: "ohms_law_voltage".into(),
                component_bindings: BTreeMap::from([
                    ("I".into(), "I1".into()),
                    ("R".into(), "R1".into()),
                ]),
                extra_inputs: BTreeMap::new(),
                unit_scope: "si_consistent_exact".into(),
                ambiguity: None,
                provenance: vec!["question:test".into()],
            },
        );
        assert_eq!(result.status, BridgeStatus::Complete);
        assert!(result.authorized());
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        assert!(!tampered.replay_verified());
    }

    #[test]
    fn binding_never_accepts_wrong_component_kind() {
        let mut request = CircuitEmRequest {
            law: "ohms_law_voltage".into(),
            component_bindings: BTreeMap::from([("R".into(), "I1".into())]),
            extra_inputs: BTreeMap::new(),
            unit_scope: "si_consistent_exact".into(),
            ambiguity: None,
            provenance: vec!["question:test".into()],
        };
        let result = evaluate_circuit_law(&artifact(), &request);
        assert_eq!(result.status, BridgeStatus::Unsupported);
        request.component_bindings.clear();
        request
            .extra_inputs
            .insert("R".into(), Rational::new(10, 1).unwrap());
        let result = evaluate_circuit_law(&artifact(), &request);
        assert_eq!(result.status, BridgeStatus::Missing);
    }
}
