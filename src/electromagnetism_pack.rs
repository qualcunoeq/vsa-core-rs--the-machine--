//! Source-derived bounded electromagnetism law pack.
//!
//! The catalog is declarative and all four laws use one generic expression
//! interpreter.  This shadow pack records assumptions and source evidence but
//! does not infer circuit behavior, signs, units, or missing quantities.

use crate::probability_pack::Rational;
use crate::science_law_pack::{LawExpr, ScienceSource};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmLawRecord {
    pub law_id: String,
    pub aliases: Vec<String>,
    pub expression: LawExpr,
    pub required_inputs: Vec<String>,
    pub assumptions: Vec<String>,
    pub source: ScienceSource,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EmStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidDomain,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmRequest {
    pub law: String,
    pub inputs: BTreeMap<String, Rational>,
    pub domain: String,
    pub unit_scope: String,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmResult {
    pub status: EmStatus,
    pub law_id: Option<String>,
    pub value: Option<Rational>,
    pub assumptions: Vec<String>,
    pub source: Option<ScienceSource>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn catalog() -> Vec<EmLawRecord> {
    let records: Vec<EmLawRecord> = serde_json::from_str(include_str!(
        "../docs/sources/openstax_bounded_electromagnetism_catalog.json"
    ))
    .expect("electromagnetism catalog must be valid JSON");
    validate_catalog(&records).expect("electromagnetism catalog must validate");
    records
}

pub fn validate_catalog(records: &[EmLawRecord]) -> Result<(), Vec<String>> {
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
        if record.required_inputs.is_empty() {
            errors.push(format!("{} declares no inputs", record.law_id));
        }
        let required: std::collections::BTreeSet<_> = record.required_inputs.iter().collect();
        if required.len() != record.required_inputs.len() {
            errors.push(format!("{} repeats an input", record.law_id));
        }
        for alias in &record.aliases {
            if alias.trim().is_empty() || !aliases.insert(alias.clone()) {
                errors.push(format!("duplicate or empty alias in {}", record.law_id));
            }
        }
        let mut expression_inputs = Vec::new();
        collect_inputs(&record.expression, &mut expression_inputs);
        if expression_inputs
            .iter()
            .any(|input| !required.contains(input))
        {
            errors.push(format!("{} uses an undeclared input", record.law_id));
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

fn eval(expression: &LawExpr, inputs: &BTreeMap<String, Rational>) -> Option<Rational> {
    match expression {
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

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn payload(result: &EmResult) -> impl Serialize + '_ {
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

pub fn evaluate(request: &EmRequest) -> EmResult {
    let mut result = EmResult {
        status: EmStatus::Missing,
        law_id: None,
        value: None,
        assumptions: Vec::new(),
        source: None,
        reasons: Vec::new(),
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    if request.provenance.is_empty() {
        result.status = EmStatus::Missing;
        result
            .reasons
            .push("source provenance is required before authorization".into());
    } else if request.domain != "source_derived_bounded_electromagnetism"
        || request.unit_scope != "si_consistent_exact"
    {
        result.status = EmStatus::InvalidDomain;
        result
            .reasons
            .push("domain or unit scope is outside the bounded electromagnetism pack".into());
    } else if let Some(ambiguity) = &request.ambiguity {
        result.status = EmStatus::Ambiguous;
        result.reasons.push(ambiguity.clone());
    } else {
        let matches: Vec<_> = catalog()
            .into_iter()
            .filter(|law| {
                law.law_id == request.law || law.aliases.iter().any(|alias| alias == &request.law)
            })
            .collect();
        if matches.len() != 1 {
            result.status = if matches.is_empty() {
                EmStatus::Missing
            } else {
                EmStatus::Ambiguous
            };
            result
                .reasons
                .push("law identifier does not select one source record".into());
        } else {
            let law = &matches[0];
            result.law_id = Some(law.law_id.clone());
            result.assumptions = law.assumptions.clone();
            result.source = Some(law.source.clone());
            if law
                .required_inputs
                .iter()
                .any(|input| !request.inputs.contains_key(input))
            {
                result.status = EmStatus::Missing;
                result.reasons.push("required law input is absent".into());
            } else {
                result.value = eval(&law.expression, &request.inputs);
                result.status = if result.value.is_some() {
                    EmStatus::Complete
                } else {
                    EmStatus::Unsupported
                };
            }
        }
    }
    result.replay_hash = digest(&(
        result.status,
        &result.law_id,
        &result.value,
        &result.assumptions,
        &result.source,
        &result.reasons,
        &result.provenance,
    ));
    result
}

impl EmResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != EmStatus::Complete
                || (self.value.is_some() && self.source.is_some()))
    }

    pub fn authorized(&self) -> bool {
        self.status == EmStatus::Complete && self.replay_verified()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_source_complete() {
        let records = catalog();
        assert_eq!(records.len(), 4);
        assert!(records
            .iter()
            .all(|record| !record.source.evidence_span.is_empty()));
    }

    #[test]
    fn missing_input_is_fail_closed() {
        let result = evaluate(&EmRequest {
            law: "ohms_law_voltage".into(),
            inputs: BTreeMap::from([(String::from("I"), Rational::new(2, 1).unwrap())]),
            domain: "source_derived_bounded_electromagnetism".into(),
            unit_scope: "si_consistent_exact".into(),
            ambiguity: None,
            provenance: vec!["em-test".into()],
        });
        assert_eq!(result.status, EmStatus::Missing);
        assert!(result.replay_verified());
    }
}
