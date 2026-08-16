//! Source-derived bounded science-law pack.
//!
//! Laws are declarative records; one generic rational expression evaluator
//! executes them. Unit scope and assumptions remain explicit and no law is
//! inferred from a nearby keyword.

use crate::probability_pack::Rational;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScienceSource {
    pub source_id: String,
    pub title: String,
    pub section: String,
    pub url: String,
    pub license: String,
    pub retrieved_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LawExpr {
    Input(String),
    Constant(i128),
    Add(Box<LawExpr>, Box<LawExpr>),
    Sub(Box<LawExpr>, Box<LawExpr>),
    Mul(Box<LawExpr>, Box<LawExpr>),
    Div(Box<LawExpr>, Box<LawExpr>),
    Neg(Box<LawExpr>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScienceLawRecord {
    pub law_id: String,
    pub aliases: Vec<String>,
    pub expression: LawExpr,
    pub required_inputs: Vec<String>,
    pub assumptions: Vec<String>,
    pub source: ScienceSource,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScienceStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidDomain,
    Inconsistent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScienceRequest {
    pub law: String,
    pub inputs: BTreeMap<String, Rational>,
    pub domain: String,
    pub unit_scope: String,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScienceResult {
    pub status: ScienceStatus,
    pub law_id: Option<String>,
    pub value: Option<Rational>,
    pub assumptions: Vec<String>,
    pub source: Option<ScienceSource>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn source() -> ScienceSource {
    ScienceSource {
        source_id: "openstax-university-physics:classical-science-laws".into(),
        title: "University Physics Volume 1".into(),
        section: "Thermodynamics and Classical Mechanics".into(),
        url: "https://openstax.org/details/books/university-physics-volume-1".into(),
        license: "CC BY 4.0; OpenStax attribution required".into(),
        retrieved_utc: "2026-08-16".into(),
    }
}

fn laws() -> Vec<ScienceLawRecord> {
    let cited = source();
    let input = |name: &str| LawExpr::Input(name.into());
    vec![
        ScienceLawRecord {
            law_id: "ideal_gas_pressure".into(),
            aliases: vec!["ideal gas law pressure".into()],
            expression: LawExpr::Div(
                Box::new(LawExpr::Mul(
                    Box::new(LawExpr::Mul(Box::new(input("n")), Box::new(input("R")))),
                    Box::new(input("T")),
                )),
                Box::new(input("V")),
            ),
            required_inputs: vec!["n".into(), "R".into(), "T".into(), "V".into()],
            assumptions: vec!["ideal gas approximation".into(), "V is nonzero".into()],
            source: cited.clone(),
        },
        ScienceLawRecord {
            law_id: "first_law_delta_u".into(),
            aliases: vec!["first law internal energy change".into()],
            expression: LawExpr::Sub(Box::new(input("Q")), Box::new(input("W"))),
            required_inputs: vec!["Q".into(), "W".into()],
            assumptions: vec!["sign convention Q into system and W by system".into()],
            source: cited.clone(),
        },
        ScienceLawRecord {
            law_id: "kinetic_energy".into(),
            aliases: vec!["classical kinetic energy".into()],
            expression: LawExpr::Div(
                Box::new(LawExpr::Mul(
                    Box::new(input("m")),
                    Box::new(LawExpr::Mul(Box::new(input("v")), Box::new(input("v")))),
                )),
                Box::new(LawExpr::Constant(2)),
            ),
            required_inputs: vec!["m".into(), "v".into()],
            assumptions: vec!["classical nonrelativistic speed".into()],
            source: cited.clone(),
        },
        ScienceLawRecord {
            law_id: "hooke_force".into(),
            aliases: vec!["linear spring force".into()],
            expression: LawExpr::Neg(Box::new(LawExpr::Mul(
                Box::new(input("k")),
                Box::new(input("x")),
            ))),
            required_inputs: vec!["k".into(), "x".into()],
            assumptions: vec!["linear spring regime".into()],
            source: cited,
        },
    ]
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn eval(expr: &LawExpr, inputs: &BTreeMap<String, Rational>) -> Option<Rational> {
    match expr {
        LawExpr::Input(name) => inputs.get(name).cloned(),
        LawExpr::Constant(value) => Rational::new(*value, 1),
        LawExpr::Add(left, right) => eval(left, inputs)?.add(&eval(right, inputs)?),
        LawExpr::Sub(left, right) => eval(left, inputs)?.sub(&eval(right, inputs)?),
        LawExpr::Mul(left, right) => eval(left, inputs)?.mul(&eval(right, inputs)?),
        LawExpr::Div(left, right) => eval(left, inputs)?.div(&eval(right, inputs)?),
        LawExpr::Neg(value) => {
            let value = eval(value, inputs)?;
            Rational::new(-value.numerator, value.denominator)
        }
    }
}

fn payload(result: &ScienceResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.law_id,
        &result.value,
        &result.assumptions,
        &result.source,
        &result.reasons,
        &result.provenance,
    )
}

/// Evaluate a source-derived science law through the generic interpreter.
pub fn evaluate_science(request: &ScienceRequest) -> ScienceResult {
    let mut output = ScienceResult {
        status: ScienceStatus::Missing,
        law_id: None,
        value: None,
        assumptions: Vec::new(),
        source: None,
        reasons: Vec::new(),
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    if request.domain != "source_derived_classical_science"
        || request.unit_scope != "si_consistent_exact"
    {
        output.status = ScienceStatus::InvalidDomain;
        output
            .reasons
            .push("domain or unit scope is outside the source-derived science pack".into());
    } else if let Some(ambiguity) = &request.ambiguity {
        output.status = ScienceStatus::Ambiguous;
        output.reasons.push(ambiguity.clone());
    } else {
        let matches: Vec<ScienceLawRecord> = laws()
            .into_iter()
            .filter(|law| {
                law.law_id == request.law || law.aliases.iter().any(|alias| alias == &request.law)
            })
            .collect();
        if matches.len() != 1 {
            output.status = if matches.is_empty() {
                ScienceStatus::Missing
            } else {
                ScienceStatus::Ambiguous
            };
            output
                .reasons
                .push("law identifier does not select one source record".into());
        } else {
            let law = &matches[0];
            output.law_id = Some(law.law_id.clone());
            output.assumptions = law.assumptions.clone();
            output.source = Some(law.source.clone());
            if law
                .required_inputs
                .iter()
                .any(|input| !request.inputs.contains_key(input))
            {
                output.status = ScienceStatus::Missing;
                output.reasons.push("required law input is absent".into());
            } else if law.law_id == "ideal_gas_pressure"
                && request.inputs.get("V") == Some(&Rational::zero())
            {
                output.status = ScienceStatus::Inconsistent;
                output.reasons.push("volume must be nonzero".into());
            } else {
                output.value = eval(&law.expression, &request.inputs);
                output.status = if output.value.is_some() {
                    ScienceStatus::Complete
                } else {
                    ScienceStatus::Unsupported
                };
            }
        }
    }
    let replay_hash = digest(&payload(&output));
    output.replay_hash = replay_hash;
    output
}

impl ScienceResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != ScienceStatus::Complete
                || (self.value.is_some() && self.source.is_some()))
    }
}
