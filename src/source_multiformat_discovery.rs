//! Generic discovery for the declarative source formats used by the
//! shadow education pipeline.
//!
//! Discovery is intentionally format-aware but subject-agnostic.  A source
//! document must declare exactly one supported block format (`FORMULA`,
//! `RELATION`, or `TOPOLOGY`); the corresponding parser supplies the schema,
//! provenance validation, and bounded record checks.  This module only
//! assembles a tamper-evident catalog candidate.  It does not infer a domain,
//! synthesize a solver, or mutate a live curriculum registry.

use crate::curriculum_campaign::SourceModuleCandidate;
use crate::source_formula_pack::source_relation_pack::{extract_relation_records, RelationRecord};
use crate::source_formula_pack::{extract_formula_records, FormulaRecord};
use crate::source_metric_pack::{extract_metric_definitions, MetricDefinitionRecord};
use crate::source_topology_pack::{extract_topology_definitions, TopologyDefinitionRecord};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceCatalogKind {
    Formula,
    Relation,
    Topology,
    Metric,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SourceCatalogRecords {
    Formula(Vec<FormulaRecord>),
    Relation(Vec<RelationRecord>),
    Topology(Vec<TopologyDefinitionRecord>),
    Metric(Vec<MetricDefinitionRecord>),
}

impl SourceCatalogRecords {
    pub fn kind(&self) -> SourceCatalogKind {
        match self {
            Self::Formula(_) => SourceCatalogKind::Formula,
            Self::Relation(_) => SourceCatalogKind::Relation,
            Self::Topology(_) => SourceCatalogKind::Topology,
            Self::Metric(_) => SourceCatalogKind::Metric,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Formula(records) => records.len(),
            Self::Relation(records) => records.len(),
            Self::Topology(records) => records.len(),
            Self::Metric(records) => records.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn source_ids(&self) -> Vec<String> {
        let mut ids = BTreeSet::new();
        match self {
            Self::Formula(records) => {
                ids.extend(records.iter().map(|record| record.source.source_id.clone()));
            }
            Self::Relation(records) => {
                ids.extend(records.iter().map(|record| record.source.source_id.clone()));
            }
            Self::Topology(records) => {
                ids.extend(records.iter().map(|record| record.source.source_id.clone()));
            }
            Self::Metric(records) => {
                ids.extend(records.iter().map(|record| record.source.source_id.clone()));
            }
        }
        ids.into_iter().collect()
    }

    fn domain_hint(&self) -> String {
        match self {
            // Formula records intentionally have no subject/domain field.  A
            // source identifier is still preserved in the candidate and the
            // formula executor remains scoped to this catalog.
            Self::Formula(_) => "source_declared_formula".into(),
            Self::Relation(records) => records
                .first()
                .map(|record| record.domain.clone())
                .unwrap_or_else(|| "source_declared_relation".into()),
            Self::Topology(records) => records
                .first()
                .map(|record| record.domain.clone())
                .unwrap_or_else(|| "source_declared_topology".into()),
            Self::Metric(records) => records
                .first()
                .map(|record| record.domain.clone())
                .unwrap_or_else(|| "source_declared_metric".into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceCatalogDocument<'a> {
    pub path: &'a str,
    pub document: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredSourceCatalog {
    pub candidate: SourceModuleCandidate,
    pub kind: SourceCatalogKind,
    pub records: SourceCatalogRecords,
    pub source_hash: String,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("source catalog serializes"))
    )
}

fn digest_text(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn payload(catalog: &DiscoveredSourceCatalog) -> impl Serialize + '_ {
    (
        &catalog.candidate,
        catalog.kind,
        &catalog.records,
        &catalog.source_hash,
    )
}

fn marker_count(document: &str, marker: &str) -> usize {
    document
        .lines()
        .filter(|line| {
            let line = line.trim();
            line == marker || line.starts_with(&format!("{marker} "))
        })
        .count()
}

/// Discover one source catalog from an explicitly declared block format.
///
/// Mixed-format documents are rejected rather than heuristically partitioned;
/// this prevents an incidental block from being silently assigned to a
/// different parser or provenance lineage.
pub fn discover_source_catalog(
    document: SourceCatalogDocument<'_>,
) -> Result<DiscoveredSourceCatalog, Vec<String>> {
    if document.path.trim().is_empty() {
        return Err(vec!["source catalog path is empty".into()]);
    }
    let counts = [
        (
            SourceCatalogKind::Formula,
            marker_count(document.document, "BEGIN FORMULA"),
        ),
        (
            SourceCatalogKind::Relation,
            marker_count(document.document, "BEGIN RELATION"),
        ),
        (
            SourceCatalogKind::Topology,
            marker_count(document.document, "BEGIN TOPOLOGY"),
        ),
        (
            SourceCatalogKind::Metric,
            marker_count(document.document, "BEGIN METRIC"),
        ),
    ];
    let present = counts
        .iter()
        .filter(|(_, count)| *count > 0)
        .map(|(kind, _)| *kind)
        .collect::<Vec<_>>();
    if present.is_empty() {
        return Err(vec![
            "source document has no supported declarative block".into()
        ]);
    }
    if present.len() != 1 {
        return Err(vec![
            "source document mixes supported declarative formats".into()
        ]);
    }
    let kind = present[0];
    let records = match kind {
        SourceCatalogKind::Formula => {
            extract_formula_records(document.document).map(SourceCatalogRecords::Formula)
        }
        SourceCatalogKind::Relation => {
            extract_relation_records(document.document).map(SourceCatalogRecords::Relation)
        }
        SourceCatalogKind::Topology => {
            extract_topology_definitions(document.document).map(SourceCatalogRecords::Topology)
        }
        SourceCatalogKind::Metric => {
            extract_metric_definitions(document.document).map(SourceCatalogRecords::Metric)
        }
    }?;
    if records.is_empty() {
        return Err(vec!["source document contains no records".into()]);
    }
    let source_ids = records.source_ids();
    if source_ids.is_empty() || source_ids.iter().any(|id| id.trim().is_empty()) {
        return Err(vec!["every record requires source provenance".into()]);
    }
    let source_hash = digest_text(document.document);
    let kind_name = match kind {
        SourceCatalogKind::Formula => "formula",
        SourceCatalogKind::Relation => "relation",
        SourceCatalogKind::Topology => "topology",
        SourceCatalogKind::Metric => "metric",
    };
    let candidate = SourceModuleCandidate {
        module_id: format!("discovered-catalog::{kind_name}::{source_hash}"),
        title: format!("Discovered {kind_name} source catalog"),
        domain: format!("source::{kind_name}::{}", records.domain_hint()),
        provides: vec![format!("source_catalog::{kind_name}::{source_hash}")],
        prerequisite_artifacts: Vec::new(),
        source_ids,
        independent_exercise_count: records.len() * 40,
    };
    let mut catalog = DiscoveredSourceCatalog {
        candidate,
        kind,
        records,
        source_hash,
        replay_hash: String::new(),
    };
    let replay_hash = {
        let unsigned = (
            &catalog.candidate,
            catalog.kind,
            &catalog.records,
            &catalog.source_hash,
        );
        digest(&unsigned)
    };
    catalog.replay_hash = replay_hash;
    Ok(catalog)
}

pub fn replay_verified(catalog: &DiscoveredSourceCatalog) -> bool {
    catalog.replay_hash == digest(&payload(catalog))
        && !catalog.candidate.source_ids.is_empty()
        && !catalog.records.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RELATION: &str = "BEGIN RELATION\nRELATION_ID: pair\nALIASES: pair\nDOMAIN: test\nPAIRS: a=b\nASSUMPTIONS: explicit\nSOURCE_ID: source:test\nTITLE: Test\nSECTION: 1\nURL: https://example.invalid\nLICENSE: test\nRETRIEVED: 2026-08-19\nEVIDENCE: explicit\nEND RELATION";

    const TOPOLOGY: &str = "BEGIN TOPOLOGY\nTOPOLOGY_ID: finite\nALIASES: topology\nDOMAIN: test\nMAX_POINTS: 4\nAXIOMS: empty;whole;unions;finite_intersections\nSOURCE_ID: source:test-topology\nTITLE: Test\nSECTION: 1\nURL: https://example.invalid\nLICENSE: test\nRETRIEVED: 2026-08-19\nEVIDENCE: explicit\nEND TOPOLOGY";

    #[test]
    fn relation_and_topology_formats_are_discovered_without_subject_branches() {
        for (document, expected) in [
            (RELATION, SourceCatalogKind::Relation),
            (TOPOLOGY, SourceCatalogKind::Topology),
        ] {
            let catalog = discover_source_catalog(SourceCatalogDocument {
                path: "test.txt",
                document,
            })
            .unwrap();
            assert_eq!(catalog.kind, expected);
            assert!(replay_verified(&catalog));
        }
    }

    #[test]
    fn mixed_or_unmarked_documents_are_rejected() {
        assert!(discover_source_catalog(SourceCatalogDocument {
            path: "test.txt",
            document: "plain text",
        })
        .is_err());
        assert!(discover_source_catalog(SourceCatalogDocument {
            path: "test.txt",
            document: &format!("{RELATION}\n{TOPOLOGY}"),
        })
        .is_err());
    }

    #[test]
    fn tampering_with_records_invalidates_replay() {
        let mut catalog = discover_source_catalog(SourceCatalogDocument {
            path: "test.txt",
            document: RELATION,
        })
        .unwrap();
        catalog.source_hash.push('x');
        assert!(!replay_verified(&catalog));
    }
}
