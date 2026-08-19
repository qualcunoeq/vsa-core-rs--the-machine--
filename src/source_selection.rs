//! Exact source selection for unknown-domain residuals.
//!
//! Selection is based on declared operation scope and source lineage, never on
//! a subject label or loose lexical overlap.  The result is a proposal receipt
//! only; source selection does not ingest records or mutate the curriculum.

use crate::source_evidence_envelope::{
    replay_verified as envelope_replay_verified, SourceEvidenceEnvelope,
};
use crate::source_residual_clustering::SourceResidualCluster;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceSelectionDecision {
    Selected,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceSelectionCandidate {
    pub path: String,
    pub source_id: String,
    pub declared_signature: Vec<String>,
    pub decision: SourceSelectionDecision,
    pub reason: String,
    pub replay_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceSelectionReceipt {
    pub cluster_signature: String,
    pub required_source_ids: Vec<String>,
    pub required_operation_hints: Vec<String>,
    pub selected_source_ids: Vec<String>,
    pub candidates: Vec<SourceSelectionCandidate>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("source selection serializes"))
    )
}

fn normalize(hints: &[String]) -> Vec<String> {
    let mut normalized = hints
        .iter()
        .map(|hint| {
            hint.split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_ascii_lowercase()
        })
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn payload(receipt: &SourceSelectionReceipt) -> impl Serialize + '_ {
    (
        &receipt.cluster_signature,
        &receipt.required_source_ids,
        &receipt.required_operation_hints,
        &receipt.selected_source_ids,
        &receipt.candidates,
    )
}

/// Select exact source lineages for a residual cluster.
pub fn select_sources(
    cluster: &SourceResidualCluster,
    candidates: &[SourceEvidenceEnvelope],
) -> SourceSelectionReceipt {
    let required_hints = normalize(&cluster.operation_hints);
    let required_ids = cluster.source_ids.iter().cloned().collect::<BTreeSet<_>>();
    let mut selected_source_ids = BTreeSet::new();
    let mut receipts = Vec::new();
    for candidate in candidates {
        let replay = envelope_replay_verified(candidate);
        let declared_signature = normalize(&candidate.operation_hints);
        let decision = if replay
            && declared_signature == required_hints
            && required_ids.contains(&candidate.source.source_id)
        {
            selected_source_ids.insert(candidate.source.source_id.clone());
            SourceSelectionDecision::Selected
        } else {
            SourceSelectionDecision::Rejected
        };
        let reason = if !replay {
            "candidate evidence failed replay".into()
        } else if declared_signature != required_hints {
            "declared operation scope differs from the residual cluster".into()
        } else if !required_ids.contains(&candidate.source.source_id) {
            "source lineage is not one of the independently observed lineages".into()
        } else {
            "exact scope and source lineage match".into()
        };
        receipts.push(SourceSelectionCandidate {
            path: candidate.path.clone(),
            source_id: candidate.source.source_id.clone(),
            declared_signature,
            decision,
            reason,
            replay_verified: replay,
        });
    }
    receipts.sort_by(|left, right| left.path.cmp(&right.path));
    let mut output = SourceSelectionReceipt {
        cluster_signature: cluster.signature.clone(),
        required_source_ids: required_ids.into_iter().collect(),
        required_operation_hints: required_hints,
        selected_source_ids: selected_source_ids.into_iter().collect(),
        candidates: receipts,
        replay_hash: String::new(),
    };
    let replay_hash = {
        let unsigned = payload(&output);
        digest(&unsigned)
    };
    output.replay_hash = replay_hash;
    output
}

pub fn replay_verified(receipt: &SourceSelectionReceipt) -> bool {
    receipt.replay_hash == digest(&payload(receipt))
        && !receipt.cluster_signature.is_empty()
        && !receipt.selected_source_ids.is_empty()
        && receipt
            .candidates
            .iter()
            .filter(|candidate| candidate.decision == SourceSelectionDecision::Selected)
            .all(|candidate| candidate.replay_verified)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_evidence_envelope::ingest_source_evidence;
    use crate::source_residual_clustering::cluster_residuals;

    fn envelope(path: &str, source_id: &str, scope: &str) -> SourceEvidenceEnvelope {
        let document = format!(
            "SOURCE_ID: {source_id}\nTITLE: Source\nSECTION: Scope\nURL: https://example.invalid/{source_id}\nLICENSE: CC BY\nRETRIEVED_UTC: 2026-08-19\nEVIDENCE_SPAN: explicit\nSCOPE: {scope}"
        );
        ingest_source_evidence(path, &document).unwrap()
    }

    #[test]
    fn exact_scope_and_lineage_are_selected() {
        let observed = vec![
            envelope("a", "source:a", "finite operation"),
            envelope("b", "source:b", "finite operation"),
        ];
        let cluster = cluster_residuals(&observed)[0].clone();
        let candidates = vec![
            observed[0].clone(),
            observed[1].clone(),
            envelope("d", "source:d", "finite operation"),
            envelope("e", "source:e", "different operation"),
        ];
        let receipt = select_sources(&cluster, &candidates);
        assert_eq!(receipt.selected_source_ids.len(), 2);
        assert!(replay_verified(&receipt));
        assert_eq!(
            receipt
                .candidates
                .iter()
                .filter(|candidate| candidate.decision == SourceSelectionDecision::Rejected)
                .count(),
            2
        );
    }

    #[test]
    fn tampered_candidate_is_rejected() {
        let observed = vec![
            envelope("a", "source:a", "finite operation"),
            envelope("b", "source:b", "finite operation"),
        ];
        let cluster = cluster_residuals(&observed)[0].clone();
        let mut tampered = observed[0].clone();
        tampered.residual_sha256.push('x');
        let receipt = select_sources(&cluster, &[tampered, observed[1].clone()]);
        assert_eq!(receipt.selected_source_ids, vec!["source:b"]);
        assert!(replay_verified(&receipt));
    }
}
