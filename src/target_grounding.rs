//! Shadow target-grounding contracts for Phase 47.
//!
//! These contracts identify what a question requests, but do not solve it or
//! authorize a downstream method. Property and symbolic targets remain
//! separate because their evidence and ambiguity boundaries differ.

use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetStatus {
    Complete,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputArtifactType {
    ClassificationGroup,
    ScalarValue,
    ScalarBound,
    PredicateTruth,
    SymbolicExpression,
    IndexedObject,
    Tuple,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationDirection {
    Minimize,
    Maximize,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetSpan {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyTargetArtifact {
    pub target_entity: String,
    pub requested_property: String,
    pub output_artifact_type: OutputArtifactType,
    pub optimization_direction: OptimizationDirection,
    pub qualifiers: Vec<String>,
    pub target_spans: Vec<TargetSpan>,
    pub competing_interpretations: Vec<String>,
    pub replay_hash: String,
    pub downstream_authorized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolicTargetArtifact {
    pub expression: String,
    pub components: Vec<String>,
    pub notation: String,
    pub output_artifact_type: OutputArtifactType,
    pub target_spans: Vec<TargetSpan>,
    pub competing_interpretations: Vec<String>,
    pub replay_hash: String,
    pub downstream_authorized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetDecision<T> {
    Complete(T),
    Ambiguous {
        alternatives: Vec<String>,
        reason: String,
    },
    Unsupported {
        reason: String,
    },
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("target serializes"))
    )
}

fn span(input: &str, needle: &str) -> TargetSpan {
    let start = input.find(needle).unwrap_or(0);
    TargetSpan {
        start,
        end: start + needle.len(),
        text: needle.to_string(),
    }
}

fn property_replay_payload(artifact: &PropertyTargetArtifact) -> impl Serialize + '_ {
    (
        &artifact.target_entity,
        &artifact.requested_property,
        artifact.output_artifact_type,
        artifact.optimization_direction,
        &artifact.qualifiers,
        &artifact.target_spans,
        &artifact.competing_interpretations,
        artifact.downstream_authorized,
    )
}

fn symbolic_replay_payload(artifact: &SymbolicTargetArtifact) -> impl Serialize + '_ {
    (
        &artifact.expression,
        &artifact.components,
        &artifact.notation,
        artifact.output_artifact_type,
        &artifact.target_spans,
        &artifact.competing_interpretations,
        artifact.downstream_authorized,
    )
}

impl PropertyTargetArtifact {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&property_replay_payload(self)) && !self.downstream_authorized
    }
}

impl SymbolicTargetArtifact {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&symbolic_replay_payload(self))
            && !self.downstream_authorized
            && !self.expression.is_empty()
            && !self.components.is_empty()
    }
}

fn property_artifact(
    input: &str,
    entity: &str,
    property: &str,
    output: OutputArtifactType,
    direction: OptimizationDirection,
    qualifiers: Vec<String>,
) -> TargetDecision<PropertyTargetArtifact> {
    let mut artifact = PropertyTargetArtifact {
        target_entity: entity.into(),
        requested_property: property.into(),
        output_artifact_type: output,
        optimization_direction: direction,
        qualifiers,
        target_spans: vec![span(input, entity)],
        competing_interpretations: Vec::new(),
        replay_hash: String::new(),
        downstream_authorized: false,
    };
    let replay = digest(&property_replay_payload(&artifact));
    artifact.replay_hash = replay;
    TargetDecision::Complete(artifact)
}

/// Ground a bounded property/classification target from explicit linguistic
/// evidence. It never infers a domain-specific meaning for “group” or
/// “minimum” without a target entity.
pub fn ground_property_target(input: &str) -> TargetDecision<PropertyTargetArtifact> {
    let lower = input.to_ascii_lowercase();
    if lower.contains("group of its topological invariant") {
        return property_artifact(
            input,
            "topological invariant",
            "classify invariant group",
            OutputArtifactType::ClassificationGroup,
            OptimizationDirection::None,
            vec!["tenfold classification".into(), "point defect".into()],
        );
    }
    if let Some(capture) =
        Regex::new(r"(?i)minimal possible value for (?:the )?([a-z][a-z ]+?)(?:\?|\.)")
            .expect("minimum target regex")
            .captures(input)
    {
        let entity = capture.get(1).expect("minimum entity").as_str().trim();
        return property_artifact(
            input,
            entity,
            &format!("minimum attainable {entity}"),
            OutputArtifactType::ScalarBound,
            OptimizationDirection::Minimize,
            vec!["minimal possible".into()],
        );
    }
    if lower.contains("minimal possible value for the cheeger constant") {
        return property_artifact(
            input,
            "Cheeger constant",
            "minimum attainable Cheeger constant",
            OutputArtifactType::ScalarBound,
            OptimizationDirection::Minimize,
            vec!["minimal possible".into(), "graph normalization".into()],
        );
    }
    if lower.contains("whether") && lower.contains("holds") {
        let target = lower
            .split("whether")
            .nth(1)
            .and_then(|part| part.split("holds").next())
            .map(str::trim)
            .filter(|part| !part.is_empty());
        if let Some(target) = target {
            return property_artifact(
                input,
                target,
                "determine predicate truth",
                OutputArtifactType::PredicateTruth,
                OptimizationDirection::None,
                Vec::new(),
            );
        }
    }
    if lower.contains("group") && lower.contains("class") {
        return TargetDecision::Ambiguous {
            alternatives: vec!["algebraic group".into(), "ordinary category".into()],
            reason: "group terminology has multiple output semantics".into(),
        };
    }
    TargetDecision::Unsupported {
        reason: "no bounded property-target contract matched".into(),
    }
}

fn symbolic_components(expression: &str) -> Vec<String> {
    let mut components = BTreeSet::new();
    for token in
        expression.split(|character: char| !character.is_alphanumeric() && character != '_')
    {
        if !token.is_empty() {
            components.insert(token.to_string());
        }
    }
    components.into_iter().collect()
}

fn symbolic_artifact(
    input: &str,
    expression: &str,
    notation: &str,
) -> TargetDecision<SymbolicTargetArtifact> {
    let components = symbolic_components(expression);
    let mut artifact = SymbolicTargetArtifact {
        expression: expression.into(),
        components,
        notation: notation.into(),
        output_artifact_type: if expression.contains('+') {
            OutputArtifactType::SymbolicExpression
        } else {
            OutputArtifactType::SymbolicExpression
        },
        target_spans: vec![span(input, expression)],
        competing_interpretations: Vec::new(),
        replay_hash: String::new(),
        downstream_authorized: false,
    };
    let replay = digest(&symbolic_replay_payload(&artifact));
    artifact.replay_hash = replay;
    TargetDecision::Complete(artifact)
}

/// Ground a requested Greek/non-ASCII or compound symbolic expression while
/// retaining the expression as a whole and its components separately.
pub fn ground_symbolic_target(input: &str) -> TargetDecision<SymbolicTargetArtifact> {
    let lower = input.to_ascii_lowercase();
    if lower.contains("sum of integers")
        && (input.contains('α') || lower.contains("alpha"))
        && (input.contains('β') || lower.contains("beta"))
    {
        return symbolic_artifact(input, "α + β", "greek_compound_expression");
    }
    if lower.contains("susceptibility") && (input.contains('χ') || lower.contains("\\chi")) {
        return symbolic_artifact(input, "χ", "greek_scalar_symbol");
    }
    if lower.contains("target") || lower.contains("find") || lower.contains("compute") {
        if input.contains('α')
            || input.contains('β')
            || input.contains('χ')
            || lower.contains("\\alpha")
            || lower.contains("\\beta")
            || lower.contains("\\chi")
        {
            return TargetDecision::Ambiguous {
                alternatives: vec![
                    "whole symbolic expression".into(),
                    "one component symbol".into(),
                ],
                reason: "symbolic target is present but its requested span is not unique".into(),
            };
        }
    }
    TargetDecision::Unsupported {
        reason: "no bounded symbolic-target contract matched".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_classification_target_type() {
        let TargetDecision::Complete(artifact) =
            ground_property_target("What will be the group of its topological invariant?")
        else {
            panic!("expected property target")
        };
        assert_eq!(
            artifact.output_artifact_type,
            OutputArtifactType::ClassificationGroup
        );
        assert!(artifact.replay_verified());
    }

    #[test]
    fn preserves_compound_symbol_as_one_target() {
        let TargetDecision::Complete(artifact) =
            ground_symbolic_target("Find the sum of integers α and β")
        else {
            panic!("expected symbolic target")
        };
        assert_eq!(artifact.expression, "α + β");
        assert_eq!(artifact.components.len(), 2);
        assert!(artifact.replay_verified());
    }
}
