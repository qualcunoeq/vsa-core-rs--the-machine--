//! Source-derived bounded finite-metric pack.
//!
//! The metric axioms are extracted from an attributed source transcription.
//! Execution is restricted to explicit finite carriers and exact integer
//! distances.  No completeness, compactness, limiting, or infinite-space
//! semantics are inferred.

use crate::source_formula_pack::{validate_source_citation, SourceCitation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[path = "source_metric_frontend.rs"]
pub mod source_metric_frontend;

const MAX_POINTS: usize = 8;
pub const DOMAIN: &str = "source_derived_finite_metric";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetricDefinitionRecord {
    pub metric_id: String,
    pub aliases: Vec<String>,
    pub domain: String,
    pub max_points: usize,
    pub axioms: Vec<String>,
    pub source: SourceCitation,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MetricOperation {
    ValidateMetric,
    Distance,
    OpenBall,
    Diameter,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DistanceEntry {
    pub left: String,
    pub right: String,
    pub distance: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetricRequest {
    pub operation: MetricOperation,
    pub metric: String,
    pub points: Vec<String>,
    pub distances: Vec<DistanceEntry>,
    pub center: Option<String>,
    pub target: Option<String>,
    pub radius: Option<i64>,
    pub domain: String,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MetricArtifact {
    ValidatedMetric {
        points: Vec<String>,
        distances: Vec<DistanceEntry>,
    },
    Distance(i64),
    Set(Vec<String>),
    Scalar(i64),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MetricStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidDomain,
    Inconsistent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetricResult {
    pub status: MetricStatus,
    pub artifact: Option<MetricArtifact>,
    pub source: Option<SourceCitation>,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("metric value serializes"))
    )
}

fn payload(result: &MetricResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.artifact,
        &result.source,
        &result.assumptions,
        &result.reasons,
        &result.provenance,
    )
}

fn output(
    request: &MetricRequest,
    status: MetricStatus,
    artifact: Option<MetricArtifact>,
    source: Option<SourceCitation>,
    assumptions: Vec<String>,
    reasons: Vec<String>,
) -> MetricResult {
    let mut result = MetricResult {
        status,
        artifact,
        source,
        assumptions,
        reasons,
        provenance: request.provenance.clone(),
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn list(value: &str, separator: char) -> Vec<String> {
    value
        .split(separator)
        .map(str::trim)
        .filter(|item| !item.is_empty() && *item != "-")
        .map(String::from)
        .collect()
}

/// Extract metric-definition records from the attributed source text.
pub fn extract_metric_definitions(
    document: &str,
) -> Result<Vec<MetricDefinitionRecord>, Vec<String>> {
    let mut errors = Vec::new();
    let mut blocks = Vec::new();
    let mut current: Option<(usize, BTreeMap<String, String>)> = None;
    for (index, raw) in document.lines().enumerate() {
        let line_number = index + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "BEGIN METRIC" {
            if current.is_some() {
                errors.push(format!("nested metric block at line {line_number}"));
            } else {
                current = Some((line_number, BTreeMap::new()));
            }
            continue;
        }
        if line == "END METRIC" {
            if let Some(block) = current.take() {
                blocks.push(block);
            } else {
                errors.push(format!("orphan metric terminator at line {line_number}"));
            }
            continue;
        }
        let Some((_, fields)) = current.as_mut() else {
            errors.push(format!("field outside metric block at line {line_number}"));
            continue;
        };
        let Some((key, value)) = line.split_once(':') else {
            errors.push(format!("malformed metric field at line {line_number}"));
            continue;
        };
        let key = key.trim().to_ascii_uppercase();
        let value = value.trim().to_string();
        if key.is_empty() || value.is_empty() || fields.insert(key.clone(), value).is_some() {
            errors.push(format!(
                "invalid or duplicate metric field {key} at line {line_number}"
            ));
        }
    }
    if let Some((line, _)) = current {
        errors.push(format!(
            "metric block beginning at line {line} is unterminated"
        ));
    }
    let mut records = Vec::new();
    for (line, fields) in blocks {
        let required = |key: &str| {
            fields
                .get(key)
                .cloned()
                .ok_or_else(|| format!("metric block at line {line} lacks {key}"))
        };
        let record = (|| -> Result<MetricDefinitionRecord, String> {
            let max_points = required("MAX_POINTS")?
                .parse::<usize>()
                .map_err(|_| "MAX_POINTS is not an integer".to_string())?;
            Ok(MetricDefinitionRecord {
                metric_id: required("METRIC_ID")?,
                aliases: list(&required("ALIASES")?, '|'),
                domain: required("DOMAIN")?,
                max_points,
                axioms: list(&required("AXIOMS")?, ';'),
                source: SourceCitation {
                    source_id: required("SOURCE_ID")?,
                    title: required("TITLE")?,
                    section: required("SECTION")?,
                    url: required("URL")?,
                    license: required("LICENSE")?,
                    retrieved_utc: required("RETRIEVED")?,
                    evidence_span: required("EVIDENCE")?,
                },
            })
        })();
        match record {
            Ok(record) => records.push(record),
            Err(error) => errors.push(format!("line {line}: {error}")),
        }
    }
    if let Err(validation_errors) = validate_metric_definitions(&records) {
        errors.extend(validation_errors);
    }
    if errors.is_empty() {
        Ok(records)
    } else {
        Err(errors)
    }
}

pub fn validate_metric_definitions(records: &[MetricDefinitionRecord]) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let mut ids = BTreeSet::new();
    let mut aliases = BTreeSet::new();
    for record in records {
        if record.metric_id.trim().is_empty() || !ids.insert(record.metric_id.clone()) {
            errors.push(format!(
                "duplicate or empty metric identifier: {}",
                record.metric_id
            ));
        }
        if record.domain.trim().is_empty()
            || record.max_points == 0
            || record.max_points > MAX_POINTS
        {
            errors.push(format!(
                "metric {} has an invalid domain or point bound",
                record.metric_id
            ));
        }
        for alias in &record.aliases {
            if alias.trim().is_empty() || !aliases.insert(alias.clone()) {
                errors.push(format!(
                    "duplicate or empty metric alias in {}",
                    record.metric_id
                ));
            }
        }
        for axiom in ["nonnegative", "identity", "symmetry", "triangle"] {
            if !record.axioms.iter().any(|candidate| candidate == axiom) {
                errors.push(format!("metric {} lacks {axiom} axiom", record.metric_id));
            }
        }
        if let Err(citation_errors) = validate_source_citation(&record.source) {
            for error in citation_errors {
                errors.push(format!("metric {}: {error}", record.metric_id));
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn canonical_points(points: &[String]) -> Option<Vec<String>> {
    let mut points = points.to_vec();
    points.sort();
    if points.is_empty()
        || points.len() > MAX_POINTS
        || points.windows(2).any(|pair| pair[0] == pair[1])
    {
        return None;
    }
    Some(points)
}

fn select_record<'a>(
    request: &MetricRequest,
    records: &'a [MetricDefinitionRecord],
) -> Result<&'a MetricDefinitionRecord, MetricResult> {
    if request.domain != DOMAIN {
        return Err(output(
            request,
            MetricStatus::InvalidDomain,
            None,
            None,
            Vec::new(),
            vec!["metric domain is outside the source-derived finite scope".into()],
        ));
    }
    if let Some(ambiguity) = &request.ambiguity {
        return Err(output(
            request,
            MetricStatus::Ambiguous,
            None,
            None,
            Vec::new(),
            vec![ambiguity.clone()],
        ));
    }
    let matches: Vec<_> = records
        .iter()
        .filter(|record| {
            record.domain == request.domain
                && (record.metric_id == request.metric
                    || record.aliases.iter().any(|alias| alias == &request.metric))
        })
        .collect();
    if matches.len() != 1 {
        return Err(output(
            request,
            if matches.is_empty() {
                MetricStatus::Missing
            } else {
                MetricStatus::Ambiguous
            },
            None,
            None,
            Vec::new(),
            vec!["metric identifier does not select exactly one source definition".into()],
        ));
    }
    Ok(matches[0])
}

fn distance_map(
    request: &MetricRequest,
    points: &[String],
) -> Result<BTreeMap<(String, String), i64>, String> {
    let point_set: BTreeSet<_> = points.iter().cloned().collect();
    let mut map = BTreeMap::new();
    for entry in &request.distances {
        if !point_set.contains(&entry.left) || !point_set.contains(&entry.right) {
            return Err("distance entry names a point outside the carrier".into());
        }
        let key = if entry.left <= entry.right {
            (entry.left.clone(), entry.right.clone())
        } else {
            (entry.right.clone(), entry.left.clone())
        };
        if map.insert(key, entry.distance).is_some() {
            return Err("distance table contains duplicate unordered pairs".into());
        }
    }
    for left in points {
        for right in points {
            let key = if left <= right {
                (left.clone(), right.clone())
            } else {
                (right.clone(), left.clone())
            };
            if !map.contains_key(&key) {
                return Err("distance table is incomplete".into());
            }
        }
    }
    Ok(map)
}

fn validate_metric(points: &[String], map: &BTreeMap<(String, String), i64>) -> Result<(), String> {
    for left in points {
        for right in points {
            let key = if left <= right {
                (left.clone(), right.clone())
            } else {
                (right.clone(), left.clone())
            };
            let distance = map[&key];
            if distance < 0 {
                return Err("metric distance is negative".into());
            }
            if left == right && distance != 0 {
                return Err("identity axiom is violated".into());
            }
            if left != right && distance == 0 {
                return Err("distinct points have zero distance".into());
            }
        }
    }
    for left in points {
        for middle in points {
            for right in points {
                let pair = |a: &String, b: &String| {
                    let key = if a <= b {
                        (a.clone(), b.clone())
                    } else {
                        (b.clone(), a.clone())
                    };
                    map[&key]
                };
                if pair(left, right) > pair(left, middle) + pair(middle, right) {
                    return Err("triangle inequality is violated".into());
                }
            }
        }
    }
    Ok(())
}

/// Evaluate a source-derived finite metric request.
pub fn evaluate_metric(
    request: &MetricRequest,
    records: &[MetricDefinitionRecord],
) -> MetricResult {
    let record = match select_record(request, records) {
        Ok(record) => record,
        Err(result) => return result,
    };
    let Some(points) = canonical_points(&request.points) else {
        return output(
            request,
            MetricStatus::Inconsistent,
            None,
            Some(record.source.clone()),
            record.axioms.clone(),
            vec!["carrier is empty, duplicated, or exceeds the finite bound".into()],
        );
    };
    if points.len() > record.max_points {
        return output(
            request,
            MetricStatus::Unsupported,
            None,
            Some(record.source.clone()),
            record.axioms.clone(),
            vec!["carrier exceeds the source-declared point bound".into()],
        );
    }
    let map = match distance_map(request, &points) {
        Ok(map) => map,
        Err(reason) => {
            return output(
                request,
                MetricStatus::Inconsistent,
                None,
                Some(record.source.clone()),
                record.axioms.clone(),
                vec![reason],
            )
        }
    };
    if let Err(reason) = validate_metric(&points, &map) {
        return output(
            request,
            MetricStatus::Inconsistent,
            None,
            Some(record.source.clone()),
            record.axioms.clone(),
            vec![reason],
        );
    }
    let mut entries = Vec::new();
    for (left_index, left) in points.iter().enumerate() {
        for right in points.iter().skip(left_index) {
            let key = if left <= right {
                (left.clone(), right.clone())
            } else {
                (right.clone(), left.clone())
            };
            entries.push(DistanceEntry {
                left: left.clone(),
                right: right.clone(),
                distance: map[&key],
            });
        }
    }
    let artifact = match request.operation {
        MetricOperation::ValidateMetric => MetricArtifact::ValidatedMetric {
            points: points.clone(),
            distances: entries,
        },
        MetricOperation::Distance => {
            let (Some(center), Some(target)) = (request.center.as_ref(), request.target.as_ref())
            else {
                return output(
                    request,
                    MetricStatus::Missing,
                    None,
                    Some(record.source.clone()),
                    record.axioms.clone(),
                    vec!["distance operation requires center and target".into()],
                );
            };
            let key = if center <= target {
                (center.clone(), target.clone())
            } else {
                (target.clone(), center.clone())
            };
            let Some(distance) = map.get(&key) else {
                return output(
                    request,
                    MetricStatus::Inconsistent,
                    None,
                    Some(record.source.clone()),
                    record.axioms.clone(),
                    vec!["distance target is outside the carrier".into()],
                );
            };
            MetricArtifact::Distance(*distance)
        }
        MetricOperation::OpenBall => {
            let (Some(center), Some(radius)) = (request.center.as_ref(), request.radius) else {
                return output(
                    request,
                    MetricStatus::Missing,
                    None,
                    Some(record.source.clone()),
                    record.axioms.clone(),
                    vec!["open-ball operation requires center and radius".into()],
                );
            };
            if radius < 0 || !points.contains(center) {
                return output(
                    request,
                    MetricStatus::Inconsistent,
                    None,
                    Some(record.source.clone()),
                    record.axioms.clone(),
                    vec!["ball radius or center is invalid".into()],
                );
            }
            let members = points
                .iter()
                .filter(|point| {
                    let key = if *point <= center {
                        ((*point).clone(), center.clone())
                    } else {
                        (center.clone(), (*point).clone())
                    };
                    map[&key] < radius
                })
                .cloned()
                .collect();
            MetricArtifact::Set(members)
        }
        MetricOperation::Diameter => {
            MetricArtifact::Scalar(map.values().copied().max().unwrap_or(0))
        }
    };
    output(
        request,
        MetricStatus::Complete,
        Some(artifact),
        Some(record.source.clone()),
        record.axioms.clone(),
        Vec::new(),
    )
}

impl MetricResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != MetricStatus::Complete || self.artifact.is_some())
            && (self.status != MetricStatus::Complete || self.source.is_some())
    }

    pub fn authorized(&self) -> bool {
        self.status == MetricStatus::Complete && self.replay_verified()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> MetricDefinitionRecord {
        MetricDefinitionRecord {
            metric_id: "finite_metric_axioms".into(),
            aliases: vec!["finite metric".into()],
            domain: DOMAIN.into(),
            max_points: 8,
            axioms: vec![
                "nonnegative".into(),
                "identity".into(),
                "symmetry".into(),
                "triangle".into(),
            ],
            source: SourceCitation {
                source_id: "test".into(),
                title: "test".into(),
                section: "1".into(),
                url: "https://example.test".into(),
                license: "test".into(),
                retrieved_utc: "2026-01-01".into(),
                evidence_span: "metric definition".into(),
            },
        }
    }

    fn request(operation: MetricOperation) -> MetricRequest {
        MetricRequest {
            operation,
            metric: "finite_metric_axioms".into(),
            points: vec!["a".into(), "b".into(), "c".into()],
            distances: vec![
                DistanceEntry {
                    left: "a".into(),
                    right: "a".into(),
                    distance: 0,
                },
                DistanceEntry {
                    left: "b".into(),
                    right: "b".into(),
                    distance: 0,
                },
                DistanceEntry {
                    left: "c".into(),
                    right: "c".into(),
                    distance: 0,
                },
                DistanceEntry {
                    left: "a".into(),
                    right: "b".into(),
                    distance: 1,
                },
                DistanceEntry {
                    left: "a".into(),
                    right: "c".into(),
                    distance: 2,
                },
                DistanceEntry {
                    left: "b".into(),
                    right: "c".into(),
                    distance: 1,
                },
            ],
            center: Some("a".into()),
            target: Some("c".into()),
            radius: Some(2),
            domain: DOMAIN.into(),
            ambiguity: None,
            provenance: vec!["test".into()],
        }
    }

    #[test]
    fn finite_metric_validates_and_replays() {
        let result = evaluate_metric(&request(MetricOperation::Distance), &[record()]);
        assert_eq!(result.artifact, Some(MetricArtifact::Distance(2)));
        assert!(result.authorized());
    }

    #[test]
    fn triangle_violation_fails_closed() {
        let mut request = request(MetricOperation::ValidateMetric);
        request.distances[4].distance = 5;
        let result = evaluate_metric(&request, &[record()]);
        assert_eq!(result.status, MetricStatus::Inconsistent);
        assert!(result.replay_verified());
    }
}
