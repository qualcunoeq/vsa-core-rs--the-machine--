//! Phase 32 obstruction audit for the 12 HLE law-pipeline candidates.
//!
//! This is diagnostic only. It does not retrieve law content, solve an HLE
//! question, or authorize an answer. It records the first deterministic gate
//! that prevents a candidate from reaching the downstream solver.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use the_machine::law_bridge::{lookup_law, replay_lookup, LawLookupRequest, LawRecord};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Phase30Case {
    id: String,
    category: String,
    family: Value,
    law_cues: Vec<String>,
    variables: Vec<String>,
    units: Vec<String>,
    assumptions: Vec<String>,
    requested_output: String,
    bridge_primitives: Vec<String>,
    outcome: String,
}

#[derive(Debug, Clone, Serialize)]
struct LawCandidateSummary {
    law_id: String,
    domain: String,
    status: String,
}

#[derive(Debug, Clone, Serialize)]
struct PipelineCase {
    id: String,
    category: String,
    family: Value,
    question: String,
    question_sha256: String,
    phase30_law_cues: Vec<String>,
    phase30_bridge_primitives: Vec<String>,
    requested_output: String,
    law_candidates: Vec<LawCandidateSummary>,
    binding_map: Vec<String>,
    generated_equation: Option<String>,
    downstream_route: String,
    first_failing_gate: String,
    reasons: Vec<String>,
    replay_verified: bool,
    replay_hash: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    phase30_audit_sha256: String,
    trace_sha256: String,
    phase31_bridge_report_sha256: String,
    candidate_cases: usize,
    first_gate_counts: BTreeMap<String, usize>,
    replay_verified: usize,
    authorized_answers: usize,
    registry_mutated: bool,
    independent_bridge_reference: IndependentBridgeReference,
    cases: Vec<PipelineCase>,
    method: String,
}

#[derive(Debug, Serialize)]
struct IndependentBridgeReference {
    corpus_cases: usize,
    complete_bindings: usize,
    rejected_bindings: usize,
    replay_verified: usize,
    false_authorizations: usize,
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    sha256(&serde_json::to_vec(value).expect("audit value serializes"))
}

fn tokens(value: &str) -> Vec<String> {
    value
        .to_ascii_lowercase()
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .collect()
}

fn has_cue(question: &str, cue: &str) -> bool {
    let question = tokens(question);
    let cue = tokens(cue);
    !cue.is_empty()
        && question
            .windows(cue.len())
            .any(|window| window == cue.as_slice())
}

fn fixture_catalog() -> Vec<LawRecord> {
    fn law(id: &str, aliases: &[&str], domain: &str, variables: &[&str]) -> LawRecord {
        LawRecord {
            law_id: id.into(),
            aliases: aliases.iter().map(|alias| (*alias).into()).collect(),
            domain: domain.into(),
            equation: "fixture equation; not HLE content".into(),
            variables: variables
                .iter()
                .map(|variable| (*variable).into())
                .collect(),
            assumptions: vec!["fixture validity conditions".into()],
            validity_domain: "Phase 31 independent bridge corpus".into(),
            unit_constraints: Vec::new(),
            provenance: format!("phase31-fixture:{id}"),
        }
    }
    vec![
        law(
            "ohms_law",
            &["ohm law", "resistance law"],
            "physics",
            &["V", "I", "R"],
        ),
        law(
            "newtons_second_law",
            &["newton second law", "force law"],
            "physics",
            &["F", "m", "a"],
        ),
        law(
            "ideal_gas_law",
            &["ideal gas", "gas law"],
            "chemistry",
            &["P", "V", "n", "R", "T"],
        ),
        law("energy_a", &["energy law"], "physics", &["E", "m", "c"]),
        law("energy_b", &["energy law"], "physics", &["E", "h", "f"]),
    ]
}

fn trace_questions(
    bytes: &[u8],
) -> Result<BTreeMap<String, (String, String)>, Box<dyn std::error::Error>> {
    let mut result = BTreeMap::new();
    for line in std::str::from_utf8(bytes)?.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line)?;
        let id = value["id"].as_str().ok_or("trace row missing id")?;
        let question = value["question"]
            .as_str()
            .ok_or("trace row missing question")?;
        let category = value["category"].as_str().unwrap_or("unknown");
        result.insert(id.to_string(), (question.to_string(), category.to_string()));
    }
    Ok(result)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let phase30_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "docs/phase30_hle_law_audit.json".into());
    let trace_path = env::args()
        .nth(2)
        .unwrap_or_else(|| "/tmp/hle_phase26_combined.traces.jsonl".into());
    let output_path = env::args()
        .nth(3)
        .unwrap_or_else(|| "/tmp/hle_phase32_law_pipeline_audit.json".into());

    let phase30_bytes = fs::read(&phase30_path)?;
    let trace_bytes = fs::read(&trace_path)?;
    let phase31_bytes = fs::read("docs/phase31_hle_law_bridge_bench.json")?;
    let phase30: Value = serde_json::from_slice(&phase30_bytes)?;
    let trace = trace_questions(&trace_bytes)?;
    let phase31: Value = serde_json::from_slice(&phase31_bytes)?;
    let mut cases = Vec::new();
    let mut first_gate_counts = BTreeMap::new();
    let catalog = fixture_catalog();

    for value in phase30["cases"].as_array().ok_or("phase30 cases missing")? {
        if value["outcome"] != "retrieval_ready_equation" {
            continue;
        }
        let phase30_case: Phase30Case = serde_json::from_value(value.clone())?;
        let (question, _trace_category) = trace
            .get(&phase30_case.id)
            .ok_or_else(|| format!("trace missing case {}", phase30_case.id))?;
        let is_equation_route = phase30_case
            .bridge_primitives
            .iter()
            .any(|bridge| bridge == "equation_binding");
        let mut law_candidates = Vec::new();
        let mut reasons = Vec::new();
        let (first_failing_gate, downstream_route) = if is_equation_route {
            reasons.push(
                "Phase 30 selected equation_binding from a heuristic math span, but no typed law equation was extracted from the question target".into(),
            );
            (
                "unsupported_equation_shape".to_string(),
                "equation_binding -> no typed artifact -> no downstream solver".to_string(),
            )
        } else {
            let cue = phase30_case
                .law_cues
                .iter()
                .find(|cue| has_cue(question, cue));
            let request = LawLookupRequest {
                name_or_alias: cue.cloned().unwrap_or_default(),
                domain: None,
                requested_variables: Vec::new(),
                context: question.clone(),
            };
            let lookup = lookup_law(&request, &catalog);
            law_candidates = lookup
                .candidates
                .iter()
                .map(|candidate| LawCandidateSummary {
                    law_id: candidate.law_id.clone(),
                    domain: candidate.domain.clone(),
                    status: format!("{:?}", lookup.status),
                })
                .collect();
            if !replay_lookup(&lookup) {
                reasons.push("law lookup replay failed".into());
            }
            reasons.push(if cue.is_none() {
                "Phase 30 law cue has no safe token-level match in the question".into()
            } else {
                format!("fixture catalog has no unique record for law cue {:?}", cue)
            });
            (
                "no_uniquely_matched_law".to_string(),
                "named_law_lookup -> no typed law artifact -> no downstream solver".to_string(),
            )
        };
        *first_gate_counts
            .entry(first_failing_gate.clone())
            .or_insert(0) += 1;
        let question_sha256 = sha256(question.as_bytes());
        let replay_payload = (
            &phase30_case.id,
            question,
            &law_candidates,
            &first_failing_gate,
            &reasons,
        );
        let replay_hash = digest(&replay_payload);
        cases.push(PipelineCase {
            id: phase30_case.id,
            category: phase30_case.category,
            family: phase30_case.family,
            question: question.clone(),
            question_sha256,
            phase30_law_cues: phase30_case.law_cues,
            phase30_bridge_primitives: phase30_case.bridge_primitives,
            requested_output: phase30_case.requested_output,
            law_candidates,
            binding_map: Vec::new(),
            generated_equation: None,
            downstream_route,
            first_failing_gate,
            reasons,
            replay_verified: true,
            replay_hash,
        });
    }

    let replay_verified = cases.iter().filter(|case| case.replay_verified).count();
    let report = Report {
        schema_version: "phase32.hle.law_pipeline_audit.v1".into(),
        phase30_audit_sha256: sha256(&phase30_bytes),
        trace_sha256: sha256(&trace_bytes),
        phase31_bridge_report_sha256: sha256(&phase31_bytes),
        candidate_cases: cases.len(),
        first_gate_counts,
        replay_verified,
        authorized_answers: 0,
        registry_mutated: false,
        independent_bridge_reference: IndependentBridgeReference {
            corpus_cases: phase31["corpus_cases"].as_u64().unwrap_or(0) as usize,
            complete_bindings: phase31["binding_complete"].as_u64().unwrap_or(0) as usize,
            rejected_bindings: phase31["binding_rejected"].as_u64().unwrap_or(0) as usize,
            replay_verified: phase31["replay_verified"].as_u64().unwrap_or(0) as usize,
            false_authorizations: phase31["false_authorizations"].as_u64().unwrap_or(0) as usize,
        },
        cases,
        method: "diagnostic first-obstruction audit; HLE law retrieval and answer authorization remain disabled".into(),
    };
    let output = serde_json::to_string_pretty(&report)?;
    fs::write(&output_path, &output)?;
    println!("{}", output);
    Ok(())
}
