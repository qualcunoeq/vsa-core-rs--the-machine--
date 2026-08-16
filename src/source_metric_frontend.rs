//! Fail-closed technical-language frontend for the source-derived finite
//! metric pack.  It accepts only explicit finite carriers and distance tables.

use super::{DistanceEntry, MetricOperation, MetricRequest, DOMAIN};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrontendStatus {
    Complete,
    Ambiguous,
    Unsupported,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetricFrontendResult {
    pub status: FrontendStatus,
    pub operation: Option<MetricOperation>,
    pub request: Option<MetricRequest>,
    pub provenance_spans: Vec<String>,
    pub alternatives: Vec<String>,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn payload(result: &MetricFrontendResult) -> impl Serialize + '_ {
    (
        result.status,
        result.operation,
        &result.request,
        &result.provenance_spans,
        &result.alternatives,
        &result.reasons,
    )
}

fn output(
    status: FrontendStatus,
    operation: Option<MetricOperation>,
    request: Option<MetricRequest>,
    spans: Vec<String>,
    alternatives: Vec<String>,
    reasons: Vec<String>,
) -> MetricFrontendResult {
    let mut result = MetricFrontendResult {
        status,
        operation,
        request,
        provenance_spans: spans,
        alternatives,
        reasons,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn segment_after<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let start = text.to_ascii_lowercase().find(marker)? + marker.len();
    let rest = &text[start..];
    let end = rest
        .find(|character| character == ';' || character == '.')
        .unwrap_or(rest.len());
    Some(rest[..end].trim())
}

fn parse_points(text: &str) -> Option<Vec<String>> {
    let segment = segment_after(text, "points:")?;
    let points = segment
        .split(',')
        .map(str::trim)
        .filter(|point| !point.is_empty())
        .map(String::from)
        .collect::<Vec<_>>();
    if points.is_empty() || points.iter().any(|point| !point.starts_with('p')) {
        None
    } else {
        Some(points)
    }
}

fn parse_distances(text: &str) -> Option<Vec<DistanceEntry>> {
    let segment = segment_after(text, "distances:")?;
    let mut entries = Vec::new();
    for item in segment.split(',') {
        let (pair, value) = item.split_once('=')?;
        let (left, right) = pair.trim().split_once('-')?;
        entries.push(DistanceEntry {
            left: left.trim().to_string(),
            right: right.trim().to_string(),
            distance: value.trim().parse().ok()?,
        });
    }
    (!entries.is_empty()).then_some(entries)
}

fn point_after(text: &str, marker: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let start = lower.find(marker)? + marker.len();
    let token = text[start..]
        .split_whitespace()
        .next()?
        .trim_matches(|character: char| !character.is_ascii_alphanumeric());
    token.starts_with('p').then(|| token.to_string())
}

fn operation_candidates(lower: &str) -> Vec<MetricOperation> {
    let mut operations = Vec::new();
    if lower.contains("validate") || lower.contains("check") || lower.contains("axiom") {
        operations.push(MetricOperation::ValidateMetric);
    }
    if lower.contains("distance from")
        || lower.contains("distance between")
        || lower.contains("determine the distance")
    {
        operations.push(MetricOperation::Distance);
    }
    if lower.contains("open ball") {
        operations.push(MetricOperation::OpenBall);
    }
    if lower.contains("diameter") {
        operations.push(MetricOperation::Diameter);
    }
    operations.sort_by_key(|operation| format!("{operation:?}"));
    operations.dedup();
    operations
}

/// Parse explicit finite metric language into a typed request.
pub fn formalize_metric_text(text: &str) -> MetricFrontendResult {
    let lower = text.to_ascii_lowercase();
    if [
        "infinite",
        "geodesic",
        "manifold",
        "hausdorff",
        "complete metric space",
        "compactness",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        return output(
            FrontendStatus::Unsupported,
            None,
            None,
            vec![text.into()],
            Vec::new(),
            vec!["request exceeds the explicit finite metric boundary".into()],
        );
    }
    if !lower.contains("metric") && !lower.contains("distance function") {
        return output(
            FrontendStatus::Missing,
            None,
            None,
            vec![text.into()],
            Vec::new(),
            vec!["no metric-domain evidence was identified".into()],
        );
    }
    if lower.contains("either") || lower.contains("unspecified") {
        return output(
            FrontendStatus::Ambiguous,
            None,
            None,
            vec![text.into()],
            vec!["multiple metric interpretations".into()],
            vec!["metric operation or carrier is unresolved".into()],
        );
    }
    let operations = operation_candidates(&lower);
    if operations.len() != 1 {
        let table_present = lower.contains("points:") && lower.contains("distances:");
        return output(
            if operations.len() > 1 || table_present {
                FrontendStatus::Ambiguous
            } else {
                FrontendStatus::Missing
            },
            None,
            None,
            vec![text.into()],
            operations
                .iter()
                .map(|operation| format!("{operation:?}"))
                .collect(),
            vec!["exactly one finite metric operation is required".into()],
        );
    }
    let operation = operations[0];
    let Some(points) = parse_points(text) else {
        return output(
            FrontendStatus::Missing,
            Some(operation),
            None,
            vec![text.into()],
            Vec::new(),
            vec!["explicit finite points are required".into()],
        );
    };
    let Some(distances) = parse_distances(text) else {
        return output(
            FrontendStatus::Missing,
            Some(operation),
            None,
            vec![text.into()],
            Vec::new(),
            vec!["an explicit distance table is required".into()],
        );
    };
    let (center, target, radius) = match operation {
        MetricOperation::Distance => {
            let center = point_after(text, "from ").or_else(|| point_after(text, "between "));
            let target = point_after(text, " to ").or_else(|| point_after(text, " and "));
            (center, target, None)
        }
        MetricOperation::OpenBall => {
            let center = point_after(text, "center ").or_else(|| point_after(text, "centered at "));
            let radius = segment_after(text, "radius ").and_then(|value| value.parse().ok());
            (center, None, radius)
        }
        _ => (None, None, None),
    };
    if matches!(operation, MetricOperation::Distance) && (center.is_none() || target.is_none()) {
        return output(
            FrontendStatus::Ambiguous,
            Some(operation),
            None,
            vec![text.into()],
            vec!["distance target pair".into()],
            vec!["distance endpoints are not uniquely bound".into()],
        );
    }
    if matches!(operation, MetricOperation::OpenBall) && (center.is_none() || radius.is_none()) {
        return output(
            FrontendStatus::Ambiguous,
            Some(operation),
            None,
            vec![text.into()],
            vec!["open-ball center and radius".into()],
            vec!["open-ball parameters are not uniquely bound".into()],
        );
    }
    let request = MetricRequest {
        operation,
        metric: "finite_metric_axioms".into(),
        points,
        distances,
        center,
        target,
        radius,
        domain: DOMAIN.into(),
        ambiguity: None,
        provenance: vec![text.into()],
    };
    output(
        FrontendStatus::Complete,
        Some(operation),
        Some(request),
        vec![text.into()],
        Vec::new(),
        Vec::new(),
    )
}

impl MetricFrontendResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self)) && !self.provenance_spans.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finite_metric_request_is_typed_and_replayable() {
        let result = formalize_metric_text(
            "For a finite metric on points: p0,p1,p2; distances: p0-p0=0,p0-p1=1,p0-p2=2,p1-p1=0,p1-p2=1,p2-p2=0; determine the distance from p0 to p2.",
        );
        assert_eq!(result.status, FrontendStatus::Complete);
        assert!(result.request.is_some());
        assert!(result.replay_verified());
    }

    #[test]
    fn unsupported_infinite_metric_is_refused() {
        let result =
            formalize_metric_text("Prove completeness of an infinite geodesic metric space.");
        assert_eq!(result.status, FrontendStatus::Unsupported);
        assert!(result.replay_verified());
    }
}
