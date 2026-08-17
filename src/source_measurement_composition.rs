//! Generic typed composition of source formulas and source unit conversions.
//!
//! This module deliberately knows nothing about geometry, physics, or any
//! other formula family.  It joins a source-declared formula request to
//! source-declared measurement conversions, checks dimensional operations, and
//! delegates execution back to the generic formula interpreter.

use crate::source_formula_frontend::{
    formalize_formula_text, FormulaFrontendResult, FormulaFrontendStatus,
};
use crate::source_formula_pack::{
    evaluate_formula_records, Expr, FormulaRecord, FormulaRequest, FormulaResult, FormulaStatus,
};
use crate::source_unit_frontend::{formalize_unit_text, UnitFrontendResult, UnitFrontendStatus};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Dimension {
    pub length: i32,
    pub mass: i32,
    pub time: i32,
}

impl Dimension {
    pub fn zero() -> Self {
        Self {
            length: 0,
            mass: 0,
            time: 0,
        }
    }
    pub fn add(&self, other: &Self) -> Self {
        Self {
            length: self.length + other.length,
            mass: self.mass + other.mass,
            time: self.time + other.time,
        }
    }
    pub fn sub(&self, other: &Self) -> Self {
        Self {
            length: self.length - other.length,
            mass: self.mass - other.mass,
            time: self.time - other.time,
        }
    }
    pub fn scale(&self, exponent: u32) -> Self {
        let exponent = exponent as i32;
        Self {
            length: self.length * exponent,
            mass: self.mass * exponent,
            time: self.time * exponent,
        }
    }
}

/// Source-unit dimension metadata is intentionally limited to units with
/// explicit catalog conversion records. Unknown units never enter a typed
/// composition.
pub fn dimension_for_unit(unit: &str) -> Option<Dimension> {
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

pub fn expression_dimension(
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnitAssignment {
    pub source_unit: String,
    pub target_unit: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompositionStatus {
    Complete,
    Ambiguous,
    Missing,
    Unsupported,
    InvalidDimensions,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MeasurementComposition {
    pub status: CompositionStatus,
    pub formula: FormulaFrontendResult,
    pub unit_results: Vec<UnitFrontendResult>,
    pub formula_result: Option<FormulaResult>,
    pub output_dimension: Option<Dimension>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("measurement composition serializes"))
    )
}

impl MeasurementComposition {
    fn finalize(mut self) -> Self {
        self.replay_hash.clear();
        self.replay_hash = digest(&self);
        self
    }
    pub fn replay_verified(&self) -> bool {
        let mut copy = self.clone();
        let hash = copy.replay_hash.clone();
        copy.replay_hash.clear();
        hash == digest(&copy) && !self.provenance.is_empty()
    }
}

/// Compose a source-formula frontend request with explicit source-unit
/// assignments. The caller supplies unit records and assignments; no formula
/// identifier or subject name is interpreted by this function.
pub fn compose_formula_text(
    text: &str,
    formula_domain: &str,
    unit_domain: &str,
    case_id: &str,
    formula_records: &[FormulaRecord],
    unit_records: &[FormulaRecord],
    assignments: &BTreeMap<String, UnitAssignment>,
) -> MeasurementComposition {
    let formula = formalize_formula_text(text, formula_domain, formula_records);
    let mut provenance = formula.provenance_spans.clone();
    let mut unit_results = Vec::new();
    let mut formula_result = None;
    let mut output_dimension = None;
    let status = match formula.status {
        FormulaFrontendStatus::Ambiguous => CompositionStatus::Ambiguous,
        FormulaFrontendStatus::Unsupported => CompositionStatus::Unsupported,
        FormulaFrontendStatus::Missing => CompositionStatus::Missing,
        FormulaFrontendStatus::Complete => {
            let Some(request) = formula.request.as_ref() else {
                return MeasurementComposition {
                    status: CompositionStatus::Missing,
                    formula,
                    unit_results,
                    formula_result,
                    output_dimension,
                    provenance: vec![format!("measurement-composition:{case_id}")],
                    replay_hash: String::new(),
                }
                .finalize();
            };
            let Some(record) = formula_records
                .iter()
                .find(|record| record.formula_id == request.formula)
            else {
                return MeasurementComposition {
                    status: CompositionStatus::Missing,
                    formula,
                    unit_results,
                    formula_result,
                    output_dimension,
                    provenance: vec![format!("measurement-composition:{case_id}")],
                    replay_hash: String::new(),
                }
                .finalize();
            };
            let mut converted = BTreeMap::new();
            let mut dimensions = BTreeMap::new();
            for input in &record.required_inputs {
                let Some(assignment) = assignments.get(input) else {
                    return MeasurementComposition {
                        status: CompositionStatus::Missing,
                        formula,
                        unit_results,
                        formula_result,
                        output_dimension,
                        provenance: vec![format!("measurement-composition:{case_id}")],
                        replay_hash: String::new(),
                    }
                    .finalize();
                };
                let amount = request.inputs[input].clone();
                let unit_text = format!(
                    "Convert {} {} to {}.",
                    amount.numerator, assignment.source_unit, assignment.target_unit
                );
                let unit = formalize_unit_text(
                    &unit_text,
                    &format!("{case_id}-unit-{input}"),
                    unit_records,
                );
                provenance.extend(unit.provenance.iter().cloned());
                let Some(unit_request) = unit.request.as_ref() else {
                    unit_results.push(unit);
                    return MeasurementComposition {
                        status: CompositionStatus::Unsupported,
                        formula,
                        unit_results,
                        formula_result,
                        output_dimension,
                        provenance,
                        replay_hash: String::new(),
                    }
                    .finalize();
                };
                let unit_value = evaluate_formula_records(unit_request, unit_domain, unit_records);
                if unit.status != UnitFrontendStatus::Complete
                    || unit_value.status != FormulaStatus::Complete
                    || unit_value.value.is_none()
                {
                    unit_results.push(unit);
                    return MeasurementComposition {
                        status: CompositionStatus::Unsupported,
                        formula,
                        unit_results,
                        formula_result,
                        output_dimension,
                        provenance,
                        replay_hash: String::new(),
                    }
                    .finalize();
                }
                provenance.extend(unit_value.provenance.iter().cloned());
                converted.insert(input.clone(), unit_value.value.expect("checked value"));
                let Some(dimension) = dimension_for_unit(&assignment.target_unit) else {
                    unit_results.push(unit);
                    return MeasurementComposition {
                        status: CompositionStatus::Unsupported,
                        formula,
                        unit_results,
                        formula_result,
                        output_dimension,
                        provenance,
                        replay_hash: String::new(),
                    }
                    .finalize();
                };
                dimensions.insert(input.clone(), dimension);
                unit_results.push(unit);
            }
            let Ok(dimension) = expression_dimension(&record.expression, &dimensions) else {
                return MeasurementComposition {
                    status: CompositionStatus::InvalidDimensions,
                    formula,
                    unit_results,
                    formula_result,
                    output_dimension,
                    provenance,
                    replay_hash: String::new(),
                }
                .finalize();
            };
            output_dimension = Some(dimension);
            let composed = FormulaRequest {
                formula: request.formula.clone(),
                inputs: converted,
                domain: formula_domain.into(),
                ambiguity: None,
                provenance: provenance.clone(),
            };
            let result = evaluate_formula_records(&composed, formula_domain, formula_records);
            if result.status != FormulaStatus::Complete || result.value.is_none() {
                formula_result = Some(result);
                CompositionStatus::Unsupported
            } else {
                provenance.extend(result.provenance.iter().cloned());
                formula_result = Some(result);
                CompositionStatus::Complete
            }
        }
    };
    provenance.push(format!("measurement-composition:{case_id}"));
    MeasurementComposition {
        status,
        formula,
        unit_results,
        formula_result,
        output_dimension,
        provenance,
        replay_hash: String::new(),
    }
    .finalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_formula_pack::extract_formula_records;

    fn assignments(names: &[&str]) -> BTreeMap<String, UnitAssignment> {
        names
            .iter()
            .map(|name| {
                (
                    (*name).into(),
                    UnitAssignment {
                        source_unit: "meters".into(),
                        target_unit: "centimeters".into(),
                    },
                )
            })
            .collect()
    }

    #[test]
    fn source_geometry_composes_with_explicit_measurements() {
        let formulas = extract_formula_records(include_str!(
            "../docs/sources/openstax_bounded_geometry_source.txt"
        ))
        .unwrap();
        let units = extract_formula_records(include_str!(
            "../docs/sources/openstax_unit_conversion_catalog.txt"
        ))
        .unwrap();
        let result = compose_formula_text(
            "Compute the rectangle area using length=4 width=3.",
            "source_derived_bounded_geometry",
            "source_catalog_unit_conversion",
            "test-supported",
            &formulas,
            &units,
            &assignments(&["length", "width"]),
        );
        assert_eq!(result.status, CompositionStatus::Complete);
        assert_eq!(result.output_dimension.as_ref().unwrap().length, 2);
        assert!(result.replay_verified());
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        assert!(!tampered.replay_verified());
    }

    #[test]
    fn ambiguous_and_unknown_units_remain_closed() {
        let formulas = extract_formula_records(include_str!(
            "../docs/sources/openstax_bounded_geometry_source.txt"
        ))
        .unwrap();
        let units = extract_formula_records(include_str!(
            "../docs/sources/openstax_unit_conversion_catalog.txt"
        ))
        .unwrap();
        let ambiguous = compose_formula_text(
            "Compute the rectangle area and triangle area using length=4 width=3 base=5 height=2.",
            "source_derived_bounded_geometry",
            "source_catalog_unit_conversion",
            "test-ambiguous",
            &formulas,
            &units,
            &assignments(&["length", "width", "base", "height"]),
        );
        assert_eq!(ambiguous.status, CompositionStatus::Ambiguous);
        let mut bad = assignments(&["length", "width"]);
        bad.get_mut("width").unwrap().source_unit = "unknown".into();
        let refused = compose_formula_text(
            "Compute the rectangle area using length=4 width=3.",
            "source_derived_bounded_geometry",
            "source_catalog_unit_conversion",
            "test-unknown",
            &formulas,
            &units,
            &bad,
        );
        assert_eq!(refused.status, CompositionStatus::Unsupported);
        assert!(refused.replay_verified());
    }
}
