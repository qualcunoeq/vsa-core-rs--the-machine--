//! Shadow-only externally grounded classical-mechanics knowledge pack.
//!
//! The pack contains a small, provenance-bearing set of textbook relations.
//! It is intentionally not registered with the production router and does not
//! answer HLE questions.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceCitation {
    pub source_id: String,
    pub title: String,
    pub section: String,
    pub url: String,
    pub license: String,
    pub retrieved_utc: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MechanicsLawKind {
    NewtonSecondLaw,
    Momentum,
    KineticEnergy,
    HookeForce,
    ElasticPotentialEnergy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MechanicsLaw {
    pub law_id: String,
    pub aliases: Vec<String>,
    pub kind: MechanicsLawKind,
    pub equation: String,
    pub variables: Vec<String>,
    pub assumptions: Vec<String>,
    pub validity_domain: String,
    pub unit_constraints: Vec<String>,
    pub source: SourceCitation,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MechanicsStatus {
    Complete,
    Missing,
    Ambiguous,
    UnitMismatch,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NumericBinding {
    pub symbol: String,
    pub value: f64,
    pub unit: String,
    pub provenance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MechanicsEvaluationRequest {
    pub law_id: String,
    pub bindings: Vec<NumericBinding>,
    pub requested_output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MechanicsEvaluationResult {
    pub status: MechanicsStatus,
    pub value: Option<f64>,
    pub law_id: Option<String>,
    pub equation: Option<String>,
    pub assumptions: Vec<String>,
    pub source: Option<SourceCitation>,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("mechanics value serializes"))
    )
}

fn source(section: &str, url: &str) -> SourceCitation {
    SourceCitation {
        source_id: format!("openstax-university-physics-volume-1:{section}"),
        title: "University Physics Volume 1".into(),
        section: section.into(),
        url: url.into(),
        license: "CC BY-NC-SA 4.0; OpenStax attribution required".into(),
        retrieved_utc: "2026-08-01".into(),
    }
}

pub fn classical_mechanics_pack() -> Vec<MechanicsLaw> {
    vec![
        MechanicsLaw {
            law_id: "newtons_second_law".into(),
            aliases: vec!["Newton's second law".into(), "net force law".into()],
            kind: MechanicsLawKind::NewtonSecondLaw,
            equation: "F_net = m * a".into(),
            variables: vec!["F_net".into(), "m".into(), "a".into()],
            assumptions: vec![
                "inertial reference frame".into(),
                "constant mass for F=ma form".into(),
                "net external force is represented".into(),
            ],
            validity_domain: "classical mechanics; non-relativistic; constant-mass form".into(),
            unit_constraints: vec![
                "F_net=N".into(),
                "m=kg".into(),
                "a=m/s^2".into(),
            ],
            source: source(
                "5.3 Newton's Second Law",
                "https://openstax.org/books/university-physics-volume-1/pages/5-3-newtons-second-law",
            ),
        },
        MechanicsLaw {
            law_id: "linear_momentum".into(),
            aliases: vec!["momentum definition".into(), "linear momentum".into()],
            kind: MechanicsLawKind::Momentum,
            equation: "p = m * v".into(),
            variables: vec!["p".into(), "m".into(), "v".into()],
            assumptions: vec!["classical point-particle momentum".into()],
            validity_domain: "classical mechanics; vector direction retained by signed components".into(),
            unit_constraints: vec!["p=kg*m/s".into(), "m=kg".into(), "v=m/s".into()],
            source: source(
                "9.1 Linear Momentum",
                "https://openstax.org/books/university-physics-volume-1/pages/9-1-linear-momentum",
            ),
        },
        MechanicsLaw {
            law_id: "kinetic_energy".into(),
            aliases: vec!["kinetic energy".into(), "energy of motion".into(), "energy".into()],
            kind: MechanicsLawKind::KineticEnergy,
            equation: "K = 1/2 * m * v^2".into(),
            variables: vec!["K".into(), "m".into(), "v".into()],
            assumptions: vec!["non-relativistic speed".into(), "particle or translational motion".into()],
            validity_domain: "classical non-relativistic mechanics".into(),
            unit_constraints: vec!["K=J".into(), "m=kg".into(), "v=m/s".into()],
            source: source(
                "7.2 Kinetic Energy",
                "https://openstax.org/books/university-physics-volume-1/pages/7-2-kinetic-energy",
            ),
        },
        MechanicsLaw {
            law_id: "hooke_force".into(),
            aliases: vec!["Hooke's law".into(), "spring restoring force".into()],
            kind: MechanicsLawKind::HookeForce,
            equation: "F_spring = -k * x".into(),
            variables: vec!["F_spring".into(), "k".into(), "x".into()],
            assumptions: vec!["ideal linear spring".into(), "x measured from relaxed position".into()],
            validity_domain: "linear elastic regime".into(),
            unit_constraints: vec!["F_spring=N".into(), "k=N/m".into(), "x=m".into()],
            source: source(
                "5.6 Common Forces",
                "https://openstax.org/books/university-physics-volume-1/pages/5-6-common-forces",
            ),
        },
        MechanicsLaw {
            law_id: "elastic_potential_energy".into(),
            aliases: vec!["elastic potential energy".into(), "spring energy".into(), "energy".into()],
            kind: MechanicsLawKind::ElasticPotentialEnergy,
            equation: "U = 1/2 * k * x^2".into(),
            variables: vec!["U".into(), "k".into(), "x".into()],
            assumptions: vec!["ideal linear spring".into(), "zero at relaxed position".into()],
            validity_domain: "linear elastic regime".into(),
            unit_constraints: vec!["U=J".into(), "k=N/m".into(), "x=m".into()],
            source: source(
                "8.1 Potential Energy of a System",
                "https://openstax.org/books/university-physics-volume-1/pages/8-1-potential-energy-of-a-system",
            ),
        },
    ]
}

pub fn lookup_mechanics(
    alias: &str,
    pack: &[MechanicsLaw],
) -> (MechanicsStatus, Vec<MechanicsLaw>) {
    let normalized = alias.trim().to_ascii_lowercase();
    let mut matches = pack
        .iter()
        .filter(|law| {
            law.law_id == normalized
                || law
                    .aliases
                    .iter()
                    .any(|candidate| candidate.to_ascii_lowercase() == normalized)
        })
        .cloned()
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| left.law_id.cmp(&right.law_id));
    let status = match matches.len() {
        0 if normalized.is_empty() => MechanicsStatus::Missing,
        0 => MechanicsStatus::Unsupported,
        1 => MechanicsStatus::Complete,
        _ => MechanicsStatus::Ambiguous,
    };
    (status, matches)
}

fn expected_unit(law: &MechanicsLaw, symbol: &str) -> Option<String> {
    law.unit_constraints.iter().find_map(|constraint| {
        let (name, unit) = constraint.split_once('=')?;
        (name == symbol).then_some(unit.to_string())
    })
}

pub fn evaluate_mechanics(
    request: &MechanicsEvaluationRequest,
    pack: &[MechanicsLaw],
) -> MechanicsEvaluationResult {
    let (lookup_status, matches) = lookup_mechanics(&request.law_id, pack);
    let mut reasons = Vec::new();
    let law = if lookup_status == MechanicsStatus::Complete {
        Some(&matches[0])
    } else {
        reasons.push("law alias does not identify one pack record".into());
        None
    };
    let mut bindings = request.bindings.clone();
    bindings.sort_by(|left, right| left.symbol.cmp(&right.symbol));
    let duplicate = bindings
        .windows(2)
        .any(|pair| pair[0].symbol == pair[1].symbol);
    let missing = law.map(|record| {
        record
            .variables
            .iter()
            .filter(|variable| **variable != request.requested_output)
            .filter(|variable| !bindings.iter().any(|binding| &binding.symbol == *variable))
            .count()
    });
    if duplicate {
        reasons.push("duplicate input binding".into());
    }
    if missing.unwrap_or(0) > 0 {
        let value = missing.expect("missing count exists when positive");
        reasons.push(format!("{value} required input binding(s) missing"));
    }
    let unit_mismatch = law.map(|record| {
        bindings.iter().any(|binding| {
            expected_unit(record, &binding.symbol).is_some_and(|expected| expected != binding.unit)
        })
    }) == Some(true);
    if unit_mismatch {
        reasons.push("input unit conflicts with source law constraint".into());
    }
    let status = if law.is_none() {
        lookup_status
    } else if duplicate {
        MechanicsStatus::Ambiguous
    } else if missing.unwrap_or(0) > 0 {
        MechanicsStatus::Missing
    } else if unit_mismatch {
        MechanicsStatus::UnitMismatch
    } else {
        MechanicsStatus::Complete
    };
    let value = if status == MechanicsStatus::Complete {
        let law = law.expect("complete status has a law");
        let get = |symbol: &str| {
            bindings
                .iter()
                .find(|binding| binding.symbol == symbol)
                .map(|binding| binding.value)
        };
        let compute = || -> Option<f64> {
            match law.kind {
                MechanicsLawKind::NewtonSecondLaw => match request.requested_output.as_str() {
                    "F_net" => Some(get("m")? * get("a")?),
                    "m" => Some(get("F_net")? / get("a")?),
                    "a" => Some(get("F_net")? / get("m")?),
                    _ => None,
                },
                MechanicsLawKind::Momentum => match request.requested_output.as_str() {
                    "p" => Some(get("m")? * get("v")?),
                    "m" => Some(get("p")? / get("v")?),
                    "v" => Some(get("p")? / get("m")?),
                    _ => None,
                },
                MechanicsLawKind::KineticEnergy => match request.requested_output.as_str() {
                    "K" => Some(0.5 * get("m")? * get("v")?.powi(2)),
                    "m" => Some(2.0 * get("K")? / get("v")?.powi(2)),
                    "v" => Some((2.0 * get("K")? / get("m")?).sqrt()),
                    _ => None,
                },
                MechanicsLawKind::HookeForce => match request.requested_output.as_str() {
                    "F_spring" => Some(-get("k")? * get("x")?),
                    "k" => Some(-get("F_spring")? / get("x")?),
                    "x" => Some(-get("F_spring")? / get("k")?),
                    _ => None,
                },
                MechanicsLawKind::ElasticPotentialEnergy => match request.requested_output.as_str()
                {
                    "U" => Some(0.5 * get("k")? * get("x")?.powi(2)),
                    "k" => Some(2.0 * get("U")? / get("x")?.powi(2)),
                    "x" => Some((2.0 * get("U")? / get("k")?).sqrt()),
                    _ => None,
                },
            }
        };
        compute()
    } else {
        None
    };
    if status == MechanicsStatus::Complete && value.is_none() {
        reasons.push("requested output is not supported by the law relation".into());
    }
    let status = if status == MechanicsStatus::Complete && value.is_none() {
        MechanicsStatus::Unsupported
    } else {
        status
    };
    let law_id = law.map(|record| record.law_id.clone());
    let equation = law.map(|record| record.equation.clone());
    let assumptions = law
        .map(|record| record.assumptions.clone())
        .unwrap_or_default();
    let source = law.map(|record| record.source.clone());
    let replay_hash = digest(&(
        &status,
        &value,
        &law_id,
        &equation,
        &assumptions,
        &source,
        &reasons,
    ));
    MechanicsEvaluationResult {
        status,
        value,
        law_id,
        equation,
        assumptions,
        source,
        reasons,
        replay_hash,
    }
}

pub fn replay_mechanics(result: &MechanicsEvaluationResult) -> bool {
    digest(&(
        &result.status,
        &result.value,
        &result.law_id,
        &result.equation,
        &result.assumptions,
        &result.source,
        &result.reasons,
    )) == result.replay_hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_pack_computes_and_replays_newton_case() {
        let result = evaluate_mechanics(
            &MechanicsEvaluationRequest {
                law_id: "Newton's second law".into(),
                bindings: vec![
                    NumericBinding {
                        symbol: "F_net".into(),
                        value: 12.0,
                        unit: "N".into(),
                        provenance: "exercise:F".into(),
                    },
                    NumericBinding {
                        symbol: "m".into(),
                        value: 3.0,
                        unit: "kg".into(),
                        provenance: "exercise:m".into(),
                    },
                ],
                requested_output: "a".into(),
            },
            &classical_mechanics_pack(),
        );
        assert_eq!(result.status, MechanicsStatus::Complete);
        assert_eq!(result.value, Some(4.0));
        assert!(replay_mechanics(&result));
    }
}
