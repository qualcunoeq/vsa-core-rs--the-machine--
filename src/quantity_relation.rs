//! Narrow, non-authorizing quantity-relation formalization.
//!
//! QuantityRelationV1 emits typed linear relation artifacts only.  It does
//! not solve the relation, infer unstated units, or mutate any registry.  A
//! later governed planner may hand an accepted artifact to an algebra
//! executor after independently validating the artifact and its replay.

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantityDecisionKind {
    Accepted,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantityConstraint {
    pub lhs: String,
    pub rhs: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantityRelationArtifact {
    pub family: String,
    pub signature: String,
    pub target: String,
    pub constraints: Vec<QuantityConstraint>,
    /// Optional exact expression for a separately governed algebra handoff.
    pub algebra_expression: Option<String>,
}

impl QuantityRelationArtifact {
    /// Deterministic local replay of the typed artifact.  This verifies that
    /// the artifact still has the declared structure without executing it.
    pub fn replay_verified(&self) -> bool {
        !self.family.is_empty()
            && !self.signature.is_empty()
            && !self.target.is_empty()
            && !self.constraints.is_empty()
            && self.algebra_expression.as_ref().is_some_and(|expression| !expression.is_empty())
            && self
                .constraints
                .iter()
                .all(|constraint| !constraint.lhs.is_empty() && !constraint.rhs.is_empty())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuantityRelationDecision {
    Accepted(QuantityRelationArtifact),
    Ambiguous,
    Unsupported,
}

impl QuantityRelationDecision {
    pub fn kind(&self) -> QuantityDecisionKind {
        match self {
            Self::Accepted(_) => QuantityDecisionKind::Accepted,
            Self::Ambiguous => QuantityDecisionKind::Ambiguous,
            Self::Unsupported => QuantityDecisionKind::Unsupported,
        }
    }
}

fn accepted(
    family: &str,
    signature: String,
    target: &str,
    lhs: String,
    rhs: String,
    algebra_expression: String,
) -> QuantityRelationDecision {
    QuantityRelationDecision::Accepted(QuantityRelationArtifact {
        family: family.into(),
        signature,
        target: target.into(),
        constraints: vec![QuantityConstraint { lhs, rhs }],
        algebra_expression: Some(algebra_expression),
    })
}

fn negative_guard(text: &str) -> Option<QuantityRelationDecision> {
    if text.contains("either")
        || text.contains("unknown")
        || text.contains("not specified")
        || text.contains("missing")
        || text.contains("unspecified")
        || (text.contains("add ") && text.contains("liters") && text.contains("kilograms"))
    {
        return Some(QuantityRelationDecision::Ambiguous);
    }
    if text.contains('%')
        || text.contains("percent")
        || text.contains("interest")
        || text.contains("exponential")
        || text.contains("nonlinear")
        || text.contains("probability")
        || text.contains("circle")
        || text.contains("three times")
        || text.contains("pauses")
        || text.contains("restarts")
        || text.contains("usual conversion")
    {
        return Some(QuantityRelationDecision::Unsupported);
    }
    None
}

/// Parse the deliberately bounded QuantityRelationV1 grammar.
pub fn formalize(prompt: &str) -> QuantityRelationDecision {
    let text = prompt
        .to_ascii_lowercase()
        .replace(['\n', '\r'], " ")
        .replace('’', "'");
    let text = text.trim();
    if let Some(decision) = negative_guard(text) {
        return decision;
    }

    let unit_rate = Regex::new(
        r"^(?:(\d+) notebooks cost (\d+) dollars\. what is the price per notebook\?|a total of (\d+) dollars buys (\d+) notebooks; find dollars per notebook\.|the total price is (\d+) dollars for (\d+) notebooks\. determine the dollars per notebook\.)$",
    )
    .unwrap();
    if let Some(caps) = unit_rate.captures(text) {
        let (count, total) = (1..=6)
            .collect::<Vec<_>>()
            .chunks(2)
            .find_map(|pair| caps.get(pair[0]).and_then(|left| caps.get(pair[1]).map(|right| (left, right))))
            .map(|(count, total)| (count.as_str(), total.as_str()))
            .unwrap();
        return accepted(
            "unit_rate",
            "[count:count,cost:currency]>currency/count>unit_rate".into(),
            "unit_rate",
            format!("notebooks * unit_rate = {total} dollars"),
            format!("notebooks = {count}"),
            format!("{total} / {count}"),
        );
    }

    let ratio = Regex::new(
        r"^(?:the ratio of red beads to blue beads is (\d+):(\d+)\. if there are (\d+) red beads, how many blue beads are there\?|for every (\d+) red beads there are (\d+) blue beads; the collection has (\d+) red beads\. find the blue count\.|there are (\d+) blue beads for (\d+) red beads under a (\d+):(\d+) ratio\. find blue beads\.)$",
    )
    .unwrap();
    if let Some(caps) = ratio.captures(text) {
        let numbers = (1..=10)
            .filter_map(|index| caps.get(index).map(|value| value.as_str()))
            .collect::<Vec<_>>();
        let (left, right, anchor) = match numbers.as_slice() {
            [left, right, anchor] => (*left, *right, *anchor),
            [left, right, anchor, ..] => (*left, *right, *anchor),
            _ => return QuantityRelationDecision::Unsupported,
        };
        return accepted(
            "ratio",
            "[left:count,right:count,ratio]>count>ratio_target".into(),
            "ratio_target",
            format!("blue/red = {right}/{left}"),
            format!("red = {anchor}"),
            format!("{right} * {anchor} / {left}"),
        );
    }

    let proportion = Regex::new(
        r"^(?:(\d+) identical batches require (\d+) liters\. how many liters are required for (\d+) batches at the same rate\?|at a constant proportion, (\d+) liters serve (\d+) batches\. determine liters for (\d+) batches\.|scale (\d+) liters for (\d+) batches to (\d+) batches at the same rate\.)$",
    )
    .unwrap();
    if let Some(caps) = proportion.captures(text) {
        let numbers = (1..=9)
            .filter_map(|index| caps.get(index).map(|value| value.as_str()))
            .collect::<Vec<_>>();
        let (source, source_count, target_count) = match numbers.as_slice() {
            [a, b, c] => (*a, *b, *c),
            _ => return QuantityRelationDecision::Unsupported,
        };
        return accepted(
            "proportion",
            "[source:quantity,source_count,target_count]>quantity>scaled_quantity".into(),
            "scaled_quantity",
            format!("quantity / batches = {source}/{source_count}"),
            format!("batches = {target_count}"),
            format!("{source} * {target_count} / {source_count}"),
        );
    }

    let conversion = Regex::new(
        r"^(?:using the stated conversion of (\d+) ([a-z]+) per ([a-z]+), convert (\d+) ([a-z]+) to ([a-z]+)\.|one ([a-z]+) contains (\d+) ([a-z]+)\. express (\d+) ([a-z]+) in ([a-z]+)\.)$",
    )
    .unwrap();
    if let Some(caps) = conversion.captures(text) {
        let (amount, factor, small, large) = if caps.get(1).is_some() {
            (caps[4].to_string(), caps[1].to_string(), caps[2].to_string(), caps[3].to_string())
        } else if caps.get(7).is_some() {
            (caps[10].to_string(), caps[8].to_string(), caps[9].to_string(), caps[7].to_string())
        } else {
            return QuantityRelationDecision::Unsupported;
        };
        let kind = if ["hours", "minutes", "days", "weeks"].contains(&large.as_str()) { "time" } else if ["liters", "milliliters"].contains(&large.as_str()) { "volume" } else if ["pounds", "ounces"].contains(&large.as_str()) { "mass" } else { "length" };
        return accepted(
            "unit_conversion",
            format!("[{kind}:{large},factor:{factor}{small}/{large}]>{small}>{kind}_converted"),
            "converted_quantity",
            format!("{large} * {factor} = {small}"),
            format!("factor = {factor}"),
            format!("{amount} * {factor}"),
        );
    }
    let conversion_rephrased = Regex::new(
        r"^express (\d+) ([a-z]+) as ([a-z]+), given (\d+) ([a-z]+) per ([a-z]+)\.$",
    )
    .unwrap();
    if let Some(caps) = conversion_rephrased.captures(text) {
        let amount = &caps[1];
        let large = &caps[2];
        let small = &caps[3];
        let factor = &caps[4];
        let kind = if ["hours", "minutes", "days", "weeks"].contains(&large) {
            "time"
        } else if ["liters", "milliliters"].contains(&large) {
            "volume"
        } else if ["pounds", "ounces"].contains(&large) {
            "mass"
        } else {
            "length"
        };
        return accepted(
            "unit_conversion",
            format!("[{kind}:{large},factor:{factor}{small}/{large}]>{small}>{kind}_converted"),
            "converted_quantity",
            format!("{amount} {large} * {factor} = {small}"),
            format!("factor = {factor}"),
            format!("{amount} * {factor}"),
        );
    }

    let sum = Regex::new(r"^(?:a box contains (\d+) red counters and (\d+) blue counters\. how many counters are there altogether\?|there are (\d+) red counters plus (\d+) blue counters in the box\. find the total count\.)$").unwrap();
    if let Some(caps) = sum.captures(text) {
        let first = caps.iter().skip(1).flatten().next().unwrap().as_str();
        let second = caps.iter().skip(1).flatten().nth(1).unwrap().as_str();
        return accepted("sum_difference", "[first:quantity,second:quantity]>quantity>target".into(), "total", format!("total = {first} + {second}"), "unit = count".into(), format!("{first} + {second}"));
    }
    let remaining = Regex::new(r"^(?:a container has (\d+) liters and (\d+) liters are removed\. how many liters remain\?|after taking (\d+) liters from a container holding (\d+) liters, state the remaining volume\.)$").unwrap();
    if let Some(caps) = remaining.captures(text) {
        let values = caps.iter().skip(1).flatten().map(|value| value.as_str()).collect::<Vec<_>>();
        let (first, second) = if text.starts_with("a container") { (values[0], values[1]) } else { (values[1], values[0]) };
        return accepted("sum_difference", "[first:quantity,second:quantity]>quantity>target".into(), "remaining", format!("remaining = {first} - {second}"), "unit = liters".into(), format!("{first} - {second}"));
    }

    let linear = Regex::new(r"^(?:a quantity starts at (\d+) units and increases by (\d+) units\. what is the final quantity\?|the final amount is (\d+) units after adding (\d+) units\. what was the starting amount\??)$").unwrap();
    if let Some(caps) = linear.captures(text) {
        let values = caps.iter().skip(1).flatten().map(|value| value.as_str()).collect::<Vec<_>>();
        let expression = if text.starts_with("a quantity") {
            format!("{} + {}", values[0], values[1])
        } else {
            format!("{} - {}", values[0], values[1])
        };
        return accepted("linear_quantity", "[base:quantity,change:quantity]>quantity>linear_target".into(), "linear_target", format!("quantity = {expression}"), "unit = units".into(), expression);
    }
    QuantityRelationDecision::Unsupported
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_expanded_relation_families() {
        let prompts = [
            "5 notebooks cost 20 dollars. What is the price per notebook?",
            "The ratio of red beads to blue beads is 2:3. If there are 8 red beads, how many blue beads are there?",
            "3 identical batches require 2 liters. How many liters are required for 5 batches at the same rate?",
            "Using the stated conversion of 100 centimeters per meters, convert 4 meters to centimeters.",
            "A box contains 8 red counters and 3 blue counters. How many counters are there altogether?",
            "A quantity starts at 10 units and increases by 2 units. What is the final quantity?",
        ];
        for prompt in prompts {
            let QuantityRelationDecision::Accepted(artifact) = formalize(prompt) else { panic!("not accepted: {prompt}"); };
            assert!(artifact.replay_verified());
        }
    }

    #[test]
    fn rejects_out_of_scope_and_ambiguous_relations() {
        assert!(matches!(formalize("A price changes by 20% each year. What is the final price?"), QuantityRelationDecision::Unsupported));
        assert!(matches!(formalize("Convert 5 miles to kilometers using the usual conversion."), QuantityRelationDecision::Unsupported));
        assert!(matches!(formalize("A vehicle travels quickly and the time is missing."), QuantityRelationDecision::Ambiguous));
    }
}
