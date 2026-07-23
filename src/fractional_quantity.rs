//! Narrow fractional-quantity reasoning.
//!
//! Only explicit fractions applied to known quantities are accepted.  This
//! module does not infer percentages, probabilities, missing denominators, or
//! symbolic fraction equations.

use crate::quantity_relation::{QuantityConstraint, QuantityRelationArtifact};
use crate::quantity_relation_integration::{
    bridge_to_algebra as bridge_quantity_to_algebra, AlgebraBridgeReceipt,
};
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FractionalDecisionKind {
    Accepted,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FractionalQuantityArtifact {
    pub operation: String,
    pub numerator: u64,
    pub denominator: u64,
    pub base_quantity: u64,
    pub target: String,
    pub expression: String,
    pub signature: String,
    pub constraints: Vec<QuantityConstraint>,
}

impl FractionalQuantityArtifact {
    pub fn replay_verified(&self) -> bool {
        self.numerator > 0
            && self.denominator > 0
            && self.numerator <= self.denominator
            && self.base_quantity > 0
            && !self.operation.is_empty()
            && !self.target.is_empty()
            && !self.expression.is_empty()
            && !self.signature.is_empty()
            && !self.constraints.is_empty()
    }

    pub fn to_quantity_relation(&self) -> QuantityRelationArtifact {
        QuantityRelationArtifact {
            family: "fractional_quantity".into(),
            signature: self.signature.clone(),
            target: self.target.clone(),
            constraints: self.constraints.clone(),
            algebra_expression: Some(self.expression.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FractionalQuantityDecision {
    Accepted(FractionalQuantityArtifact),
    Ambiguous,
    Unsupported,
}

impl FractionalQuantityDecision {
    pub fn kind(&self) -> FractionalDecisionKind {
        match self {
            Self::Accepted(_) => FractionalDecisionKind::Accepted,
            Self::Ambiguous => FractionalDecisionKind::Ambiguous,
            Self::Unsupported => FractionalDecisionKind::Unsupported,
        }
    }
}

fn fraction_value(text: &str) -> Option<(u64, u64)> {
    let normalized = text.trim().to_ascii_lowercase();
    if let Some((numerator, denominator)) = normalized.split_once('/') {
        return Some((numerator.parse().ok()?, denominator.parse().ok()?));
    }
    match normalized.as_str() {
        "a half" | "one half" | "half" => Some((1, 2)),
        "a third" | "one third" => Some((1, 3)),
        "two thirds" => Some((2, 3)),
        "a quarter" | "one quarter" => Some((1, 4)),
        "three quarters" => Some((3, 4)),
        "a fifth" | "one fifth" => Some((1, 5)),
        "two fifths" => Some((2, 5)),
        "three fifths" => Some((3, 5)),
        "four fifths" => Some((4, 5)),
        _ => None,
    }
}

fn accepted(
    operation: &str,
    numerator: u64,
    denominator: u64,
    base_quantity: u64,
    target: String,
    expression: String,
) -> FractionalQuantityDecision {
    FractionalQuantityDecision::Accepted(FractionalQuantityArtifact {
        operation: operation.into(),
        numerator,
        denominator,
        base_quantity,
        signature: format!("[fraction:{numerator}/{denominator}]>quantity>{operation}"),
        constraints: vec![QuantityConstraint {
            lhs: format!("fraction = {numerator}/{denominator}"),
            rhs: format!("base = {base_quantity}"),
        }],
        target,
        expression,
    })
}

/// Formalize explicit fraction-of-quantity statements only.
pub fn formalize(prompt: &str) -> FractionalQuantityDecision {
    let text = prompt.to_ascii_lowercase().replace(['\n', '\r'], " ");
    let text = text.trim();
    if text.contains('%')
        || text.contains("percent")
        || text.contains("probability")
        || text.contains("chance")
        || text.contains("grows")
        || text.contains("each year")
        || text.contains("compound")
    {
        return FractionalQuantityDecision::Unsupported;
    }
    if text.contains("unknown") || text.contains("not specified") || text.contains("variable") {
        return if text.contains("not specified") || text.contains("unknown") {
            FractionalQuantityDecision::Ambiguous
        } else {
            FractionalQuantityDecision::Unsupported
        };
    }

    let direct = Regex::new(
        r"^(?:compute|calculate|find|what is) (one half|a half|half|one third|a third|two thirds|one quarter|a quarter|three quarters|one fifth|a fifth|two fifths|three fifths|four fifths|\d+/\d+) of (\d+)(?: [a-z]+)?\??\.?$",
    )
    .unwrap();
    if let Some(caps) = direct.captures(text) {
        let Some((numerator, denominator)) = fraction_value(caps.get(1).unwrap().as_str()) else {
            return FractionalQuantityDecision::Unsupported;
        };
        let base = caps.get(2).unwrap().as_str().parse().unwrap();
        if numerator == 0 || numerator > denominator {
            return FractionalQuantityDecision::Unsupported;
        }
        return accepted(
            "part",
            numerator,
            denominator,
            base,
            "fractional_part".into(),
            format!("{base} * {numerator} / {denominator}"),
        );
    }

    let remaining = Regex::new(
        r"^(?:what remains after removing|find the remainder after removing) (one half|a half|half|one third|a third|two thirds|one quarter|a quarter|three quarters|one fifth|a fifth|two fifths|three fifths|four fifths|\d+/\d+) of (\d+)(?: [a-z]+)?\??\.?$",
    )
    .unwrap();
    if let Some(caps) = remaining.captures(text) {
        let Some((numerator, denominator)) = fraction_value(caps.get(1).unwrap().as_str()) else {
            return FractionalQuantityDecision::Unsupported;
        };
        let base = caps.get(2).unwrap().as_str().parse().unwrap();
        if numerator == 0 || numerator > denominator {
            return FractionalQuantityDecision::Unsupported;
        }
        return accepted(
            "remainder",
            numerator,
            denominator,
            base,
            "remainder".into(),
            format!("{base} - {base} * {numerator} / {denominator}"),
        );
    }

    let equal_parts =
        Regex::new(r"^(?:one of|each of) (\d+) equal parts of (\d+)(?: [a-z]+)?\??\.?$").unwrap();
    if let Some(caps) = equal_parts.captures(text) {
        let denominator: u64 = caps.get(1).unwrap().as_str().parse().unwrap();
        let base: u64 = caps.get(2).unwrap().as_str().parse().unwrap();
        if denominator == 0 {
            return FractionalQuantityDecision::Unsupported;
        }
        return accepted(
            "equal_part",
            1,
            denominator,
            base,
            "equal_part".into(),
            format!("{base} / {denominator}"),
        );
    }

    if text.contains("fraction") || text.contains("part of") || text.contains("half") {
        FractionalQuantityDecision::Ambiguous
    } else {
        FractionalQuantityDecision::Unsupported
    }
}

pub fn bridge_to_algebra(artifact: &FractionalQuantityArtifact) -> Option<AlgebraBridgeReceipt> {
    if !artifact.replay_verified() {
        return None;
    }
    bridge_quantity_to_algebra(&artifact.to_quantity_relation())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_explicit_fraction_operations() {
        for prompt in [
            "What is three quarters of 20?",
            "What remains after removing 1/4 of 20?",
            "One of 5 equal parts of 35.",
        ] {
            let FractionalQuantityDecision::Accepted(artifact) = formalize(prompt) else {
                panic!("not accepted: {prompt}");
            };
            assert!(artifact.replay_verified());
            assert!(bridge_to_algebra(&artifact).is_some());
        }
    }

    #[test]
    fn refuses_percentages_and_missing_fractions() {
        assert!(matches!(
            formalize("What is 20% of 50?"),
            FractionalQuantityDecision::Unsupported
        ));
        assert!(matches!(
            formalize("What fraction of 50 is the result?"),
            FractionalQuantityDecision::Ambiguous
        ));
    }
}
