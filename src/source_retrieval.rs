//! Immutable source-derived claim retrieval.
//!
//! Retrieval returns governed claim artifacts, never live facts. Claims with
//! a shared upstream lineage are preserved as one evidential lineage, while
//! conflicting objects remain non-authorizable.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaimSource {
    pub source_id: String,
    pub title: String,
    pub locator: String,
    pub retrieved_utc: String,
    pub lineage_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceClaim {
    pub claim_id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub domain: String,
    pub scope: String,
    pub validity: String,
    pub assumptions: Vec<String>,
    pub source: ClaimSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaimQuery {
    pub subject: String,
    pub predicate: String,
    pub domain: String,
    pub scope: String,
    pub provenance: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalStatus {
    Supported,
    Ambiguous,
    Conflicting,
    Missing,
    InvalidQuery,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaimRetrievalResult {
    pub status: RetrievalStatus,
    pub query: ClaimQuery,
    pub claims: Vec<SourceClaim>,
    pub distinct_objects: Vec<String>,
    pub independent_sources: Vec<String>,
    /// Source IDs are retained for provenance, while lineages represent
    /// independent upstream evidence.  Several reports may share one
    /// lineage (for example a copied summary and its original textbook).
    pub independent_lineages: Vec<String>,
    pub reasons: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn payload(result: &ClaimRetrievalResult) -> impl Serialize {
    (
        result.status,
        result.query.clone(),
        result.claims.clone(),
        result.distinct_objects.clone(),
        result.independent_sources.clone(),
        result.independent_lineages.clone(),
        result.reasons.clone(),
    )
}

/// Retrieve claims from an immutable source snapshot.
pub fn retrieve_claim(query: &ClaimQuery, corpus: &[SourceClaim]) -> ClaimRetrievalResult {
    let mut result = ClaimRetrievalResult {
        status: RetrievalStatus::Missing,
        query: query.clone(),
        claims: Vec::new(),
        distinct_objects: Vec::new(),
        independent_sources: Vec::new(),
        independent_lineages: Vec::new(),
        reasons: Vec::new(),
        replay_hash: String::new(),
    };
    if query.subject.trim().is_empty()
        || query.predicate.trim().is_empty()
        || query.domain.trim().is_empty()
        || query.scope.trim().is_empty()
    {
        result.status = RetrievalStatus::InvalidQuery;
        result
            .reasons
            .push("subject, predicate, domain, and scope are required".into());
    } else {
        result.claims = corpus
            .iter()
            .filter(|claim| {
                claim.subject == query.subject
                    && claim.predicate == query.predicate
                    && claim.domain == query.domain
                    && claim.scope == query.scope
            })
            .cloned()
            .collect();
        result
            .claims
            .sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
        result.distinct_objects = result
            .claims
            .iter()
            .map(|claim| claim.object.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        result.independent_sources = result
            .claims
            .iter()
            .map(|claim| claim.source.source_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        result.independent_lineages = result
            .claims
            .iter()
            .map(|claim| claim.source.lineage_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        result.status = match result.distinct_objects.len() {
            0 => RetrievalStatus::Missing,
            1 => RetrievalStatus::Supported,
            _ => RetrievalStatus::Conflicting,
        };
        if result.status == RetrievalStatus::Supported {
            result.reasons.push(
                "all matching claims agree on one object; retrieval remains a claim artifact"
                    .into(),
            );
        } else if result.status == RetrievalStatus::Conflicting {
            result.reasons.push("sources disagree on the object".into());
        } else {
            result
                .reasons
                .push("no source claim matches the exact query".into());
        }
    }
    result.replay_hash = digest(&payload(&result));
    result
}

impl ClaimRetrievalResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self))
            && self.claims.iter().all(|claim| {
                !claim.source.source_id.is_empty() && !claim.source.lineage_id.is_empty()
            })
    }

    /// A consumer may use a retrieved claim only when its object is unique and
    /// provenance is complete. This does not mutate any registry or fact store.
    pub fn eligible_for_shadow_use(&self) -> bool {
        self.status == RetrievalStatus::Supported
            && self.distinct_objects.len() == 1
            && !self.claims.is_empty()
            && self.replay_verified()
    }

    /// Returns whether the retrieved claim has the requested number of
    /// independent upstream lineages.  This is intentionally separate from
    /// `eligible_for_shadow_use`: one authoritative source may be enough for
    /// a bounded lookup, while corroboration-sensitive consumers can demand
    /// multiple independent lineages explicitly.
    pub fn has_independent_lineages(&self, minimum: usize) -> bool {
        self.independent_lineages.len() >= minimum
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim(id: &str, source_id: &str, lineage_id: &str) -> SourceClaim {
        SourceClaim {
            claim_id: id.into(),
            subject: "object".into(),
            predicate: "property".into(),
            object: "value".into(),
            domain: "bounded".into(),
            scope: "exact".into(),
            validity: "explicit".into(),
            assumptions: Vec::new(),
            source: ClaimSource {
                source_id: source_id.into(),
                title: source_id.into(),
                locator: format!("https://example.invalid/{source_id}"),
                retrieved_utc: "2026-08-16".into(),
                lineage_id: lineage_id.into(),
            },
        }
    }

    fn query() -> ClaimQuery {
        ClaimQuery {
            subject: "object".into(),
            predicate: "property".into(),
            domain: "bounded".into(),
            scope: "exact".into(),
            provenance: vec!["lineage-test".into()],
        }
    }

    #[test]
    fn copied_reports_do_not_count_as_independent_lineages() {
        let result = retrieve_claim(
            &query(),
            &[
                claim("primary", "textbook", "chapter-1"),
                claim("copy", "summary", "chapter-1"),
                claim("independent", "journal", "chapter-9"),
            ],
        );
        assert_eq!(result.independent_sources.len(), 3);
        assert_eq!(result.independent_lineages, vec!["chapter-1", "chapter-9"]);
        assert!(result.has_independent_lineages(2));
        assert!(result.replay_verified());
        let mut tampered = result.clone();
        tampered.independent_lineages.push("forged".into());
        assert!(!tampered.replay_verified());
    }

    #[test]
    fn one_lineage_is_not_two_sources_for_corroboration_policy() {
        let result = retrieve_claim(
            &query(),
            &[
                claim("primary", "textbook", "chapter-1"),
                claim("copy-a", "summary-a", "chapter-1"),
                claim("copy-b", "summary-b", "chapter-1"),
            ],
        );
        assert_eq!(result.independent_sources.len(), 3);
        assert_eq!(result.independent_lineages, vec!["chapter-1"]);
        assert!(!result.has_independent_lineages(2));
        assert!(result.eligible_for_shadow_use());
    }
}
