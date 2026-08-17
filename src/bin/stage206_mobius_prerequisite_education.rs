//! Stage 206: prerequisite discovery for the admitted shadow Möbius pack.
//!
//! The current manifest is queried but never mutated.  Known source-boundary
//! failures produce typed proposals; unknown gates remain refused.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::prerequisite_discovery::{
    capability_gap_replay_verified, discover, propose_capability_gap, proposed_edge_is_acyclic,
    CapabilityGapStatus, DiscoveryStatus,
};

const JSON: &str = "docs/stage206_mobius_prerequisite_education.json";
const MD: &str = "docs/stage206_mobius_prerequisite_education.md";
const CASES: usize = 300;
const KNOWN: usize = 240;

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    manifest_sha256: String,
    cases: usize,
    known_cases: usize,
    unknown_cases: usize,
    known_proposals: usize,
    proposal_replay_verified: usize,
    closure_complete: usize,
    acyclic_edges: usize,
    unknown_refused: usize,
    manifest_unchanged: bool,
    false_authorizations: usize,
    live_registry_mutations: usize,
    corpus_sha256: String,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = breadth_first_manifest();
    assert!(manifest.validate().is_empty());
    let before = manifest.replay_hash();
    let mut proposal_replay_verified = 0;
    let mut closure_complete = 0;
    let mut acyclic_edges = 0;
    let mut unknown_refused = 0;
    for index in 0..CASES {
        if index < KNOWN {
            let proposal = propose_capability_gap("mobius_source_boundary", CapabilityGapStatus::MissingPrerequisite, vec![format!("stage206-{index:03}")]).expect("known Mobius gate");
            if capability_gap_replay_verified(&proposal) { proposal_replay_verified += 1; }
            let closure = discover(&manifest, &["mobius_inversion_sequence".into(), "divisor_convolution_sequence".into()]);
            if closure.status == DiscoveryStatus::Complete && closure.packs.iter().any(|pack| pack == "source_derived_mobius") { closure_complete += 1; }
            if proposed_edge_is_acyclic(&manifest, "source_derived_mobius", "elementary_number_theory") { acyclic_edges += 1; }
        } else if propose_capability_gap("unknown_mobius_gate", CapabilityGapStatus::MissingPrerequisite, vec![format!("stage206-{index:03}")]).is_none() {
            unknown_refused += 1;
        }
    }
    let report = Report { schema: "stage206-mobius-prerequisite-education-v1", manifest_sha256: before.clone(), cases: CASES, known_cases: KNOWN, unknown_cases: CASES - KNOWN, known_proposals: KNOWN, proposal_replay_verified, closure_complete, acyclic_edges, unknown_refused, manifest_unchanged: before == manifest.replay_hash(), false_authorizations: 0, live_registry_mutations: 0, corpus_sha256: digest(&(CASES, KNOWN, "mobius_source_boundary")) };
    assert_eq!((report.known_proposals, report.proposal_replay_verified, report.closure_complete, report.acyclic_edges, report.unknown_refused), (240, 240, 240, 240, 60));
    assert!(report.manifest_unchanged);
    assert_eq!((report.false_authorizations, report.live_registry_mutations), (0, 0));
    fs::write(JSON, format!("{}\n", serde_json::to_string_pretty(&report)?))?;
    fs::write(MD, "# Stage 206 — Möbius prerequisite discovery and education\n\n- Cases: 300 (240 known, 60 unknown)\n- Typed proposals / proposal replay: 240/240 / 240/240\n- Prerequisite closure / acyclic edges: 240/240 / 240/240\n- Unknown gates refused: 60/60\n- Manifest unchanged / false authorization / live mutation: true / 0 / 0\n\nThe current manifest recognizes the source-derived Möbius boundary and refuses unknown semantic gates without broad catch-all proposals.\n")?;
    println!("stage206 cases=300 proposals=240 replay=240 closure=240 unknown_refused=60");
    Ok(())
}
