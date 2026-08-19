//! Shadow clustering of repeated source-evidence residuals.
//!
//! A residual is eligible for a candidate contract only when at least two
//! independent source identifiers repeat the same *explicit* operation/scope
//! signature.  Clustering is exact and provenance-preserving: lexical or
//! subject similarity is not enough, and proposals remain non-promoting.

use crate::source_evidence_envelope::{replay_verified, SourceEvidenceEnvelope};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceResidualCluster {
    pub signature: String,
    pub operation_hints: Vec<String>,
    pub source_ids: Vec<String>,
    pub paths: Vec<String>,
    pub residual_hashes: Vec<String>,
    pub replay_hash: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    ShadowOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceContractProposal {
    pub proposal_id: String,
    pub status: ProposalStatus,
    pub operation_hints: Vec<String>,
    pub source_ids: Vec<String>,
    pub residual_cluster: String,
    pub falsification_boundary: String,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("residual cluster serializes"))
    )
}

fn normalize_hints(envelope: &SourceEvidenceEnvelope) -> Vec<String> {
    let mut hints = envelope
        .operation_hints
        .iter()
        .map(|hint| {
            hint.split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_ascii_lowercase()
        })
        .collect::<Vec<_>>();
    hints.sort();
    hints.dedup();
    hints
}

fn cluster_payload(cluster: &SourceResidualCluster) -> impl Serialize + '_ {
    (
        &cluster.signature,
        &cluster.operation_hints,
        &cluster.source_ids,
        &cluster.paths,
        &cluster.residual_hashes,
    )
}

fn proposal_payload(proposal: &SourceContractProposal) -> impl Serialize + '_ {
    (
        &proposal.proposal_id,
        proposal.status,
        &proposal.operation_hints,
        &proposal.source_ids,
        &proposal.residual_cluster,
        &proposal.falsification_boundary,
    )
}

/// Cluster only replay-valid, non-executable evidence envelopes.
pub fn cluster_residuals(envelopes: &[SourceEvidenceEnvelope]) -> Vec<SourceResidualCluster> {
    let mut grouped: BTreeMap<Vec<String>, Vec<&SourceEvidenceEnvelope>> = BTreeMap::new();
    for envelope in envelopes {
        if !replay_verified(envelope) || envelope.executable {
            continue;
        }
        grouped
            .entry(normalize_hints(envelope))
            .or_default()
            .push(envelope);
    }
    grouped
        .into_iter()
        .filter_map(|(operation_hints, members)| {
            let source_ids = members
                .iter()
                .map(|envelope| envelope.source.source_id.clone())
                .collect::<BTreeSet<_>>();
            if source_ids.len() < 2 || operation_hints.is_empty() {
                return None;
            }
            let paths = members
                .iter()
                .map(|envelope| envelope.path.clone())
                .collect::<BTreeSet<_>>();
            let residual_hashes = members
                .iter()
                .map(|envelope| envelope.residual_sha256.clone())
                .collect::<BTreeSet<_>>();
            let signature = digest(&(&operation_hints, &source_ids));
            let mut cluster = SourceResidualCluster {
                signature,
                operation_hints,
                source_ids: source_ids.into_iter().collect(),
                paths: paths.into_iter().collect(),
                residual_hashes: residual_hashes.into_iter().collect(),
                replay_hash: String::new(),
            };
            let replay_hash = {
                let unsigned = cluster_payload(&cluster);
                digest(&unsigned)
            };
            cluster.replay_hash = replay_hash;
            Some(cluster)
        })
        .collect()
}

pub fn cluster_replay_verified(cluster: &SourceResidualCluster) -> bool {
    cluster.replay_hash == digest(&cluster_payload(cluster))
        && cluster.source_ids.len() >= 2
        && !cluster.operation_hints.is_empty()
}

/// Propose a bounded contract from a repeated cluster without promoting it.
pub fn propose_shadow_contract(cluster: &SourceResidualCluster) -> Option<SourceContractProposal> {
    if !cluster_replay_verified(cluster) {
        return None;
    }
    let mut proposal = SourceContractProposal {
        proposal_id: format!("shadow-source-contract::{}", cluster.signature),
        status: ProposalStatus::ShadowOnly,
        operation_hints: cluster.operation_hints.clone(),
        source_ids: cluster.source_ids.clone(),
        residual_cluster: cluster.signature.clone(),
        falsification_boundary:
            "reject when scope, provenance, operation signature, or typed output remains unresolved"
                .into(),
        replay_hash: String::new(),
    };
    let replay_hash = {
        let unsigned = proposal_payload(&proposal);
        digest(&unsigned)
    };
    proposal.replay_hash = replay_hash;
    Some(proposal)
}

pub fn proposal_replay_verified(proposal: &SourceContractProposal) -> bool {
    proposal.replay_hash == digest(&proposal_payload(proposal))
        && proposal.status == ProposalStatus::ShadowOnly
        && !proposal.source_ids.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_evidence_envelope::ingest_source_evidence;

    fn envelope(path: &str, source_id: &str, scope: &str) -> SourceEvidenceEnvelope {
        let document = format!(
            "SOURCE_ID: {source_id}\nTITLE: Test\nSECTION: 1\nURL: https://example.invalid/{source_id}\nLICENSE: test\nRETRIEVED_UTC: 2026-08-19\nEVIDENCE_SPAN: explicit\nSCOPE: {scope}"
        );
        ingest_source_evidence(path, &document).unwrap()
    }

    #[test]
    fn independent_repeated_scope_produces_shadow_proposal() {
        let clusters = cluster_residuals(&[
            envelope("a.txt", "source:a", "finite exact operation"),
            envelope("b.txt", "source:b", "finite exact operation"),
        ]);
        assert_eq!(clusters.len(), 1);
        assert!(cluster_replay_verified(&clusters[0]));
        let proposal = propose_shadow_contract(&clusters[0]).unwrap();
        assert_eq!(proposal.status, ProposalStatus::ShadowOnly);
        assert!(proposal_replay_verified(&proposal));
    }

    #[test]
    fn one_source_or_different_scope_does_not_cluster() {
        let clusters = cluster_residuals(&[
            envelope("a.txt", "source:a", "finite exact operation"),
            envelope("b.txt", "source:a", "finite exact operation"),
            envelope("c.txt", "source:c", "different operation"),
        ]);
        assert!(clusters.is_empty());
    }

    #[test]
    fn tampered_cluster_cannot_produce_proposal() {
        let mut clusters = cluster_residuals(&[
            envelope("a.txt", "source:a", "finite exact operation"),
            envelope("b.txt", "source:b", "finite exact operation"),
        ]);
        clusters[0].source_ids.pop();
        assert!(!cluster_replay_verified(&clusters[0]));
        assert!(propose_shadow_contract(&clusters[0]).is_none());
    }
}
