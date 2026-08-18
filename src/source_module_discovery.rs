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
use std::collections::BTreeSet;

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
}
