//! Conservative coordinate-preserving Cartesian-plot frontend.
//!
//! A plot observation is lowered only when axis identity, bounds, point
//! coordinates, plot kind, confidence, and provenance are explicit.  The
//! frontend does not infer a function, interpolation, monotonicity, or a
//! numerical value from pixels.  It emits a typed perception artifact for a
//! later, separately governed consumer.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const DOMAIN: &str = "visual_cartesian_plot";
const MAX_POINTS: usize = 32;
const MAX_SEGMENTS: usize = 64;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlotStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlotAxisObservation {
    pub label: String,
    pub minimum: i32,
    pub maximum: i32,
    pub confidence: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlotPointObservation {
    pub label: Option<String>,
    pub x: i32,
    pub y: i32,
    pub confidence: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlotSegmentObservation {
    pub from: usize,
    pub to: usize,
    pub confidence: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualPlotObservation {
    pub semantic_label: Option<String>,
    pub x_axis: Option<PlotAxisObservation>,
    pub y_axis: Option<PlotAxisObservation>,
    pub kind: Option<String>,
    pub units: Option<(String, String)>,
    pub points: Vec<PlotPointObservation>,
    pub segments: Vec<PlotSegmentObservation>,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualPlotArtifact {
    pub x_axis: PlotAxisObservation,
    pub y_axis: PlotAxisObservation,
    pub kind: String,
    pub units: Option<(String, String)>,
    pub points: Vec<PlotPointObservation>,
    pub segments: Vec<PlotSegmentObservation>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualPlotResult {
    pub status: PlotStatus,
    pub artifact: Option<VisualPlotArtifact>,
    pub alternatives: Vec<String>,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("visual plot serializes"))
    )
}

fn result(
    status: PlotStatus,
    artifact: Option<VisualPlotArtifact>,
    alternatives: Vec<String>,
    reasons: Vec<String>,
) -> VisualPlotResult {
    let mut output = VisualPlotResult {
        status,
        artifact,
        alternatives,
        reasons,
        replay_hash: String::new(),
    };
    output.replay_hash = digest(&(
        output.status,
        &output.artifact,
        &output.alternatives,
        &output.reasons,
    ));
    output
}

/// Formalize an explicitly identified bounded Cartesian plot.
pub fn formalize_visual_plot(input: &VisualPlotObservation) -> VisualPlotResult {
    if let Some(ambiguity) = &input.ambiguity {
        return result(
            PlotStatus::Ambiguous,
            None,
            vec![ambiguity.clone()],
            vec!["visual extractor reported unresolved plot alternatives".into()],
        );
    }
    if input.provenance.is_empty() {
        return result(
            PlotStatus::Missing,
            None,
            Vec::new(),
            vec!["plot observations need provenance".into()],
        );
    }
    if input.semantic_label.as_deref() != Some("cartesian_plot") {
        return result(
            PlotStatus::Unsupported,
            None,
            Vec::new(),
            vec!["visual geometry does not establish cartesian_plot semantics".into()],
        );
    }
    let (Some(x_axis), Some(y_axis)) = (&input.x_axis, &input.y_axis) else {
        return result(
            PlotStatus::Missing,
            None,
            Vec::new(),
            vec!["both explicit x and y axes are required".into()],
        );
    };
    if x_axis.label.trim().is_empty() || y_axis.label.trim().is_empty() {
        return result(
            PlotStatus::Missing,
            None,
            Vec::new(),
            vec!["axis labels must be explicit".into()],
        );
    }
    if x_axis.minimum >= x_axis.maximum || y_axis.minimum >= y_axis.maximum {
        return result(
            PlotStatus::Invalid,
            None,
            Vec::new(),
            vec!["axis bounds must be ordered and non-empty".into()],
        );
    }
    if x_axis.confidence < 80 || y_axis.confidence < 80 {
        return result(
            PlotStatus::Ambiguous,
            None,
            vec!["axis bounds".into()],
            vec!["axis confidence is below the semantic boundary".into()],
        );
    }
    let Some(kind) = &input.kind else {
        return result(
            PlotStatus::Ambiguous,
            None,
            vec!["scatter".into(), "line".into()],
            vec!["plot kind is not explicit".into()],
        );
    };
    if !matches!(kind.as_str(), "scatter" | "line") {
        return result(
            PlotStatus::Unsupported,
            None,
            Vec::new(),
            vec!["only bounded scatter and line plots are supported".into()],
        );
    }
    if let Some((x_unit, y_unit)) = &input.units {
        if x_unit.trim().is_empty() || y_unit.trim().is_empty() {
            return result(
                PlotStatus::Missing,
                None,
                Vec::new(),
                vec!["declared plot units must be non-empty".into()],
            );
        }
    }
    if input.points.is_empty() {
        return result(
            PlotStatus::Missing,
            None,
            Vec::new(),
            vec!["at least one explicit plot point is required".into()],
        );
    }
    if input.points.len() > MAX_POINTS || input.segments.len() > MAX_SEGMENTS {
        return result(
            PlotStatus::Unsupported,
            None,
            Vec::new(),
            vec!["plot exceeds the bounded point or segment budget".into()],
        );
    }
    let mut labels = BTreeSet::new();
    let mut coordinates = BTreeSet::new();
    for point in &input.points {
        if point.x < x_axis.minimum
            || point.x > x_axis.maximum
            || point.y < y_axis.minimum
            || point.y > y_axis.maximum
        {
            return result(
                PlotStatus::Invalid,
                None,
                Vec::new(),
                vec!["plot point lies outside explicit axis bounds".into()],
            );
        }
        if point.confidence < 80 {
            return result(
                PlotStatus::Ambiguous,
                None,
                vec![format!("point({}, {})", point.x, point.y)],
                vec!["point confidence is below the semantic boundary".into()],
            );
        }
        if !coordinates.insert((point.x, point.y)) {
            return result(
                PlotStatus::Invalid,
                None,
                Vec::new(),
                vec!["duplicate point coordinates are not identity-safe".into()],
            );
        }
        if let Some(label) = &point.label {
            if label.trim().is_empty() || !labels.insert(label.clone()) {
                return result(
                    PlotStatus::Invalid,
                    None,
                    Vec::new(),
                    vec!["point labels must be unique when present".into()],
                );
            }
        }
    }
    if kind == "scatter" && !input.segments.is_empty() {
        return result(
            PlotStatus::Unsupported,
            None,
            Vec::new(),
            vec!["scatter plots do not authorize connecting segments".into()],
        );
    }
    for segment in &input.segments {
        if segment.from >= input.points.len() || segment.to >= input.points.len() {
            return result(
                PlotStatus::Invalid,
                None,
                Vec::new(),
                vec!["plot segment references an unknown point".into()],
            );
        }
        if segment.from == segment.to {
            return result(
                PlotStatus::Unsupported,
                None,
                Vec::new(),
                vec!["self-segments are outside the bounded plot boundary".into()],
            );
        }
        if segment.confidence < 80 {
            return result(
                PlotStatus::Ambiguous,
                None,
                vec![format!("segment {} -> {}", segment.from, segment.to)],
                vec!["segment confidence is below the semantic boundary".into()],
            );
        }
    }
    if kind == "line" && input.segments.is_empty() && input.points.len() > 1 {
        return result(
            PlotStatus::Missing,
            None,
            Vec::new(),
            vec!["line plots require explicit point connectivity".into()],
        );
    }
    result(
        PlotStatus::Complete,
        Some(VisualPlotArtifact {
            x_axis: x_axis.clone(),
            y_axis: y_axis.clone(),
            kind: kind.clone(),
            units: input.units.clone(),
            points: input.points.clone(),
            segments: input.segments.clone(),
            provenance: input.provenance.clone(),
        }),
        Vec::new(),
        Vec::new(),
    )
}

impl VisualPlotResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash
            == digest(&(
                self.status,
                &self.artifact,
                &self.alternatives,
                &self.reasons,
            ))
    }

    pub fn authorized(&self) -> bool {
        self.status == PlotStatus::Complete
            && self.artifact.as_ref().is_some_and(|artifact| {
                !artifact.provenance.is_empty() && !artifact.points.is_empty()
            })
            && self.replay_verified()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation() -> VisualPlotObservation {
        VisualPlotObservation {
            semantic_label: Some("cartesian_plot".into()),
            x_axis: Some(PlotAxisObservation {
                label: "time".into(),
                minimum: 0,
                maximum: 10,
                confidence: 99,
            }),
            y_axis: Some(PlotAxisObservation {
                label: "value".into(),
                minimum: 0,
                maximum: 10,
                confidence: 99,
            }),
            kind: Some("line".into()),
            units: None,
            points: vec![
                PlotPointObservation {
                    label: Some("p0".into()),
                    x: 1,
                    y: 2,
                    confidence: 99,
                },
                PlotPointObservation {
                    label: Some("p1".into()),
                    x: 4,
                    y: 8,
                    confidence: 99,
                },
            ],
            segments: vec![PlotSegmentObservation {
                from: 0,
                to: 1,
                confidence: 99,
            }],
            ambiguity: None,
            provenance: vec!["plot:test".into()],
        }
    }

    #[test]
    fn explicit_plot_replays() {
        let result = formalize_visual_plot(&observation());
        assert_eq!(result.status, PlotStatus::Complete);
        assert!(result.authorized());
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        assert!(!tampered.replay_verified());
    }

    #[test]
    fn missing_kind_remains_ambiguous() {
        let mut input = observation();
        input.kind = None;
        assert_eq!(formalize_visual_plot(&input).status, PlotStatus::Ambiguous);
    }
}
