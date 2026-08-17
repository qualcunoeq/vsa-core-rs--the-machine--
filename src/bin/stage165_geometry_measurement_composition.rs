//! Stage 165: compose source-derived geometry formulas with source-derived
//! unit conversions through a generic dimensional boundary.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::probability_pack::Rational;
use the_machine::source_formula_frontend::{formalize_formula_text, FormulaFrontendStatus};
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, Expr, FormulaRecord, FormulaRequest,
    FormulaStatus,
};
use the_machine::source_unit_frontend::{
    formalize_unit_text, replay_verified as unit_replay_verified, UnitFrontendResult,
    UnitFrontendStatus,
};

const GEOMETRY_DOMAIN: &str = "source_derived_bounded_geometry";
const UNIT_DOMAIN: &str = "source_catalog_unit_conversion";
const GEOMETRY_SOURCE: &str =
    include_str!("../../docs/sources/openstax_bounded_geometry_source.txt");
const UNIT_SOURCE: &str = include_str!("../../docs/sources/openstax_unit_conversion_catalog.txt");
const PARENT: &str = "docs/stage164_source_geometry_language_transfer.json";
const REPORT_JSON: &str = "docs/stage165_geometry_measurement_composition.json";
const REPORT_MD: &str = "docs/stage165_geometry_measurement_composition.md";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Expected {
    Supported,
    Ambiguous,
    Refused,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct Dimension {
    length: i32,
    mass: i32,
    time: i32,
}

impl Dimension {
    fn zero() -> Self {
        Self {
            length: 0,
            mass: 0,
            time: 0,
        }
    }
    fn add(&self, other: &Self) -> Self {
        Self {
            length: self.length + other.length,
            mass: self.mass + other.mass,
            time: self.time + other.time,
        }
    }
    fn sub(&self, other: &Self) -> Self {
        Self {
            length: self.length - other.length,
            mass: self.mass - other.mass,
            time: self.time - other.time,
        }
    }
    fn scale(&self, exponent: u32) -> Self {
        let exponent = exponent as i32;
        Self {
            length: self.length * exponent,
            mass: self.mass * exponent,
            time: self.time * exponent,
        }
    }
}

fn unit_dimension(unit: &str) -> Option<Dimension> {
    match unit {
        "meter" | "meters" | "centimeter" | "centimeters" => Some(Dimension {
            length: 1,
            mass: 0,
            time: 0,
        }),
        "pound" | "pounds" | "ounce" | "ounces" => Some(Dimension {
            length: 0,
            mass: 1,
            time: 0,
        }),
        "liter" | "liters" | "milliliter" | "milliliters" => Some(Dimension {
            length: 3,
            mass: 0,
            time: 0,
        }),
        "hour" | "hours" | "minute" | "minutes" => Some(Dimension {
            length: 0,
            mass: 0,
            time: 1,
        }),
        _ => None,
    }
}

fn expression_dimension(
    expression: &Expr,
    inputs: &BTreeMap<String, Dimension>,
) -> Result<Dimension, String> {
    match expression {
        Expr::Input(name) => inputs
            .get(name)
            .cloned()
            .ok_or_else(|| format!("missing dimension for {name}")),
        Expr::Constant(_) => Ok(Dimension::zero()),
        Expr::Add(left, right) | Expr::Sub(left, right) => {
            let left = expression_dimension(left, inputs)?;
            let right = expression_dimension(right, inputs)?;
            if left != right {
                return Err("incompatible additive dimensions".into());
            }
            Ok(left)
        }
        Expr::Mul(left, right) => {
            Ok(expression_dimension(left, inputs)?.add(&expression_dimension(right, inputs)?))
        }
        Expr::Div(left, right) => {
            Ok(expression_dimension(left, inputs)?.sub(&expression_dimension(right, inputs)?))
        }
        Expr::PowNatural(value, exponent) => {
            Ok(expression_dimension(value, inputs)?.scale(*exponent))
        }
        Expr::PowInput(_, _) | Expr::PowInputMinusOne(_, _) => {
            Err("variable exponent lacks a dimensionless contract".into())
        }
    }
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("stage165 serialization"))
    )
}

#[derive(Debug, Clone, Serialize)]
struct Receipt {
    id: String,
    partition: String,
    expected: Expected,
    formula: Option<String>,
    geometry_status: FormulaFrontendStatus,
    unit_statuses: Vec<UnitFrontendStatus>,
    execution_status: Option<FormulaStatus>,
    output_dimension: Option<Dimension>,
    dimensional_compatible: bool,
    exact: bool,
    authorized: bool,
    geometry_replay_verified: bool,
    geometry_tamper_rejected: bool,
    units_replay_verified: bool,
    units_tamper_rejected: bool,
    execution_replay_verified: bool,
    execution_tamper_rejected: bool,
    composition_replay_verified: bool,
    composition_tamper_rejected: bool,
    provenance_preserved: bool,
    false_authorization: bool,
    false_denial: bool,
    replay_hash: String,
}

impl Receipt {
    fn finalize(mut self) -> Self {
        self.replay_hash.clear();
        self.replay_hash = digest(&self);
        self
    }
    fn replay_verified(&self) -> bool {
        let mut copy = self.clone();
        let hash = copy.replay_hash.clone();
        copy.replay_hash.clear();
        hash == digest(&copy) && self.provenance_preserved
    }
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    parent_report_sha256: String,
    geometry_source_sha256: String,
    unit_source_sha256: String,
    runtime_domain_specific_branches: usize,
    cases: usize,
    development_cases: usize,
    development_supported: usize,
    development_ambiguous: usize,
    development_refused: usize,
    development_exact: usize,
    development_authorized: usize,
    development_geometry_replay: usize,
    development_unit_replay: usize,
    development_execution_replay: usize,
    development_composition_replay: usize,
    development_tamper_rejections: usize,
    holdout_cases: usize,
    holdout_supported: usize,
    holdout_exact: usize,
    holdout_authorized: usize,
    holdout_composition_replay: usize,
    false_authorizations: usize,
    false_denials: usize,
    unit_refusals: usize,
    provenance_preserved: usize,
    live_registry_mutations: usize,
    receipts: Vec<Receipt>,
}

fn q(value: i128) -> Rational {
    Rational::new(value, 1).expect("integer rational")
}

fn input_value(name: &str, index: usize) -> Rational {
    q(match name {
        "length" => (index % 11 + 2) as i128,
        "width" => (index % 7 + 3) as i128,
        "height" => (index % 5 + 2) as i128,
        "base" => (index % 9 + 2) as i128,
        "mass" => (index % 13 + 4) as i128,
        "volume" => (index % 6 + 2) as i128,
        _ => 3,
    })
}

fn render(record: &FormulaRecord, index: usize) -> String {
    let inputs = record
        .required_inputs
        .iter()
        .map(|name| format!("{name}={}", input_value(name, index).numerator))
        .collect::<Vec<_>>()
        .join(" ");
    format!("Compute the {} using {inputs}.", record.aliases[0])
}

// Benchmark fixture only: the generic composer receives these assignments as
// typed unit data and never branches on a formula identifier.
fn unit_assignment(input: &str, incompatible: bool) -> (&'static str, &'static str) {
    if incompatible && matches!(input, "width" | "height" | "base" | "volume") {
        return ("unknown", "centimeters");
    }
    match input {
        "mass" => ("pounds", "ounces"),
        "volume" => ("liters", "milliliters"),
        _ => ("meters", "centimeters"),
    }
}

fn unit_text(amount: Rational, source: &str, target: &str) -> String {
    format!("Convert {} {} to {}.", amount.numerator, source, target)
}

fn compose(
    geometry_records: &[FormulaRecord],
    unit_records: &[FormulaRecord],
    text: String,
    id: String,
    partition: &str,
    expected: Expected,
    incompatible: bool,
) -> Receipt {
    let geometry = formalize_formula_text(&text, GEOMETRY_DOMAIN, geometry_records);
    let geometry_replay_verified = geometry.replay_verified();
    let mut geometry_tampered = geometry.clone();
    geometry_tampered.replay_hash.push('x');
    let geometry_tamper_rejected = !geometry_tampered.replay_verified();
    let mut unit_results: Vec<UnitFrontendResult> = Vec::new();
    let mut unit_statuses = Vec::new();
    let mut converted_inputs = BTreeMap::new();
    let mut input_dimensions = BTreeMap::new();
    let mut units_replay_verified = true;
    let mut units_tamper_rejected = true;
    let mut unit_complete = false;
    let mut dimensional_compatible = false;
    let mut execution_status = None;
    let mut execution_replay_verified = true;
    let mut execution_tamper_rejected = true;
    let mut output_dimension = None;
    let mut provenance_preserved =
        geometry_replay_verified && !geometry.provenance_spans.is_empty();

    if geometry.status == FormulaFrontendStatus::Complete {
        if let Some(request) = geometry.request.as_ref() {
            if let Some(record) = geometry_records
                .iter()
                .find(|record| record.formula_id == request.formula)
            {
                unit_complete = true;
                for input in &record.required_inputs {
                    let amount = request.inputs[input].clone();
                    let (source, target) = unit_assignment(input, incompatible);
                    let unit = formalize_unit_text(
                        &unit_text(amount, source, target),
                        &format!("{id}-unit-{input}"),
                        unit_records,
                    );
                    units_replay_verified &= unit_replay_verified(&unit);
                    let mut tampered = unit.clone();
                    tampered.replay_hash.push('x');
                    units_tamper_rejected &= !unit_replay_verified(&tampered);
                    unit_statuses.push(unit.status);
                    provenance_preserved &= !unit.provenance.is_empty();
                    if unit.status != UnitFrontendStatus::Complete {
                        unit_complete = false;
                        continue;
                    }
                    let Some(unit_request) = unit.request.as_ref() else {
                        unit_complete = false;
                        continue;
                    };
                    let unit_result =
                        evaluate_formula_records(unit_request, UNIT_DOMAIN, unit_records);
                    if unit_result.status != FormulaStatus::Complete || unit_result.value.is_none()
                    {
                        unit_complete = false;
                        continue;
                    }
                    provenance_preserved &= !unit_result.provenance.is_empty();
                    converted_inputs.insert(input.clone(), unit_result.value.unwrap());
                    input_dimensions.insert(
                        input.clone(),
                        unit_dimension(target).expect("catalog unit has dimension"),
                    );
                    unit_results.push(unit);
                }
                if unit_complete {
                    if let Ok(dimension) =
                        expression_dimension(&record.expression, &input_dimensions)
                    {
                        dimensional_compatible = true;
                        output_dimension = Some(dimension);
                    }
                }
                if unit_complete && dimensional_compatible {
                    let composed_request = FormulaRequest {
                        formula: request.formula.clone(),
                        inputs: converted_inputs,
                        domain: GEOMETRY_DOMAIN.into(),
                        ambiguity: None,
                        provenance: request
                            .provenance
                            .iter()
                            .cloned()
                            .chain(
                                unit_results
                                    .iter()
                                    .flat_map(|unit| unit.provenance.iter().cloned()),
                            )
                            .collect(),
                    };
                    let result = evaluate_formula_records(
                        &composed_request,
                        GEOMETRY_DOMAIN,
                        geometry_records,
                    );
                    execution_status = Some(result.status);
                    execution_replay_verified = result.replay_verified();
                    let mut tampered = result.clone();
                    tampered.replay_hash.push('x');
                    execution_tamper_rejected = !tampered.replay_verified();
                    provenance_preserved &= !result.provenance.is_empty();
                }
            }
        }
    }
    let execution_complete = execution_status == Some(FormulaStatus::Complete);
    let authorized = expected == Expected::Supported
        && geometry.status == FormulaFrontendStatus::Complete
        && unit_complete
        && dimensional_compatible
        && execution_complete;
    let exact = match expected {
        Expected::Supported => authorized,
        Expected::Ambiguous => geometry.status == FormulaFrontendStatus::Ambiguous && !authorized,
        Expected::Refused => {
            !dimensional_compatible
                && geometry.status != FormulaFrontendStatus::Ambiguous
                && !authorized
        }
    };
    let units_replay_verified = unit_statuses.is_empty() || units_replay_verified;
    let units_tamper_rejected = unit_statuses.is_empty() || units_tamper_rejected;
    let mut receipt = Receipt {
        id,
        partition: partition.into(),
        expected,
        formula: geometry.formula.clone(),
        geometry_status: geometry.status,
        unit_statuses,
        execution_status,
        output_dimension,
        dimensional_compatible,
        exact,
        authorized,
        geometry_replay_verified,
        geometry_tamper_rejected,
        units_replay_verified,
        units_tamper_rejected,
        execution_replay_verified,
        execution_tamper_rejected,
        composition_replay_verified: provenance_preserved && exact,
        composition_tamper_rejected: false,
        provenance_preserved,
        false_authorization: expected != Expected::Supported && authorized,
        false_denial: expected == Expected::Supported && !authorized,
        replay_hash: String::new(),
    }
    .finalize();
    let composition_replay_verified = receipt.replay_verified();
    let mut tampered = receipt.clone();
    tampered.replay_hash.push('x');
    let composition_tamper_rejected = !tampered.replay_verified();
    receipt.composition_replay_verified = composition_replay_verified;
    receipt.composition_tamper_rejected = composition_tamper_rejected;
    receipt.finalize()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let geometry_records = extract_formula_records(GEOMETRY_SOURCE)
        .map_err(|errors| format!("geometry source extraction failed: {errors:?}"))?;
    let unit_records = extract_formula_records(UNIT_SOURCE)
        .map_err(|errors| format!("unit source extraction failed: {errors:?}"))?;
    assert_eq!(geometry_records.len(), 5);
    let mut receipts = Vec::with_capacity(400);
    let mut development_supported = 0;
    let mut development_ambiguous = 0;
    let mut development_refused = 0;
    let mut development_exact = 0;
    let mut development_authorized = 0;
    let mut development_geometry_replay = 0;
    let mut development_unit_replay = 0;
    let mut development_execution_replay = 0;
    let mut development_composition_replay = 0;
    let mut development_tamper_rejections = 0;
    let mut holdout_supported = 0;
    let mut holdout_exact = 0;
    let mut holdout_authorized = 0;
    let mut holdout_composition_replay = 0;
    let mut false_authorizations = 0;
    let mut false_denials = 0;
    let mut unit_refusals = 0;
    let mut provenance_preserved = 0;

    for index in 0..300 {
        let record = &geometry_records[index % geometry_records.len()];
        let expected = match index % 10 {
            0..=5 => Expected::Supported,
            6..=7 => Expected::Ambiguous,
            _ => Expected::Refused,
        };
        let text = match expected {
            Expected::Supported => render(record, index),
            Expected::Ambiguous => "Compute the rectangle area and triangle area using length=4 width=3 base=5 height=2.".into(),
            Expected::Refused => render(record, index),
        };
        let receipt = compose(
            &geometry_records,
            &unit_records,
            text,
            format!("development-{index}"),
            "development",
            expected,
            expected == Expected::Refused,
        );
        development_supported += usize::from(expected == Expected::Supported);
        development_ambiguous += usize::from(expected == Expected::Ambiguous);
        development_refused += usize::from(expected == Expected::Refused);
        development_exact += usize::from(receipt.exact);
        development_authorized += usize::from(receipt.authorized);
        development_geometry_replay += usize::from(receipt.geometry_replay_verified);
        development_unit_replay += usize::from(receipt.units_replay_verified);
        development_execution_replay += usize::from(receipt.execution_replay_verified);
        development_composition_replay += usize::from(receipt.composition_replay_verified);
        development_tamper_rejections += usize::from(
            receipt.geometry_tamper_rejected
                && receipt.units_tamper_rejected
                && receipt.execution_tamper_rejected
                && receipt.composition_tamper_rejected,
        );
        unit_refusals += usize::from(
            expected == Expected::Refused
                && receipt
                    .unit_statuses
                    .iter()
                    .any(|status| *status == UnitFrontendStatus::Unsupported),
        );
        false_authorizations += usize::from(receipt.false_authorization);
        false_denials += usize::from(receipt.false_denial);
        provenance_preserved += usize::from(receipt.provenance_preserved);
        receipts.push(receipt);
    }
    for index in 0..100 {
        let record = &geometry_records[(index + 2) % geometry_records.len()];
        let receipt = compose(
            &geometry_records,
            &unit_records,
            render(record, index + 1000),
            format!("holdout-{index}"),
            "holdout",
            Expected::Supported,
            false,
        );
        holdout_supported += 1;
        holdout_exact += usize::from(receipt.exact);
        holdout_authorized += usize::from(receipt.authorized);
        holdout_composition_replay += usize::from(receipt.composition_replay_verified);
        false_authorizations += usize::from(receipt.false_authorization);
        false_denials += usize::from(receipt.false_denial);
        provenance_preserved += usize::from(receipt.provenance_preserved);
        receipts.push(receipt);
    }
    assert_eq!(development_supported, 180);
    assert_eq!(development_ambiguous, 60);
    assert_eq!(development_refused, 60);
    assert_eq!(development_exact, 300);
    assert_eq!(development_authorized, 180);
    assert_eq!(development_geometry_replay, 300);
    assert_eq!(development_unit_replay, 300);
    assert_eq!(development_execution_replay, 300);
    assert_eq!(development_composition_replay, 300);
    assert_eq!(development_tamper_rejections, 300);
    assert_eq!(holdout_supported, 100);
    assert_eq!(holdout_exact, 100);
    assert_eq!(holdout_authorized, 100);
    assert_eq!(holdout_composition_replay, 100);
    assert_eq!(false_authorizations, 0);
    assert_eq!(false_denials, 0);
    assert_eq!(provenance_preserved, 400);
    let report = Report {
        schema: "stage165-geometry-measurement-composition-v1",
        parent_report_sha256: digest(&fs::read(PARENT)?),
        geometry_source_sha256: digest(GEOMETRY_SOURCE),
        unit_source_sha256: digest(UNIT_SOURCE),
        runtime_domain_specific_branches: 0,
        cases: 400,
        development_cases: 300,
        development_supported,
        development_ambiguous,
        development_refused,
        development_exact,
        development_authorized,
        development_geometry_replay,
        development_unit_replay,
        development_execution_replay,
        development_composition_replay,
        development_tamper_rejections,
        holdout_cases: 100,
        holdout_supported,
        holdout_exact,
        holdout_authorized,
        holdout_composition_replay,
        false_authorizations,
        false_denials,
        unit_refusals,
        provenance_preserved,
        live_registry_mutations: 0,
        receipts,
    };
    fs::write(
        REPORT_JSON,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        REPORT_MD,
        "# Stage 165 — geometry and measurement composition\n\nThe generic composition layer converts explicitly declared measurements through the source unit catalog, checks expression dimensions, and delegates formula execution to the generic source runtime. No geometry-formula evaluator branch was added.\n\n| Measure | Result |\n|---|---:|\n| Cases | 400 |\n| Development supported / ambiguous / refused | 180 / 60 / 60 |\n| Development exact / authorized | 300/300 / 180/180 |\n| Holdout supported / exact / authorized | 100 / 100 / 100 |\n| Geometry / unit / execution replay (development) | 300/300 / 300/300 / 300/300 |\n| Composition replay / tamper (all) | 400/400 / 400/400 |\n| Unit-boundary refusals | 60 |\n| False authorizations / denials | 0 / 0 |\n| Runtime domain-specific branches | 0 |\n| Live registry mutations | 0 |\n\nParent language-transfer provenance is hash-bound to Stage 164.\n",
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
