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
}
