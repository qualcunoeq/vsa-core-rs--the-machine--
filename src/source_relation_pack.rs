//! Domain-agnostic source-derived relation catalog.
//!
//! A relation record is data extracted from an attributed source document.
//! The executor only validates aliases, scopes, and explicitly declared pairs;
//! it contains no biology, chemistry, or other subject-specific branches.

use super::SourceCitation;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelationRecord {
    pub relation_id: String,
    pub aliases: Vec<String>,
    pub domain: String,
    pub pairs: BTreeMap<String, String>,
    pub assumptions: Vec<String>,
    pub source: SourceCitation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelationRequest {
    pub relation: String,
    pub input: String,
    pub domain: String,
    pub ambiguity: Option<String>,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RelationStatus {
    Complete,
    Missing,
    Ambiguous,
    Unsupported,
    InvalidDomain,
    Inconsistent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelationArtifact {
    pub relation_id: String,
    pub input: String,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelationResult {
    pub status: RelationStatus,
    pub artifact: Option<RelationArtifact>,
    pub assumptions: Vec<String>,
    pub source: Option<SourceCitation>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("relation catalog serializes"))
    )
}

fn payload(result: &RelationResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.artifact,
        &result.assumptions,
        &result.source,
        &result.reasons,
        &result.provenance,
    )
}

fn output(
    request: &RelationRequest,
    status: RelationStatus,
    artifact: Option<RelationArtifact>,
    assumptions: Vec<String>,
    source: Option<SourceCitation>,
    reasons: Vec<String>,
) -> RelationResult {
    let mut result = RelationResult {
        status,
        artifact,
        assumptions,
        source,
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
        .filter(|part| !part.is_empty())
        .map(String::from)
        .collect()
}

fn parse_pairs(value: &str) -> Result<BTreeMap<String, String>, String> {
    let mut pairs = BTreeMap::new();
    for item in value.split('|').map(str::trim).filter(|item| !item.is_empty()) {
        let (left, right) = item
            .split_once('=')
            .ok_or_else(|| format!("pair lacks '=': {item}"))?;
        let left = left.trim().to_string();
        let right = right.trim().to_string();
        if left.is_empty() || right.is_empty() || pairs.insert(left.clone(), right).is_some() {
            return Err(format!("duplicate or empty relation input: {left}"));
        }
    }
    if pairs.is_empty() {
        return Err("relation declares no pairs".into());
    }
    Ok(pairs)
}

/// Parse explicit `BEGIN RELATION` blocks from a source transcription.
pub fn extract_relation_records(document: &str) -> Result<Vec<RelationRecord>, Vec<String>> {
    let mut errors = Vec::new();
    let mut blocks: Vec<(usize, BTreeMap<String, String>)> = Vec::new();
    let mut current: Option<(usize, BTreeMap<String, String>)> = None;
    for (line_index, raw) in document.lines().enumerate() {
        let line_number = line_index + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "BEGIN RELATION" {
            if current.is_some() {
                errors.push(format!("nested BEGIN RELATION at line {line_number}"));
            } else {
                current = Some((line_number, BTreeMap::new()));
            }
            continue;
        }
        if line == "END RELATION" {
            if let Some(block) = current.take() {
                blocks.push(block);
            } else {
                errors.push(format!("orphan END RELATION at line {line_number}"));
            }
            continue;
        }
        let Some((_, fields)) = current.as_mut() else {
            errors.push(format!("field outside relation block at line {line_number}"));
            continue;
        };
        let Some((key, value)) = line.split_once(':') else {
            errors.push(format!("malformed relation field at line {line_number}"));
            continue;
        };
        let key = key.trim().to_ascii_uppercase();
        let value = value.trim().to_string();
        if key.is_empty() || value.is_empty() || fields.insert(key.clone(), value).is_some() {
            errors.push(format!("invalid or duplicate field {key} at line {line_number}"));
        }
    }
    if let Some((line, _)) = current {
        errors.push(format!("relation block beginning at line {line} is unterminated"));
    }
    let mut records = Vec::new();
    for (line, fields) in blocks {
        let required = |key: &str| {
            fields
                .get(key)
                .cloned()
                .ok_or_else(|| format!("relation block at line {line} lacks {key}"))
        };
        let record = (|| -> Result<RelationRecord, String> {
            let source = SourceCitation {
                source_id: required("SOURCE_ID")?,
                title: required("TITLE")?,
                section: required("SECTION")?,
                url: required("URL")?,
                license: required("LICENSE")?,
                retrieved_utc: required("RETRIEVED")?,
                evidence_span: required("EVIDENCE")?,
            };
            Ok(RelationRecord {
                relation_id: required("RELATION_ID")?,
                aliases: list(&required("ALIASES")?, '|'),
                domain: required("DOMAIN")?,
                pairs: parse_pairs(&required("PAIRS")?)?,
                assumptions: list(&required("ASSUMPTIONS")?, ';'),
                source,
            })
        })();
        match record {
            Ok(record) => records.push(record),
            Err(error) => errors.push(format!("line {line}: {error}")),
        }
    }
    if let Err(validation_errors) = validate_relation_records(&records) {
        errors.extend(validation_errors);
    }
    if errors.is_empty() {
        Ok(records)
    } else {
        Err(errors)
    }
}

/// Validate relation identity, alias uniqueness, pair shape, and citations.
pub fn validate_relation_records(records: &[RelationRecord]) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let mut ids = BTreeSet::new();
    let mut aliases = BTreeSet::new();
    for record in records {
        if record.relation_id.trim().is_empty() || !ids.insert(record.relation_id.clone()) {
            errors.push(format!("duplicate or empty relation identifier: {}", record.relation_id));
        }
        if record.domain.trim().is_empty() || record.pairs.is_empty() {
            errors.push(format!("relation {} lacks domain or pairs", record.relation_id));
        }
        for alias in &record.aliases {
            if alias.trim().is_empty() || !aliases.insert(alias.clone()) {
                errors.push(format!("duplicate or empty relation alias in {}", record.relation_id));
            }
        }
        if record.source.source_id.trim().is_empty()
            || record.source.title.trim().is_empty()
            || record.source.section.trim().is_empty()
            || !record.source.url.starts_with("https://")
            || record.source.retrieved_utc.trim().is_empty()
            || record.source.evidence_span.trim().is_empty()
        {
            errors.push(format!("relation {} has incomplete source citation", record.relation_id));
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

/// Execute a relation record selected by ID or unique alias.
pub fn evaluate_relation(request: &RelationRequest, records: &[RelationRecord]) -> RelationResult {
    if request.domain.trim().is_empty() {
        return output(request, RelationStatus::InvalidDomain, None, Vec::new(), None, vec!["relation domain is empty".into()]);
    }
    if let Some(ambiguity) = &request.ambiguity {
        return output(request, RelationStatus::Ambiguous, None, Vec::new(), None, vec![ambiguity.clone()]);
    }
    let candidates = records
        .iter()
        .filter(|record| record.domain == request.domain
            && (record.relation_id == request.relation
                || record.aliases.iter().any(|alias| alias == &request.relation)))
        .collect::<Vec<_>>();
    if candidates.len() != 1 {
        return output(
            request,
            if candidates.is_empty() { RelationStatus::Missing } else { RelationStatus::Ambiguous },
            None,
            Vec::new(),
            None,
            vec!["relation identifier does not select exactly one scoped record".into()],
        );
    }
    let record = candidates[0];
    let Some(output_value) = record.pairs.get(&request.input) else {
        return output(request, RelationStatus::Unsupported, None, record.assumptions.clone(), Some(record.source.clone()), vec!["input is outside the declared relation alphabet".into()]);
    };
    output(
        request,
        RelationStatus::Complete,
        Some(RelationArtifact {
            relation_id: record.relation_id.clone(),
            input: request.input.clone(),
            output: output_value.clone(),
        }),
        record.assumptions.clone(),
        Some(record.source.clone()),
        Vec::new(),
    )
}

impl RelationResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && !self.provenance.is_empty()
            && (self.status != RelationStatus::Complete || self.artifact.is_some())
            && (self.status != RelationStatus::Complete || self.source.is_some())
    }

    pub fn authorized(&self) -> bool {
        self.status == RelationStatus::Complete && self.replay_verified()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_source_blocks_execution() {
        let source = "BEGIN RELATION\nRELATION_ID: pair\nEND RELATION";
        assert!(extract_relation_records(source).is_err());
    }
}
