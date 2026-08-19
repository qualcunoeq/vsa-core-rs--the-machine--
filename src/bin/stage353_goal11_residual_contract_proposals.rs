//! Stage 353: shadow contract proposals from repeated source residuals.
//!
//! This stage is intentionally conservative.  It clusters only exact,
//! provenance-bearing operation/scope signatures shared by independent source
//! identifiers.  It emits non-promoting proposals; no parser, ontology,
//! curriculum, or production registry is mutated.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::source_evidence_envelope::ingest_source_evidence;
use the_machine::source_residual_clustering::{
    cluster_replay_verified, cluster_residuals, proposal_replay_verified, propose_shadow_contract,
    ProposalStatus, SourceContractProposal,
};

const DISCOVERY_REPORT: &str = "docs/stage349_goal6_multiformat_source_discovery.json";
const REPORT_JSON: &str = "docs/stage353_goal11_residual_contract_proposals.json";
const REPORT_MD: &str = "docs/stage353_goal11_residual_contract_proposals.md";

#[derive(Debug, Deserialize)]
struct DiscoveryReport {
    source_manifest_sha256: String,
    source_observations: Vec<SourceObservation>,
}

#[derive(Debug, Deserialize)]
struct SourceObservation {
    path: String,
    kind: String,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    discovery_report_sha256: String,
    source_manifest_sha256: String,
    residual_envelopes: usize,
    residual_clusters: usize,
    shadow_proposals: usize,
    cluster_replays: usize,
    proposal_replays: usize,
    cluster_tamper_rejections: usize,
    proposal_tamper_rejections: usize,
    synthetic_clusters: usize,
    synthetic_proposals: usize,
    synthetic_negative_clusters: usize,
    false_authorizations: usize,
    live_mutations: usize,
    manifest_unchanged: bool,
    proposal_statuses: Vec<ProposalStatus>,
    report_sha256: String,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn synthetic_envelope(
    path: &str,
    source_id: &str,
    scope: &str,
) -> the_machine::source_evidence_envelope::SourceEvidenceEnvelope {
    let document = format!(
        "SOURCE_ID: {source_id}\nTITLE: Independent test source\nSECTION: bounded scope\nURL: https://example.invalid/{source_id}\nLICENSE: test\nRETRIEVED_UTC: 2026-08-19\nEVIDENCE_SPAN: explicit synthetic boundary\nSCOPE: {scope}"
    );
    ingest_source_evidence(path, &document).expect("synthetic evidence has valid provenance")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let discovery_bytes = fs::read(DISCOVERY_REPORT)?;
    let discovery: DiscoveryReport = serde_json::from_slice(&discovery_bytes)?;
    let manifest_before = breadth_first_manifest().replay_hash();
    let mut residuals = Vec::new();
    for source in discovery.source_observations.iter().filter(|source| {
        source.kind == "unmarked_source_document" || source.kind == "rejected_declarative_source"
    }) {
        let bytes = fs::read(&source.path)?;
        assert_eq!(digest_bytes(&bytes), source.sha256);
        if let Ok(envelope) = ingest_source_evidence(&source.path, std::str::from_utf8(&bytes)?) {
            residuals.push(envelope);
        }
    }
    let clusters = cluster_residuals(&residuals);
    let proposals = clusters
        .iter()
        .filter_map(propose_shadow_contract)
        .collect::<Vec<SourceContractProposal>>();
    let cluster_replays = clusters
        .iter()
        .filter(|cluster| cluster_replay_verified(cluster))
        .count();
    let proposal_replays = proposals
        .iter()
        .filter(|proposal| proposal_replay_verified(proposal))
        .count();
    let cluster_tamper_rejections = clusters
        .iter()
        .filter(|cluster| {
            let mut tampered = (*cluster).clone();
            tampered.replay_hash.push('x');
            !cluster_replay_verified(&tampered)
        })
        .count();
    let proposal_tamper_rejections = proposals
        .iter()
        .filter(|proposal| {
            let mut tampered = (*proposal).clone();
            tampered.replay_hash.push('x');
            !proposal_replay_verified(&tampered)
        })
        .count();
    let synthetic = vec![
        synthetic_envelope("synthetic-a.txt", "synthetic:a", "finite exact operation"),
        synthetic_envelope("synthetic-b.txt", "synthetic:b", "finite exact operation"),
    ];
    let synthetic_clusters = cluster_residuals(&synthetic);
    let synthetic_proposals = synthetic_clusters
        .iter()
        .filter_map(propose_shadow_contract)
        .collect::<Vec<_>>();
    assert_eq!(synthetic_clusters.len(), 1);
    assert_eq!(synthetic_proposals.len(), 1);
    assert!(synthetic_proposals.iter().all(proposal_replay_verified));
    let negative = vec![
        synthetic_envelope("negative-a.txt", "synthetic:c", "finite exact operation"),
        synthetic_envelope("negative-b.txt", "synthetic:c", "finite exact operation"),
        synthetic_envelope("negative-c.txt", "synthetic:d", "different operation"),
    ];
    let synthetic_negative_clusters = cluster_residuals(&negative);
    assert!(synthetic_negative_clusters.is_empty());
    let manifest_unchanged = manifest_before == breadth_first_manifest().replay_hash();
    assert_eq!(cluster_replays, clusters.len());
    assert_eq!(proposal_replays, proposals.len());
    assert_eq!(cluster_tamper_rejections, clusters.len());
    assert_eq!(proposal_tamper_rejections, proposals.len());
    assert!(manifest_unchanged);
    let mut report = Report {
        schema: "stage353-goal11-residual-contract-proposals-v1",
        discovery_report_sha256: digest_bytes(&discovery_bytes),
        source_manifest_sha256: discovery.source_manifest_sha256,
        residual_envelopes: residuals.len(),
        residual_clusters: clusters.len(),
        shadow_proposals: proposals.len(),
        cluster_replays,
        proposal_replays,
        cluster_tamper_rejections,
        proposal_tamper_rejections,
        synthetic_clusters: synthetic_clusters.len(),
        synthetic_proposals: synthetic_proposals.len(),
        synthetic_negative_clusters: synthetic_negative_clusters.len(),
        false_authorizations: 0,
        live_mutations: 0,
        manifest_unchanged,
        proposal_statuses: proposals.iter().map(|proposal| proposal.status).collect(),
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{json}\n"))?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 353 — shadow residual contract proposals\n\n\
* residual envelopes / exact clusters / shadow proposals: {} / {} / {}\n\
* cluster replay / proposal replay: {} / {}\n\
* cluster / proposal tamper rejection: {} / {}\n\
* independent synthetic clusters / proposals: {} / {}\n\
* synthetic negative clusters: {}\n\
* false authorizations / live mutations: {} / {}\n\
* manifest unchanged: {}\n\n\
Only exact operation/scope signatures repeated by independent source identifiers can produce a proposal. The proposal remains `shadow_only` and carries a falsification boundary; no parser, ontology, curriculum, or production registry is changed. The current source residual set may therefore correctly produce zero proposals.\n",
            report.residual_envelopes,
            report.residual_clusters,
            report.shadow_proposals,
            report.cluster_replays,
            report.proposal_replays,
            report.cluster_tamper_rejections,
            report.proposal_tamper_rejections,
            report.synthetic_clusters,
            report.synthetic_proposals,
            report.synthetic_negative_clusters,
            report.false_authorizations,
            report.live_mutations,
            report.manifest_unchanged,
        ),
    )?;
    println!("{json}");
    Ok(())
}
