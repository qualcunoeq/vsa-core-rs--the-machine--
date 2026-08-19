//! Narrow technical-language frontend for the source-derived finite-topology pack.
//!
//! The frontend accepts explicit finite carriers, open-set declarations, and
//! one bounded set operation. It never infers a topology from words such as
//! "continuous" or "neighborhood" and preserves multiple candidate carriers
//! as ambiguity.

use crate::source_topology_pack::{TopologyOperation, TopologyRequest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TopologyFrontendStatus {
    Complete,
    Ambiguous,
    Unsupported,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TopologyFrontendResult {
    pub status: TopologyFrontendStatus,
    pub request: Option<TopologyRequest>,
    pub candidate_spans: Vec<String>,
    pub unresolved_alternatives: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("topology frontend serializes"))
    )
}

fn payload(result: &TopologyFrontendResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.request,
        &result.candidate_spans,
        &result.unresolved_alternatives,
        &result.reasons,
        &result.provenance,
    )
}

fn output(
    status: TopologyFrontendStatus,
    request: Option<TopologyRequest>,
    candidates: Vec<String>,
    alternatives: Vec<String>,
    reasons: Vec<String>,
    text: &str,
) -> TopologyFrontendResult {
    let mut result = TopologyFrontendResult {
        status,
        request,
        candidate_spans: candidates,
        unresolved_alternatives: alternatives,
        reasons,
        provenance: vec![format!(
            "topology-frontend-text-sha256:{:x}",
            Sha256::digest(text.as_bytes())
        )],
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn braces_after<'a>(text: &'a str, marker: &str, stops: &[&str]) -> Vec<&'a str> {
    let lower = text.to_ascii_lowercase();
    let marker = marker.to_ascii_lowercase();
    let Some(start) = lower.find(&marker) else {
        return Vec::new();
    };
    let suffix = &text[start + marker.len()..];
    let suffix_end = stops
        .iter()
        .filter_map(|stop| lower[start + marker.len()..].find(&stop.to_ascii_lowercase()))
        .min()
        .unwrap_or(suffix.len());
    let suffix = &suffix[..suffix_end];
    let mut spans = Vec::new();
    let mut open = None;
    for (index, character) in suffix.char_indices() {
        match character {
            '{' if open.is_none() => open = Some(index),
            '}' if open.is_some() => {
                let begin = open.take().expect("opening brace exists");
                spans.push(&suffix[begin + 1..index]);
            }
            _ => {}
        }
    }
    spans
}

fn parse_set(value: &str) -> Option<Vec<String>> {
    let mut values = value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(String::from)
        .collect::<Vec<_>>();
    values.sort();
    if values.is_empty() || values.windows(2).any(|pair| pair[0] == pair[1]) {
        return None;
    }
    Some(values)
}

fn parse_operation(lower: &str) -> Option<TopologyOperation> {
    if lower.contains("validate topology") || lower.contains("is this a topology") {
        Some(TopologyOperation::ValidateTopology)
    } else if lower.contains("is open") {
        Some(TopologyOperation::IsOpen)
    } else if lower.contains("is closed") || lower.contains("closed set") {
        Some(TopologyOperation::IsClosed)
    } else if lower.contains("interior") {
        Some(TopologyOperation::Interior)
    } else if lower.contains("closure") {
        Some(TopologyOperation::Closure)
    } else {
        None
    }
}

/// Formalize explicit bounded finite-topology language into a typed request.
pub fn formalize_topology_text(text: &str) -> TopologyFrontendResult {
    let lower = text.to_ascii_lowercase();
    let unsupported_terms = [
        "metric",
        "infinite",
        "homology",
        "hausdorff",
        "compact",
        "connected",
        "manifold",
    ];
    if unsupported_terms.iter().any(|term| lower.contains(term)) {
        return output(
            TopologyFrontendStatus::Unsupported,
            None,
            Vec::new(),
            Vec::new(),
            vec!["requested topology semantics exceed the finite-set boundary".into()],
            text,
        );
    }
    let point_candidates = braces_after(text, "points:", &["target:", "open sets:"]);
    if point_candidates.is_empty() {
        return output(
            TopologyFrontendStatus::Missing,
            None,
            Vec::new(),
            Vec::new(),
            vec!["an explicit points: carrier is required".into()],
            text,
        );
    }
    if point_candidates.len() != 1 {
        return output(
            TopologyFrontendStatus::Ambiguous,
            None,
            point_candidates.iter().map(|span| (*span).into()).collect(),
            Vec::new(),
            vec!["multiple carrier declarations require explicit target selection".into()],
            text,
        );
    }
    let Some(points) = parse_set(point_candidates[0]) else {
        return output(
            TopologyFrontendStatus::Ambiguous,
            None,
            vec![point_candidates[0].into()],
            Vec::new(),
            vec!["carrier notation is empty or duplicated".into()],
            text,
        );
    };
    if points.len() > 8 {
        return output(
            TopologyFrontendStatus::Unsupported,
            None,
            vec![point_candidates[0].into()],
            Vec::new(),
            vec!["carrier exceeds the finite source bound".into()],
            text,
        );
    }
    let open_candidates = braces_after(text, "open sets:", &[]);
    if open_candidates.is_empty() {
        return output(
            TopologyFrontendStatus::Missing,
            None,
            vec![point_candidates[0].into()],
            Vec::new(),
            vec!["an explicit open sets: declaration is required".into()],
            text,
        );
    }
    let mut open_sets = Vec::new();
    for span in open_candidates {
        let Some(set) = parse_set(span) else {
            if span.trim().is_empty() {
                open_sets.push(Vec::new());
                continue;
            }
            return output(
                TopologyFrontendStatus::Ambiguous,
                None,
                vec![span.into()],
                Vec::new(),
                vec!["an open-set declaration is malformed".into()],
                text,
            );
        };
        open_sets.push(set);
    }
    let Some(operation) = parse_operation(&lower) else {
        return output(
            TopologyFrontendStatus::Missing,
            None,
            vec![point_candidates[0].into()],
            Vec::new(),
            vec!["the requested finite-topology operation is not explicit".into()],
            text,
        );
    };
    let target_candidates = braces_after(text, "target:", &["open sets:"]);
    let target = if matches!(operation, TopologyOperation::ValidateTopology) {
        None
    } else {
        if target_candidates.len() != 1 {
            return output(
                TopologyFrontendStatus::Ambiguous,
                None,
                target_candidates
                    .iter()
                    .map(|span| (*span).into())
                    .collect(),
                Vec::new(),
                vec!["a unique target set is required for this operation".into()],
                text,
            );
        }
        parse_set(target_candidates[0])
    };
    if !matches!(operation, TopologyOperation::ValidateTopology) && target.is_none() {
        return output(
            TopologyFrontendStatus::Ambiguous,
            None,
            Vec::new(),
            Vec::new(),
            vec!["target notation is empty or duplicated".into()],
            text,
        );
    }
    let request = TopologyRequest {
        operation,
        topology: "finite_topology_axioms".into(),
        points,
        open_sets,
        target_set: target,
        domain: "source_derived_finite_topology".into(),
        ambiguity: None,
        provenance: vec![format!(
            "topology-frontend-text-sha256:{:x}",
            Sha256::digest(text.as_bytes())
        )],
    };
    output(
        TopologyFrontendStatus::Complete,
        Some(request),
        vec!["points".into(), "open sets".into()],
        Vec::new(),
        Vec::new(),
        text,
    )
}

impl TopologyFrontendResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != TopologyFrontendStatus::Complete || self.request.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_finite_topology_is_typed() {
        let result = formalize_topology_text("Find the interior; points: {a,b,c}; target: {a,b}; open sets: {}; open sets: {a}; open sets: {a,b,c}.");
        assert_eq!(result.status, TopologyFrontendStatus::Complete);
        assert!(result.replay_verified());
    }

    #[test]
    fn unsupported_metric_language_fails_closed() {
        let result = formalize_topology_text("Determine whether the metric space is compact.");
        assert_eq!(result.status, TopologyFrontendStatus::Unsupported);
        assert!(result.replay_verified());
    }
}
