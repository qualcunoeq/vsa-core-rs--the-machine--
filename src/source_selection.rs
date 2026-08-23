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

/// An exact, answer-key-blind source request emitted by a typed capability
/// gap.  The request contains only the operation scope required by the gap;
/// it does not name a subject-specific source or evaluator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceGapRequest {
    pub gap_id: String,
    pub required_operation_hints: Vec<String>,
    pub minimum_lineages: usize,
    pub replay_hash: String,
}

impl SourceGapRequest {
    pub fn new(
        gap_id: impl Into<String>,
        required_operation_hints: Vec<String>,
        minimum_lineages: usize,
    ) -> Self {
        let mut request = Self {
            gap_id: gap_id.into(),
            required_operation_hints: normalize(&required_operation_hints),
            minimum_lineages,
            replay_hash: String::new(),
        };
        request.replay_hash = digest(&(
            &request.gap_id,
            &request.required_operation_hints,
            request.minimum_lineages,
        ));
        request
    }
}

/// A source selection receipt produced directly from a typed capability gap.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceGapSelectionReceipt {
    pub gap_id: String,
    pub required_operation_hints: Vec<String>,
    pub minimum_lineages: usize,
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

fn gap_payload(receipt: &SourceGapSelectionReceipt) -> impl Serialize + '_ {
    (
        &receipt.gap_id,
        &receipt.required_operation_hints,
        receipt.minimum_lineages,
        &receipt.selected_source_ids,
        &receipt.candidates,
    )
}

/// Select replay-valid source lineages by exact operation scope from a typed
/// gap request.  Subject labels and lexical overlap are intentionally ignored.
pub fn select_for_gap(
    request: &SourceGapRequest,
    candidates: &[SourceEvidenceEnvelope],
) -> SourceGapSelectionReceipt {
    let required_hints = normalize(&request.required_operation_hints);
    let mut selected_source_ids = BTreeSet::new();
    let mut receipts = Vec::new();
    for candidate in candidates {
        let replay = envelope_replay_verified(candidate);
        let declared_signature = normalize(&candidate.operation_hints);
        let decision = if replay && declared_signature == required_hints {
            selected_source_ids.insert(candidate.source.source_id.clone());
            SourceSelectionDecision::Selected
        } else {
            SourceSelectionDecision::Rejected
        };
        let reason = if !replay {
            "candidate evidence failed replay".into()
        } else if declared_signature != required_hints {
            "declared operation scope differs from the typed capability gap".into()
        } else {
            "exact typed gap scope and source lineage match".into()
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
    let mut output = SourceGapSelectionReceipt {
        gap_id: request.gap_id.clone(),
        required_operation_hints: required_hints,
        minimum_lineages: request.minimum_lineages,
        selected_source_ids: selected_source_ids.into_iter().collect(),
        candidates: receipts,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&(
        &output.gap_id,
        &output.required_operation_hints,
        output.minimum_lineages,
        &output.selected_source_ids,
        &output.candidates,
    ));
    output.replay_hash = replay_hash;
    output
}

pub fn gap_replay_verified(
    request: &SourceGapRequest,
    receipt: &SourceGapSelectionReceipt,
) -> bool {
    request.replay_hash
        == digest(&(
            &request.gap_id,
            &request.required_operation_hints,
            request.minimum_lineages,
        ))
        && receipt.replay_hash == digest(&gap_payload(receipt))
        && receipt.gap_id == request.gap_id
        && receipt.required_operation_hints == normalize(&request.required_operation_hints)
        && receipt.selected_source_ids.len() >= request.minimum_lineages
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

    #[test]
    fn typed_gap_selects_exact_scope_and_rejects_decoy() {
        let observed = vec![
            envelope(
                "health",
                "source:health",
                "bounded exact rational expression",
            ),
            envelope(
                "economics",
                "source:economics",
                "bounded exact rational expression",
            ),
        ];
        let decoy = envelope("complex", "source:complex", "bounded complex arithmetic");
        let request = SourceGapRequest::new(
            "gap::bounded-rational-source-domain",
            vec!["bounded exact rational expression".into()],
            2,
        );
        let receipt = select_for_gap(&request, &[observed[0].clone(), observed[1].clone(), decoy]);
        assert_eq!(receipt.selected_source_ids.len(), 2);
        assert!(gap_replay_verified(&request, &receipt));
        assert_eq!(
            receipt
                .candidates
                .iter()
                .filter(|candidate| candidate.decision == SourceSelectionDecision::Rejected)
                .count(),
            1
        );
    }

    #[test]
    fn typed_gap_preserves_replay_valid_refusal_when_scope_is_absent() {
        let request = SourceGapRequest::new(
            "gap::unavailable",
            vec!["bounded theorem contract".into()],
            2,
        );
        let receipt = select_for_gap(
            &request,
            &[envelope("a", "source:a", "different operation")],
        );
        assert!(receipt.selected_source_ids.is_empty());
        assert!(!gap_replay_verified(&request, &receipt));
        assert_eq!(
            receipt.candidates[0].decision,
            SourceSelectionDecision::Rejected
        );
    }
}
