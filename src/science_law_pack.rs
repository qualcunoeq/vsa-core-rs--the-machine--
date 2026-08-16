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
    pub evidence_span: String,
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
    pub constraints: Vec<ScienceConstraint>,
    pub source: ScienceSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScienceConstraint {
    NotEqualInteger(String, i128),
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

fn laws() -> Vec<ScienceLawRecord> {
    let records: Vec<ScienceLawRecord> = serde_json::from_str(include_str!(
        "../docs/sources/openstax_classical_science_catalog.json"
    ))
    .expect("source science catalog must be valid JSON");
    validate_science_law_records(&records).expect("source science catalog must validate");
    records
}

/// Validate a source-derived science catalog without interpreting the meaning
/// of any particular law.  Execution remains a generic expression walk.
pub fn validate_science_law_records(records: &[ScienceLawRecord]) -> Result<(), Vec<String>> {
    fn collect_inputs(expression: &LawExpr, names: &mut Vec<String>) {
        match expression {
            LawExpr::Input(name) => names.push(name.clone()),
            LawExpr::Constant(_) => {}
            LawExpr::Add(left, right)
            | LawExpr::Sub(left, right)
            | LawExpr::Mul(left, right)
            | LawExpr::Div(left, right) => {
                collect_inputs(left, names);
                collect_inputs(right, names);
            }
            LawExpr::Neg(value) => collect_inputs(value, names),
        }
    }

    let mut errors = Vec::new();
    let mut ids = std::collections::BTreeSet::new();
    let mut aliases = std::collections::BTreeSet::new();
    for record in records {
        if record.law_id.trim().is_empty() || !ids.insert(record.law_id.clone()) {
            errors.push(format!(
                "duplicate or empty law identifier: {}",
                record.law_id
            ));
        }
        let required: std::collections::BTreeSet<_> = record.required_inputs.iter().collect();
        if required.len() != record.required_inputs.len() || required.is_empty() {
            errors.push(format!(
                "invalid required-input declaration: {}",
                record.law_id
            ));
        }
        for alias in &record.aliases {
            if alias.trim().is_empty() || !aliases.insert(alias.clone()) {
                errors.push(format!("duplicate or empty alias in {}", record.law_id));
            }
        }
        let mut expression_inputs = Vec::new();
        collect_inputs(&record.expression, &mut expression_inputs);
        for input in expression_inputs {
            if !required.contains(&input) {
                errors.push(format!("{} uses undeclared input {}", record.law_id, input));
            }
        }
        for constraint in &record.constraints {
            let name = match constraint {
                ScienceConstraint::NotEqualInteger(name, _) => name,
            };
            if !required.contains(name) {
                errors.push(format!(
                    "{} constrains undeclared input {}",
                    record.law_id, name
                ));
            }
        }
        let source = &record.source;
        if source.source_id.trim().is_empty()
            || source.title.trim().is_empty()
            || source.section.trim().is_empty()
            || !source.url.starts_with("https://")
            || source.license.trim().is_empty()
            || source.retrieved_utc.trim().is_empty()
            || source.evidence_span.trim().is_empty()
        {
            errors.push(format!("{} has incomplete source evidence", record.law_id));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
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
            } else if law.constraints.iter().any(|constraint| match constraint {
                ScienceConstraint::NotEqualInteger(name, forbidden) => request
                    .inputs
                    .get(name)
                    .is_some_and(|value| value.denominator == 1 && value.numerator == *forbidden),
            }) {
                output.status = ScienceStatus::Inconsistent;
                output
                    .reasons
                    .push("a declared input constraint is violated".into());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_catalog_validates_and_preserves_evidence() {
        let records: Vec<ScienceLawRecord> = serde_json::from_str(include_str!(
            "../docs/sources/openstax_classical_science_catalog.json"
        ))
        .unwrap();
        assert_eq!(records.len(), 4);
        assert!(validate_science_law_records(&records).is_ok());
        assert!(records.iter().all(|record| {
            !record.source.evidence_span.is_empty() && !record.source.url.is_empty()
        }));
    }

    #[test]
    fn catalog_constraint_rejects_zero_volume() {
        let result = evaluate_science(&ScienceRequest {
            law: "ideal_gas_pressure".into(),
            inputs: BTreeMap::from([
                ("n".into(), Rational::new(1, 1).unwrap()),
                ("R".into(), Rational::new(8, 1).unwrap()),
                ("T".into(), Rational::new(300, 1).unwrap()),
                ("V".into(), Rational::zero()),
            ]),
            domain: "source_derived_classical_science".into(),
            unit_scope: "si_consistent_exact".into(),
            ambiguity: None,
            provenance: vec!["science-catalog-test".into()],
        });
        assert_eq!(result.status, ScienceStatus::Inconsistent);
        assert!(result.replay_verified());
    }
}
