//! Shadow-only structural formalization between mechanics prose and the
//! externally grounded classical-mechanics pack.

use crate::classical_mechanics_pack::{
    evaluate_mechanics, MechanicsEvaluationRequest, MechanicsStatus, NumericBinding,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProvenanceSpan {
    pub field: String,
    pub marker: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SituationStatus {
    Unique,
    Ambiguous,
    Missing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MechanicsSituation {
    pub status: SituationStatus,
    pub mass: Option<f64>,
    pub force: Option<f64>,
    pub acceleration: Option<f64>,
    pub velocity: Option<f64>,
    pub spring_constant: Option<f64>,
    pub displacement: Option<f64>,
    pub requested_output: Option<String>,
    pub candidate_laws: Vec<String>,
    pub unresolved_assumptions: Vec<String>,
    pub provenance: Vec<ProvenanceSpan>,
    pub replay_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SituationExecution {
    pub situation: MechanicsSituation,
    pub mechanics_status: Option<MechanicsStatus>,
    pub value: Option<f64>,
    pub law_id: Option<String>,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("situation serializes"))
    )
}

fn number_after(text: &str, marker: &str) -> Option<f64> {
    let lower = text.to_ascii_lowercase();
    let start = lower.find(marker)? + marker.len();
    let suffix = &text[start..];
    let mut token = String::new();
    let mut started = false;
    for character in suffix.chars() {
        if character.is_ascii_digit()
            || (character == '.' && started)
            || (character == '-' && !started)
        {
            token.push(character);
            started = true;
        } else if started {
            break;
        }
    }
    token.parse().ok()
}

fn has(text: &str, marker: &str) -> bool {
    text.to_ascii_lowercase().contains(marker)
}

fn asks(text: &str, target: &str) -> bool {
    has(text, target)
        && [
            "find",
            "calculate",
            "compute",
            "determine",
            "evaluate",
            "what is",
        ]
        .iter()
        .any(|verb| has(text, verb) || has(text, "what "))
}

fn add_provenance(provenance: &mut Vec<ProvenanceSpan>, field: &str, marker: &str, text: &str) {
    if has(text, marker) {
        provenance.push(ProvenanceSpan {
            field: field.into(),
            marker: marker.into(),
        });
    }
}

pub fn formalize_mechanics_situation(text: &str) -> MechanicsSituation {
    let lower = text.to_ascii_lowercase();
    let mass = number_after(text, "mass");
    let force = number_after(text, "net force").or_else(|| number_after(text, "force"));
    let acceleration = number_after(text, "acceleration");
    let velocity = number_after(text, "velocity")
        .or_else(|| number_after(text, "speed"))
        .or_else(|| number_after(text, "moves at"));
    let spring_constant =
        number_after(text, "spring constant").or_else(|| number_after(text, "stiffness"));
    let displacement = number_after(text, "displacement")
        .or_else(|| number_after(text, "extension"))
        .or_else(|| number_after(text, "compression"))
        .or_else(|| number_after(text, "displaced by"));
    let requested_output = if asks(text, "kinetic energy") || asks(text, "energy of motion") {
        Some("K".into())
    } else if asks(text, "elastic potential") || asks(text, "spring energy") {
        Some("U".into())
    } else if asks(text, "momentum") {
        Some("p".into())
    } else if asks(text, "restoring force") || asks(text, "spring force") {
        Some("F_spring".into())
    } else if asks(text, "acceleration") {
        Some("a".into())
    } else if asks(text, "net force") {
        Some("F_net".into())
    } else {
        None
    };
    let mut candidates = Vec::new();
    if (mass.is_some() && force.is_some() && requested_output.as_deref() == Some("a"))
        || (mass.is_some()
            && acceleration.is_some()
            && requested_output.as_deref() == Some("F_net"))
        || (force.is_some() && acceleration.is_some() && requested_output.as_deref() == Some("m"))
    {
        candidates.push("newtons_second_law".to_string());
    }
    if mass.is_some() && velocity.is_some() && requested_output.as_deref() == Some("p") {
        candidates.push("linear_momentum".to_string());
    }
    if mass.is_some() && velocity.is_some() && requested_output.as_deref() == Some("K") {
        candidates.push("kinetic_energy".to_string());
    }
    if spring_constant.is_some()
        && displacement.is_some()
        && requested_output.as_deref() == Some("F_spring")
    {
        candidates.push("hooke_force".to_string());
    }
    if spring_constant.is_some()
        && displacement.is_some()
        && requested_output.as_deref() == Some("U")
    {
        candidates.push("elastic_potential_energy".to_string());
    }
    if mass.is_some() && velocity.is_some() && requested_output.is_none() {
        candidates.extend(["linear_momentum".into(), "kinetic_energy".into()]);
    }
    let mut unresolved_assumptions = Vec::new();
    if (has(text, "momentum") && has(text, "kinetic energy"))
        || (has(text, "spring force") && has(text, "elastic potential"))
    {
        unresolved_assumptions.push("multiple requested law outputs require composition".into());
    }
    if (has(text, "two bodies")
        || has(text, "two objects")
        || has(text, "multiple bodies")
        || has(text, "several bodies"))
        && !has(text, "single body")
    {
        unresolved_assumptions.push("multi-body scope is outside the single-body pack".into());
    }
    let unsupported_domain = (has(text, "relativistic") && !has(text, "non-relativistic"))
        || has(text, "rotation")
        || has(text, "rotational")
        || has(text, "fluid")
        || has(text, "thermodynamic")
        || has(text, "quantum");
    if unsupported_domain {
        unresolved_assumptions.push("requires an out-of-pack mechanics domain".into());
    }
    if !candidates.is_empty()
        && !has(text, "inertial")
        && candidates.iter().any(|law| law == "newtons_second_law")
    {
        unresolved_assumptions.push("inertial reference frame not stated".into());
    }
    if candidates.iter().any(|law| law == "newtons_second_law")
        && force.is_some()
        && !has(text, "net force")
    {
        unresolved_assumptions.push("ordinary force is not proven to be net force".into());
    }
    if candidates
        .iter()
        .any(|law| law == "newtons_second_law" || law == "linear_momentum")
        && has(text, "magnitude")
        && (!has(text, "direction") || has(text, "no direction"))
        && !has(text, "one-dimensional")
    {
        unresolved_assumptions.push("vector direction is not specified".into());
    }
    if candidates
        .iter()
        .any(|law| law == "hooke_force" || law == "elastic_potential_energy")
        && !(has(&lower, "ideal linear spring")
            || has(&lower, "ideal linear regime")
            || has(&lower, "spring is ideal linear"))
    {
        unresolved_assumptions.push("linear spring model not stated".into());
    }
    if candidates.iter().any(|law| law == "kinetic_energy") && !has(&lower, "non-relativistic") {
        unresolved_assumptions.push("non-relativistic regime not stated".into());
    }
    let status = if unsupported_domain {
        SituationStatus::Unsupported
    } else if candidates.len() > 1 {
        SituationStatus::Ambiguous
    } else if candidates.len() == 1 && unresolved_assumptions.is_empty() {
        SituationStatus::Unique
    } else if candidates.is_empty() {
        SituationStatus::Missing
    } else {
        SituationStatus::Ambiguous
    };
    let mut provenance = Vec::new();
    add_provenance(&mut provenance, "mass", "mass", text);
    add_provenance(&mut provenance, "force", "force", text);
    add_provenance(&mut provenance, "acceleration", "acceleration", text);
    add_provenance(&mut provenance, "velocity", "velocity", text);
    add_provenance(&mut provenance, "velocity", "speed", text);
    add_provenance(&mut provenance, "velocity", "moves at", text);
    add_provenance(&mut provenance, "spring_constant", "spring constant", text);
    add_provenance(&mut provenance, "spring_constant", "stiffness", text);
    add_provenance(&mut provenance, "displacement", "displacement", text);
    add_provenance(&mut provenance, "displacement", "extension", text);
    add_provenance(&mut provenance, "displacement", "compression", text);
    add_provenance(&mut provenance, "displacement", "displaced by", text);
    let mut situation = MechanicsSituation {
        status,
        mass,
        force,
        acceleration,
        velocity,
        spring_constant,
        displacement,
        requested_output,
        candidate_laws: candidates,
        unresolved_assumptions,
        provenance,
        replay_hash: String::new(),
    };
    situation.replay_hash = digest(&(
        &situation.status,
        &situation.mass,
        &situation.force,
        &situation.acceleration,
        &situation.velocity,
        &situation.spring_constant,
        &situation.displacement,
        &situation.requested_output,
        &situation.candidate_laws,
        &situation.unresolved_assumptions,
        &situation.provenance,
    ));
    situation
}

pub fn execute_mechanics_situation(situation: &MechanicsSituation) -> SituationExecution {
    let mut reasons = situation.unresolved_assumptions.clone();
    let result = if situation.status == SituationStatus::Unique {
        let law_id = situation.candidate_laws[0].clone();
        let bindings = match law_id.as_str() {
            "newtons_second_law" => vec![
                NumericBinding {
                    symbol: "F_net".into(),
                    value: situation.force.unwrap(),
                    unit: "N".into(),
                    provenance: "situation:force".into(),
                },
                NumericBinding {
                    symbol: "m".into(),
                    value: situation.mass.unwrap(),
                    unit: "kg".into(),
                    provenance: "situation:mass".into(),
                },
            ],
            "linear_momentum" => vec![
                NumericBinding {
                    symbol: "m".into(),
                    value: situation.mass.unwrap(),
                    unit: "kg".into(),
                    provenance: "situation:mass".into(),
                },
                NumericBinding {
                    symbol: "v".into(),
                    value: situation.velocity.unwrap(),
                    unit: "m/s".into(),
                    provenance: "situation:velocity".into(),
                },
            ],
            "kinetic_energy" => vec![
                NumericBinding {
                    symbol: "m".into(),
                    value: situation.mass.unwrap(),
                    unit: "kg".into(),
                    provenance: "situation:mass".into(),
                },
                NumericBinding {
                    symbol: "v".into(),
                    value: situation.velocity.unwrap(),
                    unit: "m/s".into(),
                    provenance: "situation:velocity".into(),
                },
            ],
            "hooke_force" | "elastic_potential_energy" => vec![
                NumericBinding {
                    symbol: "k".into(),
                    value: situation.spring_constant.unwrap(),
                    unit: "N/m".into(),
                    provenance: "situation:spring_constant".into(),
                },
                NumericBinding {
                    symbol: "x".into(),
                    value: situation.displacement.unwrap(),
                    unit: "m".into(),
                    provenance: "situation:displacement".into(),
                },
            ],
            _ => Vec::new(),
        };
        let request = MechanicsEvaluationRequest {
            law_id,
            bindings,
            requested_output: situation.requested_output.clone().unwrap(),
        };
        Some(evaluate_mechanics(
            &request,
            &crate::classical_mechanics_pack::classical_mechanics_pack(),
        ))
    } else {
        None
    };
    let (mechanics_status, value, law_id) = if let Some(result) = result {
        reasons.extend(result.reasons.clone());
        (Some(result.status), result.value, result.law_id)
    } else {
        (None, None, None)
    };
    let mut execution = SituationExecution {
        situation: situation.clone(),
        mechanics_status,
        value,
        law_id,
        reasons,
        replay_hash: String::new(),
    };
    execution.replay_hash = digest(&(
        &execution.situation,
        &execution.mechanics_status,
        &execution.value,
        &execution.law_id,
        &execution.reasons,
    ));
    execution
}

pub fn replay_situation(situation: &MechanicsSituation) -> bool {
    digest(&(
        &situation.status,
        &situation.mass,
        &situation.force,
        &situation.acceleration,
        &situation.velocity,
        &situation.spring_constant,
        &situation.displacement,
        &situation.requested_output,
        &situation.candidate_laws,
        &situation.unresolved_assumptions,
        &situation.provenance,
    )) == situation.replay_hash
}

pub fn replay_execution(execution: &SituationExecution) -> bool {
    digest(&(
        &execution.situation,
        &execution.mechanics_status,
        &execution.value,
        &execution.law_id,
        &execution.reasons,
    )) == execution.replay_hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_newton_situation_executes_and_replays() {
        let situation = formalize_mechanics_situation(
            "An inertial object has mass 3 kg and net force 12 N. Find acceleration.",
        );
        assert_eq!(situation.status, SituationStatus::Unique);
        assert_eq!(situation.candidate_laws, vec!["newtons_second_law"]);
        assert!(replay_situation(&situation));
        let result = execute_mechanics_situation(&situation);
        assert_eq!(result.value, Some(4.0));
    }

    #[test]
    fn generic_energy_stays_ambiguous() {
        let situation = formalize_mechanics_situation(
            "An object has mass 3 kg and velocity 4 m/s. Find the energy.",
        );
        assert_eq!(situation.status, SituationStatus::Ambiguous);
        assert!(replay_situation(&situation));
    }
}
