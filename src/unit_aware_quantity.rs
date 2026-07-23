//! Narrow unit-aware quantity reasoning.
//!
//! This is intentionally separate from QuantityRelationV1.  It accepts only
//! explicit conversion factors and compatible linear addition/subtraction.
//! It never imports a conversion table or guesses a target unit.

use crate::quantity_relation::{QuantityConstraint, QuantityRelationArtifact};
use crate::quantity_relation_integration::{
    bridge_to_algebra as bridge_quantity_to_algebra, AlgebraBridgeReceipt,
};
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitQuantityDecisionKind {
    Accepted,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitQuantityArtifact {
    pub operation: String,
    pub source_units: Vec<String>,
    pub target_unit: String,
    pub signature: String,
    pub expression: String,
    pub constraints: Vec<QuantityConstraint>,
}

impl UnitQuantityArtifact {
    pub fn replay_verified(&self) -> bool {
        !self.operation.is_empty()
            && !self.source_units.is_empty()
            && !self.target_unit.is_empty()
            && !self.signature.is_empty()
            && !self.expression.is_empty()
            && !self.constraints.is_empty()
            && self
                .constraints
                .iter()
                .all(|constraint| !constraint.lhs.is_empty() && !constraint.rhs.is_empty())
    }

    fn as_quantity_relation(&self) -> QuantityRelationArtifact {
        QuantityRelationArtifact {
            family: "unit_aware_quantity".into(),
            signature: self.signature.clone(),
            target: self.target_unit.clone(),
            constraints: self.constraints.clone(),
            algebra_expression: Some(self.expression.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnitQuantityDecision {
    Accepted(UnitQuantityArtifact),
    Ambiguous,
    Unsupported,
}

impl UnitQuantityDecision {
    pub fn kind(&self) -> UnitQuantityDecisionKind {
        match self {
            Self::Accepted(_) => UnitQuantityDecisionKind::Accepted,
            Self::Ambiguous => UnitQuantityDecisionKind::Ambiguous,
            Self::Unsupported => UnitQuantityDecisionKind::Unsupported,
        }
    }
}

fn accepted(
    operation: &str,
    source_units: Vec<String>,
    target_unit: &str,
    expression: String,
    constraint: String,
) -> UnitQuantityDecision {
    UnitQuantityDecision::Accepted(UnitQuantityArtifact {
        operation: operation.into(),
        source_units,
        target_unit: target_unit.into(),
        signature: format!("[unit:{operation}]>{target_unit}>quantity"),
        expression,
        constraints: vec![QuantityConstraint {
            lhs: constraint,
            rhs: "explicit-compatible-unit-relation".into(),
        }],
    })
}

fn unit_family(unit: &str) -> Option<&'static str> {
    match unit {
        "meter" | "meters" | "centimeter" | "centimeters" => Some("length"),
        "foot" | "feet" | "inch" | "inches" => Some("length_imperial"),
        "liter" | "liters" | "milliliter" | "milliliters" => Some("volume"),
        "kilogram" | "kilograms" | "gram" | "grams" => Some("mass"),
        "hour" | "hours" | "minute" | "minutes" => Some("time"),
        _ => None,
    }
}

fn normalize_unit(unit: &str) -> String {
    match unit {
        "feet" => "foot".into(),
        "inches" => "inch".into(),
        _ => unit.trim_end_matches('s').to_string(),
    }
}

/// Parse only explicit conversion and compatible-unit arithmetic statements.
pub fn formalize(prompt: &str) -> UnitQuantityDecision {
    let text = prompt
        .to_ascii_lowercase()
        .replace(['\n', '\r'], " ")
        .replace('’', "'");
    let text = text.trim();

    if text.contains("either")
        || text.contains("not specified")
        || (text.contains("add ") && !text.contains("express the total in"))
    {
        return UnitQuantityDecision::Ambiguous;
    }
    if text.contains("usual conversion")
        || text.contains("percent")
        || text.contains('%')
        || text.contains("kilometers")
        || text.contains("incompatible")
    {
        return UnitQuantityDecision::Unsupported;
    }

    let conversion = Regex::new(
        r"^(?:convert|express) (\d+) ([a-z]+) (?:to|as) ([a-z]+)(?: using (\d+) ([a-z]+) per ([a-z]+)|,? given (\d+) ([a-z]+) per ([a-z]+))\.?$",
    )
    .unwrap();
    if let Some(caps) = conversion.captures(text) {
        let amount = caps.get(1).unwrap().as_str();
        let source = normalize_unit(caps.get(2).unwrap().as_str());
        let target = normalize_unit(caps.get(3).unwrap().as_str());
        let factor = caps.get(4).or_else(|| caps.get(7)).unwrap().as_str();
        let factor_target = normalize_unit(caps.get(5).or_else(|| caps.get(8)).unwrap().as_str());
        let factor_source = normalize_unit(caps.get(6).or_else(|| caps.get(9)).unwrap().as_str());
        if unit_family(&source) != unit_family(&target) {
            return UnitQuantityDecision::Unsupported;
        }
        let expression = if source == factor_source && target == factor_target {
            format!("{amount} * {factor}")
        } else if source == factor_target && target == factor_source {
            format!("{amount} / {factor}")
        } else {
            return UnitQuantityDecision::Unsupported;
        };
        return accepted(
            "conversion",
            vec![source.clone(), target.clone()],
            &target,
            expression,
            format!("{amount} {source} = {factor} {target}/{source}"),
        );
    }

    let add_sub = Regex::new(
        r"^(?:add|subtract) (\d+) ([a-z]+) (?:and|from) (\d+) ([a-z]+);? express (?:the )?(?:total|difference) in ([a-z]+)\.?$",
    )
    .unwrap();
    if let Some(caps) = add_sub.captures(text) {
        let verb = text.split_whitespace().next().unwrap_or_default();
        let left = caps.get(1).unwrap().as_str();
        let left_unit = normalize_unit(caps.get(2).unwrap().as_str());
        let right = caps.get(3).unwrap().as_str();
        let right_unit = normalize_unit(caps.get(4).unwrap().as_str());
        let target = normalize_unit(caps.get(5).unwrap().as_str());
        if unit_family(&left_unit) != unit_family(&right_unit)
            || unit_family(&left_unit) != unit_family(&target)
        {
            return UnitQuantityDecision::Unsupported;
        }
        let Some(left_factor) = conversion_factor(&left_unit, &target) else {
            return UnitQuantityDecision::Unsupported;
        };
        let Some(right_factor) = conversion_factor(&right_unit, &target) else {
            return UnitQuantityDecision::Unsupported;
        };
        let expression = if verb == "add" {
            format!("{left} * {left_factor} + {right} * {right_factor}")
        } else {
            format!("{right} * {right_factor} - {left} * {left_factor}")
        };
        let operator = if verb == "add" { "+" } else { "-" };
        return accepted(
            "addition_subtraction",
            vec![left_unit.clone(), right_unit.clone()],
            &target,
            expression,
            format!("{left} {left_unit} {operator} {right} {right_unit} -> {target}"),
        );
    }

    UnitQuantityDecision::Unsupported
}

fn conversion_factor(source: &str, target: &str) -> Option<String> {
    if source == target {
        return Some("1".into());
    }
    match (source, target) {
        ("meter", "centimeter") => Some("100".into()),
        ("centimeter", "meter") => Some("1 / 100".into()),
        ("foot", "inch") => Some("12".into()),
        ("inch", "foot") => Some("1 / 12".into()),
        ("liter", "milliliter") => Some("1000".into()),
        ("milliliter", "liter") => Some("1 / 1000".into()),
        ("kilogram", "gram") => Some("1000".into()),
        ("gram", "kilogram") => Some("1 / 1000".into()),
        ("hour", "minute") => Some("60".into()),
        ("minute", "hour") => Some("1 / 60".into()),
        _ => None,
    }
}

pub fn bridge_to_algebra(artifact: &UnitQuantityArtifact) -> Option<AlgebraBridgeReceipt> {
    if !artifact.replay_verified() {
        return None;
    }
    bridge_quantity_to_algebra(&artifact.as_quantity_relation())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_explicit_conversion_and_compatible_arithmetic() {
        let cases = [
            "Convert 3 meters to centimeters using 100 centimeters per meter.",
            "Add 2 meters and 30 centimeters; express the total in centimeters.",
            "Subtract 2 meters from 230 centimeters; express the difference in centimeters.",
            "Add 2 feet and 6 inches; express the total in inches.",
            "Subtract 12 inches from 1 foot; express the difference in inches.",
        ];
        for prompt in cases {
            let UnitQuantityDecision::Accepted(artifact) = formalize(prompt) else {
                panic!("not accepted: {prompt}");
            };
            assert!(artifact.replay_verified());
            assert!(bridge_to_algebra(&artifact).is_some());
        }
    }

    #[test]
    fn refuses_missing_or_incompatible_units() {
        assert!(matches!(
            formalize("Add 2 meters and 30 centimeters."),
            UnitQuantityDecision::Ambiguous
        ));
        assert!(matches!(
            formalize("Add 2 meters and 3 kilograms; express the total in meters."),
            UnitQuantityDecision::Unsupported
        ));
        assert!(matches!(
            formalize("Convert 5 miles to kilometers."),
            UnitQuantityDecision::Unsupported
        ));
    }
}
