//! Source-derived declarative formula pack.
//!
//! Formula records are data acquired from a cited source. A single generic
//! rational expression interpreter executes every record; there are no
//! formula-specific evaluator branches. The pack is shadow-only.

use crate::probability_pack::Rational;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceCitation {
    pub source_id: String,
    pub title: String,
    pub section: String,
    pub url: String,
    pub license: String,
    pub retrieved_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Expr {
    Input(String),
    Constant(i128),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    PowNatural(Box<Expr>, u32),
    PowInput(Box<Expr>, String),
    PowInputMinusOne(Box<Expr>, String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InputConstraint {
    Positive(String),
    PositiveInteger(String),
    NonnegativeInteger(String),
    Probability(String),
    NotEqualInteger(String, i128),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormulaRecord {
    pub formula_id: String,
    pub aliases: Vec<String>,
    pub expression: Expr,
    pub required_inputs: Vec<String>,
    pub assumptions: Vec<String>,
    pub constraints: Vec<InputConstraint>,
    pub source: SourceCitation,
}

/// Validate a declarative source catalog before it is eligible for shadow
/// execution.  The validator is domain-agnostic: it checks identity,
/// expression inputs, constraints, and citation completeness without knowing
/// what any formula means.
pub fn validate_formula_records(records: &[FormulaRecord]) -> Result<(), Vec<String>> {
    fn collect_inputs(expression: &Expr, names: &mut Vec<String>) {
        match expression {
            Expr::Input(name) => names.push(name.clone()),
            Expr::Constant(_) => {}
            Expr::Add(left, right)
            | Expr::Sub(left, right)
            | Expr::Mul(left, right)
            | Expr::Div(left, right) => {
                collect_inputs(left, names);
                collect_inputs(right, names);
            }
            Expr::PowNatural(base, _)
            | Expr::PowInput(base, _)
            | Expr::PowInputMinusOne(base, _) => collect_inputs(base, names),
        }
    }

    let mut errors = Vec::new();
    let mut formula_ids = std::collections::BTreeSet::new();
    let mut aliases = std::collections::BTreeSet::new();
    for record in records {
        if record.formula_id.trim().is_empty() {
            errors.push("formula identifier is empty".into());
        }
        if !formula_ids.insert(record.formula_id.clone()) {
            errors.push(format!(
                "duplicate formula identifier: {}",
                record.formula_id
            ));
        }
        if record.required_inputs.is_empty() {
            errors.push(format!(
                "formula {} declares no required inputs",
                record.formula_id
            ));
        }
        let required: std::collections::BTreeSet<_> = record.required_inputs.iter().collect();
        if required.len() != record.required_inputs.len() {
            errors.push(format!(
                "formula {} repeats a required input",
                record.formula_id
            ));
        }
        let mut expression_inputs = Vec::new();
        collect_inputs(&record.expression, &mut expression_inputs);
        for input in expression_inputs {
            if !required.contains(&input) {
                errors.push(format!(
                    "formula {} uses undeclared input {}",
                    record.formula_id, input
                ));
            }
        }
        for alias in &record.aliases {
            if alias.trim().is_empty() || !aliases.insert(alias.clone()) {
                errors.push(format!("duplicate or empty alias in {}", record.formula_id));
            }
        }
        for constraint in &record.constraints {
            let name = match constraint {
                InputConstraint::Positive(name)
                | InputConstraint::PositiveInteger(name)
                | InputConstraint::NonnegativeInteger(name)
                | InputConstraint::Probability(name)
                | InputConstraint::NotEqualInteger(name, _) => name,
            };
            if !required.contains(name) {
                errors.push(format!(
                    "formula {} constrains undeclared input {}",
                    record.formula_id, name
                ));
            }
        }
        if record.source.source_id.trim().is_empty()
            || record.source.title.trim().is_empty()
            || record.source.section.trim().is_empty()
            || !record.source.url.starts_with("https://")
            || record.source.retrieved_utc.trim().is_empty()
        {
            errors.push(format!(
                "formula {} has incomplete source citation",
                record.formula_id
            ));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FormulaStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidDomain,
    Inconsistent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormulaRequest {
    pub formula: String,
    pub inputs: BTreeMap<String, Rational>,
    pub domain: String,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FormulaResult {
    pub status: FormulaStatus,
    pub formula_id: Option<String>,
    pub value: Option<Rational>,
    pub assumptions: Vec<String>,
    pub source: Option<SourceCitation>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn source() -> SourceCitation {
    SourceCitation {
        source_id: "openstax-precalculus-2e:sequences-series".into(),
        title: "Precalculus 2e".into(),
        section: "Sequences, Series, and the Binomial Theorem".into(),
        url: "https://openstax.org/details/books/precalculus-2e".into(),
        license: "CC BY 4.0; OpenStax attribution required".into(),
        retrieved_utc: "2026-08-16".into(),
    }
}

fn formulas() -> Vec<FormulaRecord> {
    let cited = source();
    let input = |name: &str| Expr::Input(name.into());
    vec![
        FormulaRecord {
            formula_id: "arithmetic_nth_term".into(),
            aliases: vec!["arithmetic sequence term".into(), "affine sequence".into()],
            expression: Expr::Add(
                Box::new(input("a1")),
                Box::new(Expr::Mul(
                    Box::new(Expr::Sub(Box::new(input("n")), Box::new(Expr::Constant(1)))),
                    Box::new(input("d")),
                )),
            ),
            required_inputs: vec!["a1".into(), "n".into(), "d".into()],
            assumptions: vec!["n is a positive integer".into()],
            constraints: vec![InputConstraint::PositiveInteger("n".into())],
            source: cited.clone(),
        },
        FormulaRecord {
            formula_id: "arithmetic_partial_sum".into(),
            aliases: vec!["arithmetic series sum".into()],
            expression: Expr::Div(
                Box::new(Expr::Mul(
                    Box::new(input("n")),
                    Box::new(Expr::Add(
                        Box::new(Expr::Mul(
                            Box::new(Expr::Constant(2)),
                            Box::new(input("a1")),
                        )),
                        Box::new(Expr::Mul(
                            Box::new(Expr::Sub(Box::new(input("n")), Box::new(Expr::Constant(1)))),
                            Box::new(input("d")),
                        )),
                    )),
                )),
                Box::new(Expr::Constant(2)),
            ),
            required_inputs: vec!["a1".into(), "n".into(), "d".into()],
            assumptions: vec!["n is a positive integer".into()],
            constraints: vec![InputConstraint::PositiveInteger("n".into())],
            source: cited.clone(),
        },
        FormulaRecord {
            formula_id: "geometric_nth_term".into(),
            aliases: vec!["geometric sequence term".into()],
            expression: Expr::Mul(
                Box::new(input("a1")),
                Box::new(Expr::PowInputMinusOne(Box::new(input("r")), "n".into())),
            ),
            required_inputs: vec!["a1".into(), "n".into(), "r".into()],
            assumptions: vec!["n is a positive integer; exponent is n-1".into()],
            constraints: vec![InputConstraint::PositiveInteger("n".into())],
            source: cited.clone(),
        },
        FormulaRecord {
            formula_id: "geometric_partial_sum".into(),
            aliases: vec!["geometric series sum".into()],
            expression: Expr::Div(
                Box::new(Expr::Mul(
                    Box::new(input("a1")),
                    Box::new(Expr::Sub(
                        Box::new(Expr::PowInput(Box::new(input("r")), "n".into())),
                        Box::new(Expr::Constant(1)),
                    )),
                )),
                Box::new(Expr::Sub(Box::new(input("r")), Box::new(Expr::Constant(1)))),
            ),
            required_inputs: vec!["a1".into(), "n".into(), "r".into()],
            assumptions: vec!["n is a positive integer; r is not 1".into()],
            constraints: vec![
                InputConstraint::PositiveInteger("n".into()),
                InputConstraint::NotEqualInteger("r".into(), 1),
            ],
            source: cited,
        },
    ]
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn eval(expr: &Expr, inputs: &BTreeMap<String, Rational>) -> Option<Rational> {
    match expr {
        Expr::Input(name) => inputs.get(name).cloned(),
        Expr::Constant(value) => Rational::new(*value, 1),
        Expr::Add(left, right) => eval(left, inputs)?.add(&eval(right, inputs)?),
        Expr::Sub(left, right) => eval(left, inputs)?.sub(&eval(right, inputs)?),
        Expr::Mul(left, right) => eval(left, inputs)?.mul(&eval(right, inputs)?),
        Expr::Div(left, right) => eval(left, inputs)?.div(&eval(right, inputs)?),
        Expr::PowNatural(base, exponent) => {
            let mut value = Rational::one();
            let base = eval(base, inputs)?;
            for _ in 0..*exponent {
                value = value.mul(&base)?;
            }
            Some(value)
        }
        Expr::PowInput(base, input) => {
            let exponent = inputs.get(input)?;
            if exponent.denominator != 1 || exponent.numerator < 0 {
                return None;
            }
            let mut value = Rational::one();
            let base = eval(base, inputs)?;
            for _ in 0..exponent.numerator as u32 {
                value = value.mul(&base)?;
            }
            Some(value)
        }
        Expr::PowInputMinusOne(base, input) => {
            let exponent = inputs.get(input)?;
            if exponent.denominator != 1 || exponent.numerator < 1 {
                return None;
            }
            let mut value = Rational::one();
            let base = eval(base, inputs)?;
            for _ in 0..(exponent.numerator as u32 - 1) {
                value = value.mul(&base)?;
            }
            Some(value)
        }
    }
}

fn constraints_satisfied(record: &FormulaRecord, inputs: &BTreeMap<String, Rational>) -> bool {
    record
        .constraints
        .iter()
        .all(|constraint| match constraint {
            InputConstraint::Positive(name) => inputs.get(name).is_some_and(Rational::positive),
            InputConstraint::PositiveInteger(name) => inputs
                .get(name)
                .is_some_and(|value| value.denominator == 1 && value.numerator > 0),
            InputConstraint::NonnegativeInteger(name) => inputs
                .get(name)
                .is_some_and(|value| value.denominator == 1 && value.numerator >= 0),
            InputConstraint::Probability(name) => {
                inputs.get(name).is_some_and(Rational::in_unit_interval)
            }
            InputConstraint::NotEqualInteger(name, forbidden) => inputs
                .get(name)
                .is_some_and(|value| value.denominator != 1 || value.numerator != *forbidden),
        })
}

fn payload(result: &FormulaResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.formula_id,
        &result.value,
        &result.assumptions,
        &result.source,
        &result.reasons,
        &result.provenance,
    )
}

/// Evaluate a source-derived formula through the generic expression runtime.
pub fn evaluate_formula(request: &FormulaRequest) -> FormulaResult {
    let pack = formulas();
    let mut output = FormulaResult {
        status: FormulaStatus::Missing,
        formula_id: None,
        value: None,
        assumptions: Vec::new(),
        source: None,
        reasons: Vec::new(),
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    if request.domain != "source_derived_sequences_series" {
        output.status = FormulaStatus::InvalidDomain;
        output
            .reasons
            .push("domain is outside the source-derived formula pack".into());
    } else if let Some(ambiguity) = &request.ambiguity {
        output.status = FormulaStatus::Ambiguous;
        output.reasons.push(ambiguity.clone());
    } else {
        let matches: Vec<&FormulaRecord> = pack
            .iter()
            .filter(|record| {
                record.formula_id == request.formula
                    || record.aliases.iter().any(|alias| alias == &request.formula)
            })
            .collect();
        if matches.len() != 1 {
            output.status = if matches.is_empty() {
                FormulaStatus::Missing
            } else {
                FormulaStatus::Ambiguous
            };
            output
                .reasons
                .push("formula identifier does not select one source record".into());
        } else {
            let record = matches[0];
            output.formula_id = Some(record.formula_id.clone());
            output.assumptions = record.assumptions.clone();
            output.source = Some(record.source.clone());
            if record
                .required_inputs
                .iter()
                .any(|name| !request.inputs.contains_key(name))
            {
                output.status = FormulaStatus::Missing;
                output
                    .reasons
                    .push("required source-formula input is absent".into());
            } else if !constraints_satisfied(record, &request.inputs) {
                output.status = FormulaStatus::Inconsistent;
                output
                    .reasons
                    .push("source-record input constraints are not satisfied".into());
            } else {
                output.value = eval(&record.expression, &request.inputs);
                output.status = if output.value.is_some() {
                    FormulaStatus::Complete
                } else {
                    FormulaStatus::Unsupported
                };
            }
        }
    }
    let replay_hash = digest(&payload(&output));
    output.replay_hash = replay_hash;
    output
}

/// Evaluate an independently supplied declarative catalog through the same
/// generic expression runtime. The catalog is data: no formula identifier is
/// interpreted by a capability-specific execution branch.
pub fn evaluate_formula_records(
    request: &FormulaRequest,
    expected_domain: &str,
    records: &[FormulaRecord],
) -> FormulaResult {
    let mut output = FormulaResult {
        status: FormulaStatus::Missing,
        formula_id: None,
        value: None,
        assumptions: Vec::new(),
        source: None,
        reasons: Vec::new(),
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    if request.domain != expected_domain {
        output.status = FormulaStatus::InvalidDomain;
        output
            .reasons
            .push("domain is outside the supplied source catalog".into());
    } else if let Some(ambiguity) = &request.ambiguity {
        output.status = FormulaStatus::Ambiguous;
        output.reasons.push(ambiguity.clone());
    } else {
        let matches: Vec<&FormulaRecord> = records
            .iter()
            .filter(|record| {
                record.formula_id == request.formula
                    || record.aliases.iter().any(|alias| alias == &request.formula)
            })
            .collect();
        if matches.len() != 1 {
            output.status = if matches.is_empty() {
                FormulaStatus::Missing
            } else {
                FormulaStatus::Ambiguous
            };
            output
                .reasons
                .push("catalog identifier does not select one source record".into());
        } else {
            let record = matches[0];
            output.formula_id = Some(record.formula_id.clone());
            output.assumptions = record.assumptions.clone();
            output.source = Some(record.source.clone());
            if record
                .required_inputs
                .iter()
                .any(|name| !request.inputs.contains_key(name))
            {
                output.status = FormulaStatus::Missing;
                output
                    .reasons
                    .push("required catalog input is absent".into());
            } else if !constraints_satisfied(record, &request.inputs) {
                output.status = FormulaStatus::Inconsistent;
                output
                    .reasons
                    .push("source-record input constraints are not satisfied".into());
            } else {
                output.value = eval(&record.expression, &request.inputs);
                output.status = if output.value.is_some() {
                    FormulaStatus::Complete
                } else {
                    FormulaStatus::Unsupported
                };
            }
        }
    }
    let replay_hash = digest(&payload(&output));
    output.replay_hash = replay_hash;
    output
}

impl FormulaResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != FormulaStatus::Complete
                || (self.value.is_some() && self.source.is_some()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn citation() -> SourceCitation {
        SourceCitation {
            source_id: "test-source".into(),
            title: "Test source".into(),
            section: "Test section".into(),
            url: "https://example.invalid/source".into(),
            license: "test".into(),
            retrieved_utc: "2026-08-16".into(),
        }
    }

    #[test]
    fn catalog_validator_accepts_well_formed_records() {
        let records = vec![FormulaRecord {
            formula_id: "ratio".into(),
            aliases: vec!["quotient".into()],
            expression: Expr::Div(
                Box::new(Expr::Input("a".into())),
                Box::new(Expr::Input("b".into())),
            ),
            required_inputs: vec!["a".into(), "b".into()],
            assumptions: vec!["b is nonzero".into()],
            constraints: vec![InputConstraint::NotEqualInteger("b".into(), 0)],
            source: citation(),
        }];
        assert!(validate_formula_records(&records).is_ok());
    }

    #[test]
    fn catalog_validator_rejects_undeclared_expression_inputs() {
        let records = vec![FormulaRecord {
            formula_id: "bad".into(),
            aliases: Vec::new(),
            expression: Expr::Input("missing".into()),
            required_inputs: vec!["declared".into()],
            assumptions: Vec::new(),
            constraints: Vec::new(),
            source: citation(),
        }];
        let errors = validate_formula_records(&records).unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.contains("undeclared input")));
    }
}
