//! Shadow-only bootstrap proposals for genuinely unrecognized source domains.
//!
//! This is intentionally a proposal layer, not an automatic domain learner.
//! It can recognize repeated, explicit source operation scopes and describe the
//! missing typed artifact/prerequisite.  It cannot turn source prose into an
//! executable pack, infer subject semantics from a title, or mutate a live
//! registry.  The distinction is important for Goal 11: unknown-domain
//! residuals must become falsifiable acquisition plans before they become code.

use crate::prerequisite_discovery::{
    capability_gap_replay_verified, propose_capability_gap, CapabilityGap, CapabilityGapStatus,
};
use crate::source_evidence_envelope::{
    replay_verified as envelope_replay_verified, SourceEvidenceEnvelope,
};
use crate::source_residual_clustering::{
    cluster_replay_verified, cluster_residuals, propose_shadow_contract, ProposalStatus,
    SourceContractProposal, SourceResidualCluster,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapDecision {
    ShadowProposal,
    ResidualPreserved,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnknownDomainBootstrapProposal {
    pub proposal_id: String,
    pub decision: BootstrapDecision,
    pub cluster: SourceResidualCluster,
    pub source_contract: Option<SourceContractProposal>,
    pub capability_gap: Option<CapabilityGap>,
    pub source_sections: Vec<String>,
    pub candidate_artifact_schema: String,
    pub validation_plan: Vec<String>,
    pub falsification_conditions: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("unknown bootstrap serializes"))
    )
}

fn payload(proposal: &UnknownDomainBootstrapProposal) -> impl Serialize + '_ {
    (
        &proposal.proposal_id,
        proposal.decision,
        &proposal.cluster,
        &proposal.source_contract,
        &proposal.capability_gap,
        &proposal.source_sections,
        &proposal.candidate_artifact_schema,
        &proposal.validation_plan,
        &proposal.falsification_conditions,
    )
}

fn schema_from_hints(hints: &[String]) -> String {
    format!(
        "UnknownTypedArtifact{{declared_operations=[{}]; provenance; assumptions; boundaries; unresolved_alternatives}}",
        hints.join(" | ")
    )
}

/// Convert repeated, replay-valid residuals into a shadow bootstrap proposal.
///
/// The function deliberately requires two independent source IDs through the
/// residual clusterer.  A proposal is still not executable: it carries a
/// generic artifact schema and a validation plan, while the domain-specific
/// representation and method remain absent.
pub fn propose_unknown_domain_bootstrap(
    envelopes: &[SourceEvidenceEnvelope],
) -> Vec<UnknownDomainBootstrapProposal> {
    let clusters = cluster_residuals(envelopes);
    clusters
        .into_iter()
        .filter(|cluster| cluster_replay_verified(cluster))
        .filter_map(|cluster| {
            let source_contract = propose_shadow_contract(&cluster)?;
            if source_contract.status != ProposalStatus::ShadowOnly {
                return None;
            }
            let gap = propose_capability_gap(
                "unknown_source_residual",
                CapabilityGapStatus::MissingPrerequisite,
                cluster.residual_hashes.clone(),
            )?;
            if !capability_gap_replay_verified(&gap) {
                return None;
            }
            let source_sections = envelopes
                .iter()
                .filter(|envelope| {
                    envelope_replay_verified(envelope)
                        && cluster.source_ids.contains(&envelope.source.source_id)
                })
                .map(|envelope| envelope.source.section.clone())
                .collect::<Vec<_>>();
            let mut proposal = UnknownDomainBootstrapProposal {
                proposal_id: format!("unknown-domain-shadow::{}", cluster.signature),
                decision: BootstrapDecision::ShadowProposal,
                candidate_artifact_schema: schema_from_hints(&cluster.operation_hints),
                validation_plan: vec![
                    "extract explicit definitions and assumptions from each independent source"
                        .into(),
                    "generate supported, ambiguous, unsupported, and paraphrase cases".into(),
                    "validate typed artifacts and replay before any method synthesis".into(),
                    "pressure-test injected defects in a sandbox clone".into(),
                    "keep promotion disabled until an external policy gate passes".into(),
                ],
                falsification_conditions: vec![
                    "source lineages collapse to one upstream origin".into(),
                    "operation scope or output type differs across sources".into(),
                    "assumptions are implicit or mutually inconsistent".into(),
                    "candidate interpretation requires lexical subject guessing".into(),
                    "typed replay or negative-boundary validation fails".into(),
                ],
                source_contract: Some(source_contract),
                capability_gap: Some(gap),
                cluster,
                source_sections,
                replay_hash: String::new(),
            };
            let replay_hash = {
                let unsigned = payload(&proposal);
                digest(&unsigned)
            };
            proposal.replay_hash = replay_hash;
            Some(proposal)
        })
        .collect()
}

pub fn replay_verified(proposal: &UnknownDomainBootstrapProposal) -> bool {
    proposal.replay_hash == digest(&payload(proposal))
        && proposal.decision == BootstrapDecision::ShadowProposal
        && proposal
            .source_contract
            .as_ref()
            .is_some_and(|contract| contract.status == ProposalStatus::ShadowOnly)
        && proposal
            .capability_gap
            .as_ref()
            .is_some_and(capability_gap_replay_verified)
        && !proposal.candidate_artifact_schema.is_empty()
        && !proposal.validation_plan.is_empty()
        && !proposal.falsification_conditions.is_empty()
}

/// A source can be inspected as metadata without being accepted as an
/// executable bootstrap.  This helper makes that negative boundary explicit.
pub fn executable_bootstrap_allowed(_proposal: &UnknownDomainBootstrapProposal) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_evidence_envelope::ingest_source_evidence;

    fn envelope(path: &str, source_id: &str, scope: &str) -> SourceEvidenceEnvelope {
        let document = format!(
            "SOURCE_ID: {source_id}\nTITLE: Unknown domain source\nSECTION: {scope}\nURL: https://example.invalid/{source_id}\nLICENSE: CC BY\nRETRIEVED_UTC: 2026-08-19\nEVIDENCE_SPAN: explicit definitions and boundaries\nSCOPE: {scope}"
        );
        ingest_source_evidence(path, &document).unwrap()
    }

    #[test]
    fn repeated_unknown_scope_produces_non_executable_proposal() {
        let envelopes = vec![
            envelope(
                "a.txt",
                "source:a",
                "finite simplicial complexes boundary matrices betti numbers",
            ),
            envelope(
                "b.txt",
                "source:b",
                "finite simplicial complexes boundary matrices betti numbers",
            ),
        ];
        let proposals = propose_unknown_domain_bootstrap(&envelopes);
        assert_eq!(proposals.len(), 1);
        assert!(replay_verified(&proposals[0]));
        assert!(!executable_bootstrap_allowed(&proposals[0]));
        assert_eq!(proposals[0].decision, BootstrapDecision::ShadowProposal);
    }

    #[test]
    fn one_source_and_scope_mismatch_are_preserved_as_residuals() {
        let one_source = vec![
            envelope("a.txt", "source:a", "finite operation"),
            envelope("b.txt", "source:a", "finite operation"),
        ];
        assert!(propose_unknown_domain_bootstrap(&one_source).is_empty());
        let mismatch = vec![
            envelope("a.txt", "source:a", "finite operation"),
            envelope("b.txt", "source:b", "different operation"),
        ];
        assert!(propose_unknown_domain_bootstrap(&mismatch).is_empty());
    }

    #[test]
    fn tampered_residual_cannot_bootstrap() {
        let mut envelopes = vec![
            envelope("a.txt", "source:a", "finite operation"),
            envelope("b.txt", "source:b", "finite operation"),
        ];
        envelopes[0].residual_sha256.push('x');
        assert!(propose_unknown_domain_bootstrap(&envelopes).is_empty());
    }
}
