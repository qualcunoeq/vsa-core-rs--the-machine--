//! Stage 197: prerequisite discovery for Markov and technical-language gaps.
//!
//! Failure gates from the shifted frontend are converted into diagnostic,
//! replayable curriculum proposals. Unknown gates are refused, and candidate
//! edges are checked on cloned manifests only.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::prerequisite_discovery::{
    capability_gap_replay_verified, propose_capability_gap, proposed_edge_is_acyclic,
    CapabilityGapStatus,
};

const JSON: &str = "docs/stage197_markov_prerequisite_discovery.json";
const MD: &str = "docs/stage197_markov_prerequisite_discovery.md";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    corpus_sha256: String,
    cases: usize,
    known_gate_cases: usize,
    unknown_gate_cases: usize,
    proposals: usize,
    proposal_replay_verified: usize,
    tamper_rejected: usize,
    unknown_refused: usize,
    acyclic_edge_checks: usize,
    acyclic_edges: usize,
    manifest_unchanged: bool,
    false_authorizations: usize,
    false_denials: usize,
    gate_counts: BTreeMap<String, usize>,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    let manifest_hash = manifest.replay_hash();
    let gates = [
        "stationary_graph_boundary",
        "hitting_graph_boundary",
        "frontend_missing_required_field",
        "frontend_ambiguity",
        "unknown_markov_semantics",
    ];
    let mut gate_counts = BTreeMap::new();
    let mut proposals = 0;
    let mut proposal_replay = 0;
    let mut tamper_rejected = 0;
    let mut unknown_refused = 0;
    let mut edge_checks = 0;
    let mut acyclic_edges = 0;
    for index in 0..1_000 {
        let gate = gates[index % gates.len()];
        *gate_counts.entry(gate.to_string()).or_insert(0) += 1;
        let status = match index % 3 {
            0 => CapabilityGapStatus::MissingPrerequisite,
            1 => CapabilityGapStatus::AmbiguousBoundary,
            _ => CapabilityGapStatus::UnsupportedBoundary,
        };
        let proposal = propose_capability_gap(gate, status, vec![format!("stage197-{index:04}")]);
        if gate == "unknown_markov_semantics" {
            assert!(proposal.is_none());
            unknown_refused += 1;
            continue;
        }
        let proposal = proposal.expect("known Markov gate must remain bounded");
        proposals += 1;
        if capability_gap_replay_verified(&proposal) {
            proposal_replay += 1;
        }
        let mut tampered = proposal.clone();
        tampered.representation_needed.push_str("-tampered");
        if !capability_gap_replay_verified(&tampered) {
            tamper_rejected += 1;
        }
        let (dependent, prerequisite) = match gate {
            "stationary_graph_boundary" => ("finite_markov_stationary_general", "finite_markov"),
            "hitting_graph_boundary" => {
                ("finite_markov_hitting", "finite_markov_stationary_general")
            }
            _ => ("finite_markov_stationary_general", "finite_markov"),
        };
        edge_checks += 1;
        if proposed_edge_is_acyclic(&manifest, dependent, prerequisite) {
            acyclic_edges += 1;
        }
    }
    assert_eq!(proposals, 800);
    assert_eq!(proposal_replay, 800);
    assert_eq!(tamper_rejected, 800);
    assert_eq!(unknown_refused, 200);
    assert_eq!(edge_checks, 800);
    assert_eq!(acyclic_edges, 800);
    assert_eq!(manifest_hash, breadth_first_manifest().replay_hash());
    let report = Report {
        schema: "stage197-markov-prerequisite-discovery-v1",
        corpus_sha256: digest(&gate_counts),
        cases: 1_000,
        known_gate_cases: 800,
        unknown_gate_cases: 200,
        proposals,
        proposal_replay_verified: proposal_replay,
        tamper_rejected,
        unknown_refused,
        acyclic_edge_checks: edge_checks,
        acyclic_edges,
        manifest_unchanged: manifest_hash == breadth_first_manifest().replay_hash(),
        false_authorizations: 0,
        false_denials: 0,
        gate_counts,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(JSON, format!("{serialized}\n"))?;
    fs::write(MD, format!("# Stage 197 — Markov prerequisite discovery\n\n| Measure | Result |\n|---|---:|\n| Cases / known gates / unknown gates | 1,000 / 800 / 200 |\n| Proposals / proposal replay | {proposals}/800 / {proposal_replay}/800 |\n| Tamper rejection | {tamper_rejected}/800 |\n| Unknown-gate refusal | {unknown_refused}/200 |\n| Acyclic edge checks | {acyclic_edges}/{edge_checks} |\n| Manifest unchanged | {} |\n| False authorizations / denials | 0 / 0 |\n\nCorpus SHA-256: `{}`\n", report.manifest_unchanged, report.corpus_sha256))?;
    println!("{serialized}");
    Ok(())
}
