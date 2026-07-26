//! Shadow grounding of mathematical regions to a question target.
//!
//! Parsing a math span is not enough for technical questions: definitions,
//! assumptions, quoted formulas, and requested expressions can all coexist.
//! This module keeps every candidate and only selects one when the surrounding
//! request provides a bounded, auditable target signal.

use crate::notation_normalization::{normalize_equation, NormalizationStatus};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MathRegionRole {
    Definition,
    Assumption,
    Evidence,
    RequestedExpression,
    AnswerFormatConstraint,
    Quoted,
    Incidental,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MathRegionCandidate {
    pub index: usize,
    pub source: String,
    pub span: String,
    pub role: MathRegionRole,
    pub role_evidence: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroundingStatus {
    Accepted,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionTarget {
    pub requested_operation: Option<String>,
    pub requested_entity: Option<String>,
    pub candidate_regions: Vec<MathRegionCandidate>,
    pub supporting_region_indices: Vec<usize>,
    pub selected_region_index: Option<usize>,
    pub rejected_region_indices: Vec<usize>,
    pub unresolved_alternatives: Vec<usize>,
    pub provenance_spans: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundingResult {
    pub status: GroundingStatus,
    pub target: QuestionTarget,
    pub normalized_source: Option<String>,
    pub normalized_status: Option<NormalizationStatus>,
    pub symbol_bindings: Vec<String>,
    pub replay_verified: bool,
    pub receipt_hash: String,
    pub reason: String,
}

fn receipt_hash<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("grounding receipt serializes");
    format!("{:x}", Sha256::digest(bytes))
}

fn region_spans(source: &str) -> Vec<(String, String, usize, usize)> {
    let pairs = [("\\[", "\\]"), ("\\(", "\\)"), ("$$", "$$"), ("$", "$")];
    let mut spans = Vec::new();
    for (open, close) in pairs {
        let mut cursor = 0;
        while let Some(relative_start) = source[cursor..].find(open) {
            let start = cursor + relative_start;
            let body_start = start + open.len();
            let Some(relative_end) = source[body_start..].find(close) else {
                break;
            };
            let end = body_start + relative_end;
            spans.push((
                source[body_start..end].trim().to_string(),
                format!("{start}..{}", end + close.len()),
                start,
                end + close.len(),
            ));
            cursor = end + close.len();
        }
    }
    spans.sort_by_key(|(_, _, start, _)| *start);
    spans.dedup_by(|left, right| left.2 == right.2 && left.3 == right.3);
    spans
}

fn local_context(source: &str, start: usize, end: usize) -> (String, String) {
    let before = &source[..start.min(source.len())];
    let after = &source[end.min(source.len())..];
    let before = before
        .char_indices()
        .rev()
        .find(|(_, character)| ".,;:!?".contains(*character))
        .map(|(index, _)| &before[index + 1..])
        .unwrap_or(before);
    let after = after
        .char_indices()
        .find(|(_, character)| ".,;:!?".contains(*character))
        .map(|(index, _)| &after[..index])
        .unwrap_or(after);
    let before_window: String = before
        .chars()
        .rev()
        .take(120)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    let after_window: String = after.chars().take(120).collect();
    (
        before_window.to_ascii_lowercase(),
        after_window.to_ascii_lowercase(),
    )
}

fn role_for(source: &str, start: usize, end: usize) -> (MathRegionRole, Vec<String>) {
    let (before, after) = local_context(source, start, end);
    let context = format!("{before} {after}");
    let mut evidence = Vec::new();
    if [
        "quoted",
        "according to",
        "source says",
        "excerpt",
        "citation",
    ]
    .iter()
    .any(|marker| context.contains(marker))
    {
        evidence.push("quotation_or_citation_context".into());
        return (MathRegionRole::Quoted, evidence);
    }
    if [
        "answer choices",
        "answer format",
        "write",
        "output",
        "respond",
    ]
    .iter()
    .any(|marker| context.contains(marker))
    {
        evidence.push("answer_format_context".into());
        return (MathRegionRole::AnswerFormatConstraint, evidence);
    }
    let assumption_nearby = ["given ", "suppose", "assume", "assuming", "use "]
        .iter()
        .any(|marker| before.ends_with(marker) || before.contains(&format!(" {marker}")));
    let definition_nearby = ["define", "defined", "denote", "let ", "where "]
        .iter()
        .any(|marker| after.starts_with(marker) || after.contains(&format!(" {marker}")));
    if assumption_nearby || definition_nearby {
        evidence.push("definition_or_assumption_context".into());
        if assumption_nearby {
            return (MathRegionRole::Assumption, evidence);
        }
        return (MathRegionRole::Definition, evidence);
    }
    if [
        "find",
        "solve",
        "compute",
        "calculate",
        "determine",
        "evaluate",
        "derive",
        "what is",
        "which",
    ]
    .iter()
    .any(|marker| context.contains(marker))
    {
        evidence.push("question_request_context".into());
        return (MathRegionRole::RequestedExpression, evidence);
    }
    evidence.push("no_local_role_signal".into());
    (MathRegionRole::Incidental, evidence)
}

fn request_signal(source: &str) -> Option<String> {
    let lower = source.to_ascii_lowercase();
    [
        ("find", "find"),
        ("compute", "compute"),
        ("calculate", "calculate"),
        ("determine", "determine"),
        ("evaluate", "evaluate"),
        ("derive", "derive"),
        ("what is", "identify"),
        ("which", "select"),
        ("express", "express"),
        ("for what range", "range"),
    ]
    .iter()
    .find(|(marker, _)| lower.contains(marker))
    .map(|(_, operation)| (*operation).to_string())
}

/// Ground math regions to a target without authorizing a downstream route.
pub fn ground_math_target(source: &str) -> GroundingResult {
    let spans = region_spans(source);
    let candidates: Vec<_> = spans
        .iter()
        .enumerate()
        .map(|(index, (math, span, start, end))| {
            let (role, role_evidence) = role_for(source, *start, *end);
            MathRegionCandidate {
                index,
                source: math.clone(),
                span: span.clone(),
                role,
                role_evidence,
            }
        })
        .collect();
    let operation = request_signal(source);
    let requested: Vec<usize> = candidates
        .iter()
        .filter(|candidate| candidate.role == MathRegionRole::RequestedExpression)
        .map(|candidate| candidate.index)
        .collect();
    let mut selected = None;
    let mut unresolved = Vec::new();
    let mut status = GroundingStatus::Unsupported;
    let mut reason = "no mathematical region found".to_string();
    if candidates.is_empty() {
        status = GroundingStatus::Unsupported;
    } else if requested.len() > 1 {
        unresolved = requested.clone();
        status = GroundingStatus::Ambiguous;
        reason = "multiple math regions are linked to the request".into();
    } else if let Some(index) = requested.first().copied() {
        selected = Some(index);
        status = GroundingStatus::Accepted;
        reason = "unique request-linked math region selected".into();
    } else if candidates.len() == 1 && operation.is_some() {
        selected = Some(0);
        status = GroundingStatus::Accepted;
        reason = "single math region selected under explicit request".into();
    } else if operation.is_some() {
        unresolved = candidates.iter().map(|candidate| candidate.index).collect();
        status = GroundingStatus::Ambiguous;
        reason = "request exists but no unique target-linked region was identified".into();
    }
    let supporting = candidates
        .iter()
        .filter(|candidate| {
            Some(candidate.index) != selected
                && matches!(
                    candidate.role,
                    MathRegionRole::Definition
                        | MathRegionRole::Assumption
                        | MathRegionRole::Evidence
                )
        })
        .map(|candidate| candidate.index)
        .collect();
    let rejected = candidates
        .iter()
        .filter(|candidate| {
            Some(candidate.index) != selected && !unresolved.contains(&candidate.index)
        })
        .map(|candidate| candidate.index)
        .collect();
    let mut normalized_source = None;
    let mut normalized_status = None;
    let mut bindings = Vec::new();
    let mut replay_verified = false;
    if let Some(index) = selected {
        let candidate = &candidates[index];
        let normalized = normalize_equation(&format!("\\({}\\)", candidate.source));
        normalized_status = Some(normalized.status);
        normalized_source = normalized.normalized_source;
        bindings = normalized.symbol_bindings;
        replay_verified = normalized.replay_verified;
        if normalized.status != NormalizationStatus::Accepted {
            status = GroundingStatus::Unsupported;
            reason = "target-linked region is outside the bounded notation contract".into();
        }
    }
    let target = QuestionTarget {
        requested_operation: operation,
        requested_entity: None,
        candidate_regions: candidates,
        supporting_region_indices: supporting,
        selected_region_index: selected,
        rejected_region_indices: rejected,
        unresolved_alternatives: unresolved,
        provenance_spans: spans.iter().map(|(_, span, _, _)| span.clone()).collect(),
    };
    let receipt_hash = receipt_hash(&(
        status,
        &target,
        &normalized_source,
        &bindings,
        replay_verified,
    ));
    GroundingResult {
        status,
        target,
        normalized_source,
        normalized_status,
        symbol_bindings: bindings,
        replay_verified,
        receipt_hash,
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_target_after_definition() {
        let result = ground_math_target("Let $x=2$ be defined. Find $y=x+1$.");
        assert_eq!(result.status, GroundingStatus::Accepted, "{result:?}");
        assert_eq!(result.target.selected_region_index, Some(1));
        assert_eq!(result.target.supporting_region_indices, vec![0]);
        assert!(result.replay_verified);
    }

    #[test]
    fn preserves_multiple_plausible_targets() {
        let result = ground_math_target("Given $x=1$, find either $x+1$ or $x+2$.");
        assert_eq!(result.status, GroundingStatus::Ambiguous);
        assert_eq!(result.target.unresolved_alternatives, vec![1, 2]);
        assert!(result.target.selected_region_index.is_none());
    }

    #[test]
    fn rejects_incidental_regions_without_a_request() {
        let result = ground_math_target("The definition $x=1$ is cited here.");
        assert_eq!(result.status, GroundingStatus::Unsupported);
        assert!(result.target.selected_region_index.is_none());
    }
}
