//! Generic discovery of bounded source-formula modules.
//!
//! Discovery is structural: it extracts only explicit, provenance-bearing
//! formula records from a bounded document and derives a typed catalog
//! candidate. It does not infer subject semantics, fill assumptions, or
//! mutate curriculum memory.

use crate::curriculum_campaign::SourceModuleCandidate;
use crate::source_formula_pack::{extract_formula_records, FormulaRecord};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceDocument<'a> {
    pub domain: &'a str,
    pub version: &'a str,
    pub source_hint: &'a str,
    pub document: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredSourceModule {
    pub candidate: SourceModuleCandidate,
    pub records: Vec<FormulaRecord>,
    pub source_hash: String,
    pub replay_hash: String,
}

/// Discover source modules from a corpus without a predeclared subject list.
///
/// Module boundaries are derived only from the `SOURCE_ID` provenance already
/// present in each extracted record.  A source document with multiple source
/// sections therefore yields one candidate per source lineage; no evaluator,
/// domain label, or curriculum-memory mutation is inferred here.
pub fn discover_formula_corpus(
    documents: &[&str],
    source_hint: &str,
) -> Result<Vec<DiscoveredSourceModule>, Vec<String>> {
    let mut grouped: BTreeMap<String, Vec<FormulaRecord>> = BTreeMap::new();
    let mut errors = Vec::new();
    for (index, document) in documents.iter().enumerate() {
        match extract_formula_records(document) {
            Ok(records) => {
                for record in records {
                    if record.source.source_id.trim().is_empty() {
                        if source_hint.trim().is_empty() {
                            errors.push(format!("document {index} has a record without SOURCE_ID"));
                        } else {
                            errors.push(format!(
                                "document {index} has provenance-free record; source hint cannot replace SOURCE_ID"
                            ));
                        }
                    } else {
                        grouped
                            .entry(record.source.source_id.clone())
                            .or_default()
                            .push(record);
                    }
                }
            }
            Err(document_errors) => errors.extend(
                document_errors
                    .into_iter()
                    .map(|error| format!("document {index}: {error}")),
            ),
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    if grouped.is_empty() {
        return Err(vec![
            "source corpus contains no provenance-bearing records".into()
        ]);
    }
    grouped
        .into_iter()
        .map(|(source_id, records)| {
            let mut formula_ids = BTreeSet::new();
            if records
                .iter()
                .any(|record| !formula_ids.insert(record.formula_id.clone()))
            {
                return Err(vec![format!(
                    "source {source_id} contains duplicate formula_id"
                )]);
            }
            let first = records.first().expect("nonempty provenance group");
            let candidate = SourceModuleCandidate {
                module_id: format!("discovered-source::{source_id}"),
                title: format!("Discovered source catalog: {}", first.source.title),
                domain: format!("source::{source_id}"),
                provides: vec![format!("source_catalog::{source_id}")],
                prerequisite_artifacts: Vec::new(),
                source_ids: vec![source_id.clone()],
                independent_exercise_count: records.len() * 40,
            };
            let source_hash = digest(&(source_id, &records));
            let mut module = DiscoveredSourceModule {
                candidate,
                records,
                source_hash,
                replay_hash: String::new(),
            };
            let replay_hash = digest(&payload(&module));
            module.replay_hash = replay_hash;
            Ok(module)
        })
        .collect()
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn payload(module: &DiscoveredSourceModule) -> impl Serialize + '_ {
    (&module.candidate, &module.records, &module.source_hash)
}

/// Discover one formula catalog candidate from an explicit source document.
pub fn discover_formula_module(
    document: SourceDocument<'_>,
) -> Result<DiscoveredSourceModule, Vec<String>> {
    if document.domain.trim().is_empty() || document.version.trim().is_empty() {
        return Err(vec!["source module domain and version are required".into()]);
    }
    let records = extract_formula_records(document.document)?;
    if records.is_empty() {
        return Err(vec!["source document contains no formula records".into()]);
    }
    let source_ids = records
        .iter()
        .map(|record| record.source.source_id.clone())
        .collect::<BTreeSet<_>>();
    let source_ids = if source_ids.is_empty() {
        if document.source_hint.trim().is_empty() {
            return Err(vec!["source provenance is absent".into()]);
        }
        vec![document.source_hint.to_owned()]
    } else {
        source_ids.into_iter().collect()
    };
    let candidate = SourceModuleCandidate {
        module_id: format!(
            "discovered-source::{}::{}",
            document.domain, document.version
        ),
        title: format!(
            "Discovered source catalog {} {}",
            document.domain, document.version
        ),
        domain: document.domain.into(),
        provides: vec![format!(
            "source_catalog::{}::{}",
            document.domain, document.version
        )],
        prerequisite_artifacts: Vec::new(),
        source_ids,
        independent_exercise_count: records.len() * 40,
    };
    let source_hash = digest(&document.document);
    let mut module = DiscoveredSourceModule {
        candidate,
        records,
        source_hash,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&module));
    module.replay_hash = replay_hash;
    Ok(module)
}

pub fn replay_verified(module: &DiscoveredSourceModule) -> bool {
    module.replay_hash == digest(&payload(module))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "BEGIN FORMULA ratio\nALIASES: quotient\nEXPRESSION: a / b\nINPUTS: a, b\nASSUMPTIONS: b positive\nCONSTRAINTS: positive:b\nSOURCE_ID: source:test\nTITLE: Test\nSECTION: Ratios\nURL: https://example.invalid/test\nLICENSE: test\nRETRIEVED: 2026-08-18\nEVIDENCE: explicit ratio\nEND FORMULA";

    #[test]
    fn discovery_derives_exact_catalog_candidate_and_replay() {
        let module = discover_formula_module(SourceDocument {
            domain: "test_domain",
            version: "v1",
            source_hint: "source:hint",
            document: SOURCE,
        })
        .unwrap();
        assert_eq!(module.records.len(), 1);
        assert_eq!(
            module.candidate.provides[0],
            "source_catalog::test_domain::v1"
        );
        assert_eq!(module.candidate.source_ids, vec!["source:test"]);
        assert!(replay_verified(&module));
        let mut tampered = module.clone();
        tampered.source_hash.push('x');
        assert!(!replay_verified(&tampered));
    }

    #[test]
    fn malformed_source_is_rejected_before_candidate_creation() {
        let malformed = SOURCE.replace("EXPRESSION:", "EXPRESSION: @");
        let result = discover_formula_module(SourceDocument {
            domain: "test_domain",
            version: "v1",
            source_hint: "source:hint",
            document: &malformed,
        });
        assert!(result.is_err());
    }

    #[test]
    fn corpus_discovery_groups_records_by_provenance_without_domain_labels() {
        let second = SOURCE
            .replace("ratio", "difference")
            .replace("a / b", "a - b");
        let modules = discover_formula_corpus(&[SOURCE, &second], "unused-hint").unwrap();
        assert_eq!(modules.len(), 1);
        assert_eq!(modules[0].records.len(), 2);
        assert_eq!(
            modules[0].candidate.provides,
            vec!["source_catalog::source:test"]
        );
        assert!(replay_verified(&modules[0]));
    }

    #[test]
    fn corpus_discovery_rejects_malformed_input_before_any_module() {
        let malformed = SOURCE.replace("EXPRESSION:", "EXPRESSION: @");
        let result = discover_formula_corpus(&[SOURCE, &malformed], "unused-hint");
        assert!(result.is_err());
    }
}
