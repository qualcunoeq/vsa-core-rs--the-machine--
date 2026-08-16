//! Source-derived bounded finite-topology pack.
//!
//! The topology axioms are extracted from an attributed source document.  The
//! executor is a generic finite-set engine: it validates a declared topology
//! and computes only finite open/closed-set operations.  It contains no
//! benchmark- or question-specific branches and refuses topology claims that
//! require an unbounded or metric representation.

use crate::source_formula_pack::SourceCitation;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const MAX_POINTS: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TopologyDefinitionRecord {
    pub topology_id: String,
    pub aliases: Vec<String>,
    pub domain: String,
    pub max_points: usize,
    pub axioms: Vec<String>,
    pub source: SourceCitation,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TopologyOperation {
    ValidateTopology,
    IsOpen,
    IsClosed,
    Interior,
    Closure,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TopologyRequest {
    pub operation: TopologyOperation,
    pub topology: String,
    pub points: Vec<String>,
    pub open_sets: Vec<Vec<String>>,
    pub target_set: Option<Vec<String>>,
    pub domain: String,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TopologyArtifact {
    ValidatedTopology {
        points: Vec<String>,
        open_sets: Vec<Vec<String>>,
    },
    Boolean(bool),
    Set(Vec<String>),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TopologyStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidDomain,
    Inconsistent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TopologyResult {
    pub status: TopologyStatus,
    pub artifact: Option<TopologyArtifact>,
    pub source: Option<SourceCitation>,
    pub assumptions: Vec<String>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("topology value serializes"))
    )
}

fn payload(result: &TopologyResult) -> impl Serialize + '_ {
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
    request: &TopologyRequest,
    status: TopologyStatus,
    artifact: Option<TopologyArtifact>,
    source: Option<SourceCitation>,
    assumptions: Vec<String>,
    reasons: Vec<String>,
) -> TopologyResult {
    let mut result = TopologyResult {
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

/// Extract topology-definition records from an attributed source document.
pub fn extract_topology_definitions(
    document: &str,
) -> Result<Vec<TopologyDefinitionRecord>, Vec<String>> {
    let mut errors = Vec::new();
    let mut blocks: Vec<(usize, BTreeMap<String, String>)> = Vec::new();
    let mut current: Option<(usize, BTreeMap<String, String>)> = None;
    for (line_index, raw) in document.lines().enumerate() {
        let line_number = line_index + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "BEGIN TOPOLOGY" {
            if current.is_some() {
                errors.push(format!("nested BEGIN TOPOLOGY at line {line_number}"));
            } else {
                current = Some((line_number, BTreeMap::new()));
            }
            continue;
        }
        if line == "END TOPOLOGY" {
            if let Some(block) = current.take() {
                blocks.push(block);
            } else {
                errors.push(format!("orphan END TOPOLOGY at line {line_number}"));
            }
            continue;
        }
        let Some((_, fields)) = current.as_mut() else {
            errors.push(format!("field outside topology block at line {line_number}"));
            continue;
        };
        let Some((key, value)) = line.split_once(':') else {
            errors.push(format!("malformed topology field at line {line_number}"));
            continue;
        };
        let key = key.trim().to_ascii_uppercase();
        let value = value.trim().to_string();
        if key.is_empty() || value.is_empty() || fields.insert(key.clone(), value).is_some() {
            errors.push(format!("invalid or duplicate field {key} at line {line_number}"));
        }
    }
    if let Some((line, _)) = current {
        errors.push(format!("topology block beginning at line {line} is unterminated"));
    }

    let mut records = Vec::new();
    for (line, fields) in blocks {
        let required = |key: &str| {
            fields
                .get(key)
                .cloned()
                .ok_or_else(|| format!("topology block at line {line} lacks {key}"))
        };
        let record = (|| -> Result<TopologyDefinitionRecord, String> {
            let max_points = required("MAX_POINTS")?
                .parse::<usize>()
                .map_err(|_| "MAX_POINTS is not an integer".to_string())?;
            let source = SourceCitation {
                source_id: required("SOURCE_ID")?,
                title: required("TITLE")?,
                section: required("SECTION")?,
                url: required("URL")?,
                license: required("LICENSE")?,
                retrieved_utc: required("RETRIEVED")?,
                evidence_span: required("EVIDENCE")?,
            };
            Ok(TopologyDefinitionRecord {
                topology_id: required("TOPOLOGY_ID")?,
                aliases: list(&required("ALIASES")?, '|'),
                domain: required("DOMAIN")?,
                max_points,
                axioms: list(&required("AXIOMS")?, ';'),
                source,
            })
        })();
        match record {
            Ok(record) => records.push(record),
            Err(error) => errors.push(format!("line {line}: {error}")),
        }
    }
    if let Err(validation_errors) = validate_topology_definitions(&records) {
        errors.extend(validation_errors);
    }
    if errors.is_empty() {
        Ok(records)
    } else {
        Err(errors)
    }
}

/// Validate source records before they can be used by the shadow executor.
pub fn validate_topology_definitions(
    records: &[TopologyDefinitionRecord],
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let mut ids = BTreeSet::new();
    let mut aliases = BTreeSet::new();
    for record in records {
        if record.topology_id.trim().is_empty() || !ids.insert(record.topology_id.clone()) {
            errors.push(format!("duplicate or empty topology identifier: {}", record.topology_id));
        }
        if record.domain.trim().is_empty() || record.max_points == 0 || record.max_points > MAX_POINTS {
            errors.push(format!("topology {} has an invalid domain or point bound", record.topology_id));
        }
        for alias in &record.aliases {
            if alias.trim().is_empty() || !aliases.insert(alias.clone()) {
                errors.push(format!("duplicate or empty topology alias in {}", record.topology_id));
            }
        }
        for required_axiom in ["empty", "whole", "unions", "finite_intersections"] {
            if !record.axioms.iter().any(|axiom| axiom == required_axiom) {
                errors.push(format!("topology {} lacks {required_axiom} axiom", record.topology_id));
            }
        }
        if record.source.source_id.trim().is_empty()
            || record.source.title.trim().is_empty()
            || record.source.section.trim().is_empty()
            || !record.source.url.starts_with("https://")
            || record.source.retrieved_utc.trim().is_empty()
            || record.source.evidence_span.trim().is_empty()
        {
            errors.push(format!("topology {} has incomplete source citation", record.topology_id));
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

fn canonical_set(values: &[String]) -> Option<Vec<String>> {
    let mut values = values.to_vec();
    values.sort();
    if values.windows(2).any(|pair| pair[0] == pair[1]) {
        return None;
    }
    Some(values)
}

fn subset(left: &[String], right: &[String]) -> bool {
    left.iter().all(|value| right.binary_search(value).is_ok())
}

fn union(left: &[String], right: &[String]) -> Vec<String> {
    let mut result = left.to_vec();
    result.extend(right.iter().cloned());
    result.sort();
    result.dedup();
    result
}

fn intersection(left: &[String], right: &[String]) -> Vec<String> {
    left.iter()
        .filter(|value| right.binary_search(value).is_ok())
        .cloned()
        .collect()
}

fn canonical_open_sets(points: &[String], open_sets: &[Vec<String>]) -> Option<Vec<Vec<String>>> {
    let mut canonical = Vec::new();
    for open in open_sets {
        let open = canonical_set(open)?;
        if !subset(&open, points) {
            return None;
        }
        if !canonical.contains(&open) {
            canonical.push(open);
        }
    }
    canonical.sort();
    Some(canonical)
}

fn validate_finite_topology(points: &[String], open_sets: &[Vec<String>]) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let points = canonical_set(points).ok_or("carrier contains duplicate points")?;
    if points.is_empty() || points.len() > MAX_POINTS {
        return Err("carrier is empty or exceeds the finite bound".into());
    }
    let open_sets = canonical_open_sets(&points, open_sets).ok_or("open set is malformed or outside carrier")?;
    let empty: Vec<String> = Vec::new();
    if !open_sets.contains(&empty) || !open_sets.contains(&points) {
        return Err("topology must contain the empty and whole sets".into());
    }
    for left in &open_sets {
        for right in &open_sets {
            if !open_sets.contains(&union(left, right)) || !open_sets.contains(&intersection(left, right)) {
                return Err("open sets are not closed under unions and finite intersections".into());
            }
        }
    }
    Ok((points, open_sets))
}

fn selected_record<'a>(request: &TopologyRequest, records: &'a [TopologyDefinitionRecord]) -> Result<&'a TopologyDefinitionRecord, TopologyResult> {
    if request.domain.trim().is_empty() {
        return Err(output(request, TopologyStatus::InvalidDomain, None, None, Vec::new(), vec!["topology domain is empty".into()]));
    }
    if let Some(ambiguity) = &request.ambiguity {
        return Err(output(request, TopologyStatus::Ambiguous, None, None, Vec::new(), vec![ambiguity.clone()]));
    }
    let candidates = records
        .iter()
        .filter(|record| record.domain == request.domain && (record.topology_id == request.topology || record.aliases.iter().any(|alias| alias == &request.topology)))
        .collect::<Vec<_>>();
    if candidates.len() != 1 {
        return Err(output(request, if candidates.is_empty() { TopologyStatus::Missing } else { TopologyStatus::Ambiguous }, None, None, Vec::new(), vec!["topology identifier does not select exactly one scoped definition".into()]));
    }
    Ok(candidates[0])
}

/// Execute a bounded finite-topology request using an extracted definition.
pub fn evaluate_topology(request: &TopologyRequest, records: &[TopologyDefinitionRecord]) -> TopologyResult {
    let record = match selected_record(request, records) {
        Ok(record) => record,
        Err(result) => return result,
    };
    if request.points.len() > record.max_points {
        return output(request, TopologyStatus::Unsupported, None, Some(record.source.clone()), record.axioms.clone(), vec!["carrier exceeds source-declared finite bound".into()]);
    }
    let (points, open_sets) = match validate_finite_topology(&request.points, &request.open_sets) {
        Ok(value) => value,
        Err(reason) => return output(request, TopologyStatus::Inconsistent, None, Some(record.source.clone()), record.axioms.clone(), vec![reason]),
    };
    let target = match request.operation {
        TopologyOperation::ValidateTopology => None,
        _ => match request.target_set.as_ref().and_then(|target| canonical_set(target)) {
            Some(target) if subset(&target, &points) => Some(target),
            Some(_) => return output(request, TopologyStatus::Inconsistent, None, Some(record.source.clone()), record.axioms.clone(), vec!["target set is outside carrier".into()]),
            None => return output(request, TopologyStatus::Inconsistent, None, Some(record.source.clone()), record.axioms.clone(), vec!["target set is malformed".into()]),
            },
        };
    let artifact = match request.operation {
        TopologyOperation::ValidateTopology => TopologyArtifact::ValidatedTopology { points, open_sets },
        TopologyOperation::IsOpen => TopologyArtifact::Boolean(open_sets.contains(&target.expect("target checked"))),
        TopologyOperation::IsClosed => {
            let target = target.expect("target checked");
            let complement = points.iter().filter(|point| !target.contains(point)).cloned().collect::<Vec<_>>();
            TopologyArtifact::Boolean(open_sets.contains(&complement))
        }
        TopologyOperation::Interior => {
            let target = target.expect("target checked");
            let interior = open_sets.iter().filter(|open| subset(open, &target)).fold(Vec::new(), |acc, open| union(&acc, open));
            TopologyArtifact::Set(interior)
        }
        TopologyOperation::Closure => {
            let target = target.expect("target checked");
            let closed_supersets = points.iter().cloned().collect::<Vec<_>>();
            let closed_sets = open_sets.iter().map(|open| points.iter().filter(|point| !open.contains(point)).cloned().collect::<Vec<_>>()).collect::<Vec<_>>();
            let closure = closed_sets.iter().filter(|closed| subset(&target, closed)).fold(closed_supersets, |acc, closed| intersection(&acc, closed));
            TopologyArtifact::Set(closure)
        }
    };
    output(request, TopologyStatus::Complete, Some(artifact), Some(record.source.clone()), record.axioms.clone(), Vec::new())
}

impl TopologyResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != TopologyStatus::Complete || self.artifact.is_some())
            && (self.status != TopologyStatus::Complete || self.source.is_some())
    }

    pub fn authorized(&self) -> bool {
        self.status == TopologyStatus::Complete && self.replay_verified()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> TopologyDefinitionRecord {
        TopologyDefinitionRecord {
            topology_id: "finite_topology".into(),
            aliases: vec!["topology".into()],
            domain: "source_derived_finite_topology".into(),
            max_points: 8,
            axioms: vec!["empty".into(), "whole".into(), "unions".into(), "finite_intersections".into()],
            source: SourceCitation {
                source_id: "test".into(), title: "test".into(), section: "1".into(), url: "https://example.test".into(), license: "test".into(), retrieved_utc: "2026-01-01".into(), evidence_span: "definition".into(),
            },
        }
    }

    #[test]
    fn finite_topology_validates_and_replays() {
        let request = TopologyRequest {
            operation: TopologyOperation::Interior,
            topology: "finite_topology".into(),
            points: vec!["a".into(), "b".into()],
            open_sets: vec![vec![], vec!["a".into()], vec!["a".into(), "b".into()]],
            target_set: Some(vec!["a".into(), "b".into()]),
            domain: "source_derived_finite_topology".into(),
            ambiguity: None,
            provenance: vec!["test".into()],
        };
        let result = evaluate_topology(&request, &[record()]);
        assert!(result.authorized());
        assert_eq!(result.artifact, Some(TopologyArtifact::Set(vec!["a".into(), "b".into()])));
    }

    #[test]
    fn malformed_source_is_rejected() {
        assert!(extract_topology_definitions("BEGIN TOPOLOGY\nTOPOLOGY_ID: x\nEND TOPOLOGY").is_err());
    }
}
