//! Conservative coordinate-preserving geometry-diagram frontend.
//!
//! The frontend records explicit geometric observations only.  Coordinates do
//! not authorize collinearity, parallelism, equality, incidence, or a proof;
//! those relations must be supplied explicitly by the upstream extractor and
//! are carried as provenance-bound observations for a later solver.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const DOMAIN: &str = "visual_cartesian_geometry";
const MAX_POINTS: usize = 32;
const MAX_SEGMENTS: usize = 48;
const MAX_CIRCLES: usize = 16;
const MAX_RELATIONS: usize = 64;
const COORDINATE_BOUND: i32 = 10_000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GeometryStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GeometryPointObservation {
    pub label: String,
    pub x: i32,
    pub y: i32,
    pub confidence: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GeometrySegmentObservation {
    pub id: String,
    pub from: String,
    pub to: String,
    pub confidence: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GeometryCircleObservation {
    pub id: String,
    pub center: String,
    pub radius: i32,
    pub confidence: u8,
}

/// An explicit relationship emitted by a visual extractor.  The frontend
/// accepts only the bounded vocabulary; it never derives these relationships
/// from coordinates.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GeometryRelationObservation {
    pub kind: String,
    pub left: String,
    pub right: String,
    pub confidence: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualGeometryObservation {
    pub semantic_label: Option<String>,
    pub points: Vec<GeometryPointObservation>,
    pub segments: Vec<GeometrySegmentObservation>,
    pub circles: Vec<GeometryCircleObservation>,
    pub relations: Vec<GeometryRelationObservation>,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualGeometryArtifact {
    pub points: Vec<GeometryPointObservation>,
    pub segments: Vec<GeometrySegmentObservation>,
    pub circles: Vec<GeometryCircleObservation>,
    pub relations: Vec<GeometryRelationObservation>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualGeometryResult {
    pub status: GeometryStatus,
    pub artifact: Option<VisualGeometryArtifact>,
    pub alternatives: Vec<String>,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("visual geometry serializes"))
    )
}

fn payload(result: &VisualGeometryResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.artifact,
        &result.alternatives,
        &result.reasons,
    )
}

fn result(
    status: GeometryStatus,
    artifact: Option<VisualGeometryArtifact>,
    alternatives: Vec<String>,
    reasons: Vec<String>,
) -> VisualGeometryResult {
    let mut output = VisualGeometryResult {
        status,
        artifact,
        alternatives,
        reasons,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&(
        output.status,
        &output.artifact,
        &output.alternatives,
        &output.reasons,
    ));
    output.replay_hash = replay_hash;
    output
}

fn object_exists(
    reference: &str,
    points: &BTreeSet<String>,
    segments: &BTreeSet<String>,
    circles: &BTreeSet<String>,
) -> bool {
    points.contains(reference) || segments.contains(reference) || circles.contains(reference)
}

fn object_kind(
    reference: &str,
    points: &BTreeSet<String>,
    segments: &BTreeSet<String>,
    circles: &BTreeSet<String>,
) -> Option<&'static str> {
    if points.contains(reference) {
        Some("point")
    } else if segments.contains(reference) {
        Some("segment")
    } else if circles.contains(reference) {
        Some("circle")
    } else {
        None
    }
}

/// Formalize explicit bounded geometry observations without deriving geometry.
pub fn formalize_visual_geometry(input: &VisualGeometryObservation) -> VisualGeometryResult {
    if let Some(ambiguity) = &input.ambiguity {
        return result(
            GeometryStatus::Ambiguous,
            None,
            vec![ambiguity.clone()],
            vec!["visual extractor reported unresolved geometry alternatives".into()],
        );
    }
    if input.provenance.is_empty() {
        return result(
            GeometryStatus::Missing,
            None,
            Vec::new(),
            vec!["geometry observations need provenance".into()],
        );
    }
    if input.semantic_label.as_deref() != Some("cartesian_geometry_diagram") {
        return result(
            GeometryStatus::Unsupported,
            None,
            Vec::new(),
            vec!["visual geometry does not establish cartesian diagram semantics".into()],
        );
    }
    if input.points.is_empty() {
        return result(
            GeometryStatus::Missing,
            None,
            Vec::new(),
            vec!["at least one explicit geometry point is required".into()],
        );
    }
    if input.points.len() > MAX_POINTS
        || input.segments.len() > MAX_SEGMENTS
        || input.circles.len() > MAX_CIRCLES
        || input.relations.len() > MAX_RELATIONS
    {
        return result(
            GeometryStatus::Unsupported,
            None,
            Vec::new(),
            vec!["geometry exceeds the bounded observation budget".into()],
        );
    }

    let mut points = BTreeSet::new();
    for point in &input.points {
        if point.label.trim().is_empty() {
            return result(
                GeometryStatus::Missing,
                None,
                Vec::new(),
                vec!["geometry point labels must be explicit".into()],
            );
        }
        if !points.insert(point.label.clone()) {
            return result(
                GeometryStatus::Invalid,
                None,
                Vec::new(),
                vec!["duplicate point labels are not identity-safe".into()],
            );
        }
        if point.x.abs() > COORDINATE_BOUND || point.y.abs() > COORDINATE_BOUND {
            return result(
                GeometryStatus::Unsupported,
                None,
                Vec::new(),
                vec!["point coordinate exceeds the bounded diagram range".into()],
            );
        }
        if point.confidence < 80 {
            return result(
                GeometryStatus::Ambiguous,
                None,
                vec![point.label.clone()],
                vec!["point confidence is below the semantic boundary".into()],
            );
        }
    }

    let mut segments = BTreeSet::new();
    for segment in &input.segments {
        if segment.id.trim().is_empty() {
            return result(
                GeometryStatus::Missing,
                None,
                Vec::new(),
                vec!["segment identifiers must be explicit".into()],
            );
        }
        if !segments.insert(segment.id.clone()) {
            return result(
                GeometryStatus::Invalid,
                None,
                Vec::new(),
                vec!["duplicate segment identifiers are not identity-safe".into()],
            );
        }
        if !points.contains(&segment.from) || !points.contains(&segment.to) {
            return result(
                GeometryStatus::Invalid,
                None,
                Vec::new(),
                vec!["segment endpoint is not an explicit point".into()],
            );
        }
        if segment.from == segment.to {
            return result(
                GeometryStatus::Unsupported,
                None,
                Vec::new(),
                vec!["zero-length segments are outside the bounded boundary".into()],
            );
        }
        if segment.confidence < 80 {
            return result(
                GeometryStatus::Ambiguous,
                None,
                vec![segment.id.clone()],
                vec!["segment confidence is below the semantic boundary".into()],
            );
        }
    }

    let mut circles = BTreeSet::new();
    for circle in &input.circles {
        if circle.id.trim().is_empty() {
            return result(
                GeometryStatus::Missing,
                None,
                Vec::new(),
                vec!["circle identifiers must be explicit".into()],
            );
        }
        if !circles.insert(circle.id.clone()) {
            return result(
                GeometryStatus::Invalid,
                None,
                Vec::new(),
                vec!["duplicate circle identifiers are not identity-safe".into()],
            );
        }
        if !points.contains(&circle.center) {
            return result(
                GeometryStatus::Invalid,
                None,
                Vec::new(),
                vec!["circle center is not an explicit point".into()],
            );
        }
        if circle.radius <= 0 {
            return result(
                GeometryStatus::Invalid,
                None,
                Vec::new(),
                vec!["circle radius must be positive".into()],
            );
        }
        if circle.confidence < 80 {
            return result(
                GeometryStatus::Ambiguous,
                None,
                vec![circle.id.clone()],
                vec!["circle confidence is below the semantic boundary".into()],
            );
        }
    }

    let allowed_relations = [
        "collinear",
        "parallel",
        "perpendicular",
        "equal_length",
        "tangent",
    ];
    let mut relation_keys = BTreeSet::new();
    for relation in &input.relations {
        if !allowed_relations.contains(&relation.kind.as_str()) {
            return result(
                GeometryStatus::Unsupported,
                None,
                Vec::new(),
                vec![format!(
                    "geometry relation '{}' is outside the bounded vocabulary",
                    relation.kind
                )],
            );
        }
        if !object_exists(&relation.left, &points, &segments, &circles)
            || !object_exists(&relation.right, &points, &segments, &circles)
        {
            return result(
                GeometryStatus::Invalid,
                None,
                Vec::new(),
                vec!["geometry relation references an unknown object".into()],
            );
        }
        let left_kind = object_kind(&relation.left, &points, &segments, &circles)
            .expect("object existence checked");
        let right_kind = object_kind(&relation.right, &points, &segments, &circles)
            .expect("object existence checked");
        let compatible = if relation.kind == "tangent" {
            (left_kind == "segment" && right_kind == "circle")
                || (left_kind == "circle" && right_kind == "segment")
        } else {
            left_kind == "segment" && right_kind == "segment"
        };
        if !compatible {
            return result(
                GeometryStatus::Unsupported,
                None,
                Vec::new(),
                vec![format!(
                    "relation '{}' is incompatible with {} and {} objects",
                    relation.kind, left_kind, right_kind
                )],
            );
        }
        if relation.left == relation.right {
            return result(
                GeometryStatus::Invalid,
                None,
                Vec::new(),
                vec!["geometry relation cannot relate an object to itself".into()],
            );
        }
        if relation.confidence < 80 {
            return result(
                GeometryStatus::Ambiguous,
                None,
                vec![format!(
                    "{}:{}~{}",
                    relation.kind, relation.left, relation.right
                )],
                vec!["geometry relation confidence is below the semantic boundary".into()],
            );
        }
        let (first, second) = if relation.left <= relation.right {
            (relation.left.clone(), relation.right.clone())
        } else {
            (relation.right.clone(), relation.left.clone())
        };
        let key = (relation.kind.clone(), first, second);
        if !relation_keys.insert(key) {
            return result(
                GeometryStatus::Invalid,
                None,
                Vec::new(),
                vec!["duplicate geometry relations are not identity-safe".into()],
            );
        }
    }

    result(
        GeometryStatus::Complete,
        Some(VisualGeometryArtifact {
            points: input.points.clone(),
            segments: input.segments.clone(),
            circles: input.circles.clone(),
            relations: input.relations.clone(),
            provenance: input.provenance.clone(),
        }),
        Vec::new(),
        Vec::new(),
    )
}

impl VisualGeometryResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
    }

    pub fn authorized(&self) -> bool {
        self.status == GeometryStatus::Complete
            && self.artifact.as_ref().is_some_and(|artifact| {
                !artifact.points.is_empty() && !artifact.provenance.is_empty()
            })
            && self.replay_verified()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation() -> VisualGeometryObservation {
        VisualGeometryObservation {
            semantic_label: Some("cartesian_geometry_diagram".into()),
            points: vec![
                GeometryPointObservation {
                    label: "A".into(),
                    x: 0,
                    y: 0,
                    confidence: 99,
                },
                GeometryPointObservation {
                    label: "B".into(),
                    x: 4,
                    y: 0,
                    confidence: 99,
                },
                GeometryPointObservation {
                    label: "C".into(),
                    x: 0,
                    y: 3,
                    confidence: 99,
                },
            ],
            segments: vec![
                GeometrySegmentObservation {
                    id: "AB".into(),
                    from: "A".into(),
                    to: "B".into(),
                    confidence: 99,
                },
                GeometrySegmentObservation {
                    id: "AC".into(),
                    from: "A".into(),
                    to: "C".into(),
                    confidence: 99,
                },
            ],
            circles: vec![],
            relations: vec![GeometryRelationObservation {
                kind: "perpendicular".into(),
                left: "AB".into(),
                right: "AC".into(),
                confidence: 99,
            }],
            ambiguity: None,
            provenance: vec!["diagram:test".into()],
        }
    }

    #[test]
    fn explicit_geometry_replays() {
        let result = formalize_visual_geometry(&observation());
        assert_eq!(result.status, GeometryStatus::Complete);
        assert!(result.authorized());
        let mut tampered = result.clone();
        tampered.replay_hash.push('x');
        assert!(!tampered.replay_verified());
    }

    #[test]
    fn coordinates_do_not_infer_relations() {
        let mut input = observation();
        input.relations.clear();
        assert_eq!(
            formalize_visual_geometry(&input).status,
            GeometryStatus::Complete
        );
        assert!(formalize_visual_geometry(&input).artifact.is_some());
    }

    #[test]
    fn relation_type_mismatch_is_refused() {
        let mut input = observation();
        input.relations[0].right = "A".into();
        assert_eq!(
            formalize_visual_geometry(&input).status,
            GeometryStatus::Unsupported
        );
    }

    #[test]
    fn symmetric_reverse_relation_is_not_new_evidence() {
        let mut input = observation();
        input.relations.push(GeometryRelationObservation {
            kind: "perpendicular".into(),
            left: "AC".into(),
            right: "AB".into(),
            confidence: 99,
        });
        assert_eq!(
            formalize_visual_geometry(&input).status,
            GeometryStatus::Invalid
        );
    }
}
