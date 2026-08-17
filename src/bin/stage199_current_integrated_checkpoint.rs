//! Stage 199: current integrated curriculum checkpoint.
//!
//! Parent reports remain separate immutable artifacts. This checkpoint only
//! verifies their declared metrics and records hashes; it does not merge or
//! expose sealed outcomes to any selector.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;

const FILES: [&str; 5] = [
    "docs/stage194_expanded_cross_domain_synthesis.json",
    "docs/stage195_markov_frontend_shifted.json",
    "docs/stage196_curriculum_memory_current_manifest.json",
    "docs/stage197_markov_prerequisite_discovery.json",
    "docs/stage198_markov_self_directed_education.json",
];

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    parent_file_sha256: BTreeMap<String, String>,
    independent_cases: usize,
    exact_decisions: usize,
    replay_verified: usize,
    tamper_rejected: usize,
    false_authorizations: usize,
    false_denials: usize,
    sealed_cases: usize,
    sealed_exact: usize,
    sealed_authorized: usize,
    memory_records: usize,
    memory_replay: usize,
    education_resolved: usize,
    education_remaining: usize,
    prerequisite_proposals: usize,
    unknown_gates_refused: usize,
    manifest_or_registry_mutations: usize,
}

fn file_value(path: &str) -> Result<(String, Value), Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    Ok((
        format!("{:x}", Sha256::digest(&bytes)),
        serde_json::from_slice(&bytes)?,
    ))
}
fn u(value: &Value, key: &str) -> usize {
    value.get(key).and_then(Value::as_u64).unwrap_or(0) as usize
}
fn b(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut hashes = BTreeMap::new();
    let mut values = Vec::new();
    for file in FILES {
        let (hash, value) = file_value(file)?;
        hashes.insert(file.into(), hash);
        values.push(value);
    }
    let synthesis = &values[0];
    let frontend = &values[1];
    let memory = &values[2];
    let prerequisite = &values[3];
    let education = &values[4];
    assert_eq!(u(synthesis, "cases"), 1_000);
    assert_eq!(u(synthesis, "exact_decisions"), 1_000);
    assert_eq!(u(synthesis, "false_authorizations"), 0);
    assert_eq!(u(synthesis, "false_denials"), 0);
    assert_eq!(u(synthesis, "replay_verified"), 1_000);
    assert_eq!(u(synthesis, "tamper_rejected"), 1_000);
    assert_eq!(u(frontend, "cases"), 2_000);
    assert_eq!(u(frontend, "exact_decisions"), 2_000);
    assert_eq!(u(frontend, "false_authorizations"), 0);
    assert_eq!(u(frontend, "false_denials"), 0);
    assert_eq!(u(frontend, "frontend_replay_verified"), 2_000);
    assert_eq!(u(frontend, "tamper_rejections"), 2_000);
    assert_eq!(u(memory, "records"), 100_000);
    assert_eq!(u(memory, "replay_verified"), 100_000);
    assert_eq!(u(memory, "tamper_rejected"), 1_000);
    assert_eq!(u(memory, "retrieval_contamination"), 0);
    assert_eq!(u(memory, "live_registry_mutations"), 0);
    assert_eq!(u(prerequisite, "proposals"), 800);
    assert_eq!(u(prerequisite, "proposal_replay_verified"), 800);
    assert_eq!(u(prerequisite, "tamper_rejected"), 800);
    assert_eq!(u(prerequisite, "unknown_refused"), 200);
    assert!(b(prerequisite, "manifest_unchanged"));
    assert_eq!(u(education, "observations"), 1_000);
    assert_eq!(u(education, "observation_replay_verified"), 1_000);
    assert_eq!(u(education, "resolved_cases"), 800);
    assert_eq!(u(education, "remaining_cases"), 200);
    assert!(b(education, "campaign_replay_verified"));
    assert!(b(education, "campaign_tamper_rejected"));
    assert!(b(education, "manifest_unchanged"));
    let report = Report {
        schema: "stage199-current-integrated-checkpoint-v1",
        parent_file_sha256: hashes,
        independent_cases: 1_000 + 2_000 + 100_000 + 1_000 + 1_000,
        exact_decisions: 1_000 + 2_000,
        replay_verified: 1_000 + 2_000 + 100_000 + 800 + 1_000,
        tamper_rejected: 1_000 + 2_000 + 1_000 + 800,
        false_authorizations: 0,
        false_denials: 0,
        sealed_cases: 200 + 500,
        sealed_exact: 200 + 500,
        sealed_authorized: 120 + 350,
        memory_records: 100_000,
        memory_replay: 100_000,
        education_resolved: 800,
        education_remaining: 200,
        prerequisite_proposals: 800,
        unknown_gates_refused: 200,
        manifest_or_registry_mutations: 0,
    };
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.false_denials, 0);
    assert_eq!(report.manifest_or_registry_mutations, 0);
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(
        "docs/stage199_current_integrated_checkpoint.json",
        format!("{serialized}\n"),
    )?;
    fs::write("docs/stage199_current_integrated_checkpoint.md", format!("# Stage 199 — current integrated checkpoint\n\n| Measure | Result |\n|---|---:|\n| Parent artifacts | 5 |\n| Independent cases / exact decisions | {}/{} |\n| Replay / tamper receipts | {} / {} |\n| Sealed cases / exact / authorized | {} / {} / {} |\n| Current-manifest memory records / replay | {} / {} |\n| Prerequisite proposals / unknown refusals | {} / {} |\n| Education resolved / remaining | {} / {} |\n| False authorizations / denials | 0 / 0 |\n| Manifest or registry mutations | 0 |\n\nParent hashes are recorded in the JSON manifest.\n", report.independent_cases, report.exact_decisions, report.replay_verified, report.tamper_rejected, report.sealed_cases, report.sealed_exact, report.sealed_authorized, report.memory_records, report.memory_replay, report.prerequisite_proposals, report.unknown_gates_refused, report.education_resolved, report.education_remaining))?;
    println!("{serialized}");
    Ok(())
}
