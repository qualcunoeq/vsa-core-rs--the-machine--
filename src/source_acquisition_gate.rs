//! Generic, provenance-bound gate for source-derived shadow acquisition.
//!
//! This gate deliberately knows nothing about a subject domain.  It accepts
//! executable evidence only when the selected source lineage, typed module,
//! supported exercises, and negative boundaries all remain replayable.  The
//! result is a clone-only proposal; it cannot mutate a live registry.

use crate::source_selection::{
    replay_verified as selection_replay_verified, SourceSelectionReceipt,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutableLineageEvidence {
    pub source_id: String,
    pub module_id: String,
    pub source_hash: String,
    pub module_replay_verified: bool,
    pub exact_decisions: usize,
    pub supported_exercises: usize,
    pub execution_replays: usize,
    pub boundary_cases: usize,
    pub boundary_replays: usize,
    pub tamper_rejections: usize,
    pub false_authorizations: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AcquisitionDecision {
    PromotableInClone,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceAcquisitionReceipt {
    pub selected_source_ids: Vec<String>,
    pub executable_source_ids: Vec<String>,
    pub minimum_lineages: usize,
    pub minimum_supported_exercises: usize,
    pub decision: AcquisitionDecision,
    pub reasons: Vec<String>,
    pub live_registry_mutations: usize,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("source acquisition serializes"))
    )
}

fn payload(receipt: &SourceAcquisitionReceipt) -> impl Serialize + '_ {
    (
        &receipt.selected_source_ids,
        &receipt.executable_source_ids,
        receipt.minimum_lineages,
        receipt.minimum_supported_exercises,
        receipt.decision,
        &receipt.reasons,
        receipt.live_registry_mutations,
    )
}

/// Evaluate source-derived executable evidence without promoting it.
///
/// A lineage is eligible only if it was selected by the exact source gate,
/// has a replay-valid module, has replay-valid supported and boundary cases,
/// and reports no false authorization.  Distinct executable source IDs are
/// required so one source cannot corroborate itself.
pub fn evaluate_source_acquisition(
    selection: &SourceSelectionReceipt,
    evidence: &[ExecutableLineageEvidence],
    minimum_lineages: usize,
    minimum_supported_exercises: usize,
) -> SourceAcquisitionReceipt {
    let selected: BTreeSet<String> = selection.selected_source_ids.iter().cloned().collect();
    let mut executable = BTreeSet::new();
    let mut reasons = Vec::new();

    if !selection_replay_verified(selection) {
        reasons.push("source-selection receipt is not replay-valid".into());
    }
    if selected.len() < minimum_lineages {
        reasons.push(format!(
            "selected source lineages {} are below required {}",
            selected.len(),
            minimum_lineages
        ));
    }

    let mut seen = BTreeSet::new();
    for item in evidence {
        if !seen.insert(item.source_id.clone()) {
            reasons.push(format!("duplicate executable lineage: {}", item.source_id));
            continue;
        }
        if !selected.contains(&item.source_id) {
            reasons.push(format!(
                "executable lineage {} was not selected by the source gate",
                item.source_id
            ));
            continue;
        }
        let item_ok = item.module_replay_verified
            && !item.source_hash.is_empty()
            && item.exact_decisions == item.supported_exercises + item.boundary_cases
            && item.execution_replays == item.supported_exercises
            && item.boundary_replays == item.boundary_cases
            && item.tamper_rejections == item.exact_decisions
            && item.false_authorizations == 0
            && item.supported_exercises >= minimum_supported_exercises;
        if item_ok {
            executable.insert(item.source_id.clone());
        } else {
            reasons.push(format!(
                "lineage {} lacks complete replayable execution evidence",
                item.source_id
            ));
        }
    }

    if executable.len() < minimum_lineages {
        reasons.push(format!(
            "executable source lineages {} are below required {}",
            executable.len(),
            minimum_lineages
        ));
    }
    if reasons.is_empty() {
        reasons
            .push("independent executable lineages satisfy the clone-only acquisition gate".into());
    }
    let decision = if reasons.len() == 1
        && executable.len() >= minimum_lineages
        && selected.len() >= minimum_lineages
    {
        AcquisitionDecision::PromotableInClone
    } else {
        AcquisitionDecision::Blocked
    };
    let mut receipt = SourceAcquisitionReceipt {
        selected_source_ids: selected.into_iter().collect(),
        executable_source_ids: executable.into_iter().collect(),
        minimum_lineages,
        minimum_supported_exercises,
        decision,
        reasons,
        live_registry_mutations: 0,
        replay_hash: String::new(),
    };
    let replay_hash = {
        let unsigned = payload(&receipt);
        digest(&unsigned)
    };
    receipt.replay_hash = replay_hash;
    receipt
}

pub fn replay_verified(receipt: &SourceAcquisitionReceipt) -> bool {
    receipt.replay_hash == digest(&payload(receipt))
        && receipt.live_registry_mutations == 0
        && receipt
            .executable_source_ids
            .iter()
            .all(|id| receipt.selected_source_ids.contains(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_evidence_envelope::ingest_source_evidence;
    use crate::source_residual_clustering::cluster_residuals;
    use crate::source_selection::select_sources;

    fn selection() -> SourceSelectionReceipt {
        let make = |path: &str, source_id: &str| {
            ingest_source_evidence(
                path,
                &format!(
                    "SOURCE_ID: {source_id}\nTITLE: Source\nSECTION: ratio\nURL: https://example.invalid/{source_id}\nLICENSE: CC BY\nRETRIEVED_UTC: 2026-08-19\nEVIDENCE_SPAN: explicit\nSCOPE: bounded rational expression"
                ),
            )
            .unwrap()
        };
        let observed = vec![make("a", "source:a"), make("b", "source:b")];
        let cluster = cluster_residuals(&observed)[0].clone();
        select_sources(&cluster, &observed)
    }

    fn evidence(source_id: &str) -> ExecutableLineageEvidence {
        ExecutableLineageEvidence {
            source_id: source_id.into(),
            module_id: format!("module::{source_id}"),
            source_hash: "hash".into(),
            module_replay_verified: true,
            exact_decisions: 3,
            supported_exercises: 2,
            execution_replays: 2,
            boundary_cases: 1,
            boundary_replays: 1,
            tamper_rejections: 3,
            false_authorizations: 0,
        }
    }

    #[test]
    fn two_replayable_lineages_are_promotable_only_in_clone() {
        let receipt = evaluate_source_acquisition(
            &selection(),
            &[evidence("source:a"), evidence("source:b")],
            2,
            2,
        );
        assert_eq!(receipt.decision, AcquisitionDecision::PromotableInClone);
        assert!(replay_verified(&receipt));
        assert_eq!(receipt.live_registry_mutations, 0);
    }

    #[test]
    fn one_lineage_is_blocked() {
        let receipt = evaluate_source_acquisition(&selection(), &[evidence("source:a")], 2, 2);
        assert_eq!(receipt.decision, AcquisitionDecision::Blocked);
        assert!(replay_verified(&receipt));
    }

    #[test]
    fn tampered_evidence_cannot_promote() {
        let mut item = evidence("source:a");
        item.tamper_rejections = 0;
        let receipt =
            evaluate_source_acquisition(&selection(), &[item, evidence("source:b")], 2, 2);
        assert_eq!(receipt.decision, AcquisitionDecision::Blocked);
    }
}
