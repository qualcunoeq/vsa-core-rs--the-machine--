//! Phase 44 diagnostic rerun of the frozen 11 HLE equation-binding cases.
//! Binding is evaluated in shadow mode only; no answer is authorized.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use the_machine::equation_problem_binding::{bind_equation_problem, BindingStatus};

const DATASET: &str = "data/hle.jsonl";
const LAW_REPORT: &str = "docs/phase30_hle_law_audit.json";

#[derive(Debug, Serialize)]
struct CaseResult {
    id: String,
    question_sha256: String,
    binding_status: BindingStatus,
    terminal_classification: String,
    target_candidates: Vec<String>,
    symbols: Vec<String>,
    constraints: usize,
    first_failing_gate: String,
    reason: String,
    replay_verified: bool,
    downstream_authorized: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    dataset_sha256: String,
    law_report_sha256: String,
    frozen_case_count: usize,
    classification_counts: BTreeMap<String, usize>,
    binding_replay_verified: usize,
    downstream_authorizations: usize,
    cases: Vec<CaseResult>,
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset_bytes = fs::read(DATASET)?;
    let law_bytes = fs::read(LAW_REPORT)?;
    let law: Value = serde_json::from_slice(&law_bytes)?;
    let ids: BTreeSet<String> = law["cases"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|row| {
            row["outcome"].as_str() == Some("in_question_equation")
                && row["requested_output"].as_str() == Some("scalar_or_structured_value")
                && row["bridge_primitives"].as_array().is_some_and(|items| {
                    items.iter().any(|v| v.as_str() == Some("equation_binding"))
                })
        })
        .filter_map(|row| row["id"].as_str().map(String::from))
        .collect();
    let mut counts = BTreeMap::new();
    let mut cases = Vec::new();
    let mut replay = 0;
    let mut authorizations = 0;
    for line in String::from_utf8(dataset_bytes.clone())?
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        let row: Value = serde_json::from_str(line)?;
        let id = row["id"].as_str().unwrap_or("");
        if !ids.contains(id) {
            continue;
        }
        let question = row["question"].as_str().unwrap_or("");
        let binding = bind_equation_problem(question);
        let classification = match binding.status {
            BindingStatus::Complete => {
                let lower = question.to_ascii_lowercase();
                if lower.contains("least squares") || lower.contains("regression") {
                    "binding_complete_existing_method"
                } else if lower.contains("law") || lower.contains("equation") {
                    "binding_complete_missing_factual_prerequisite"
                } else {
                    "binding_complete_missing_specialist_method"
                }
            }
            BindingStatus::Ambiguous => "ambiguous_binding",
            BindingStatus::Unsupported => "unsupported_representation",
        }
        .to_string();
        *counts.entry(classification.clone()).or_insert(0) += 1;
        replay += usize::from(binding.replay_verified());
        authorizations += usize::from(binding.downstream_authorized);
        let first_gate = match binding.status {
            BindingStatus::Complete => "downstream_method_or_prerequisite",
            BindingStatus::Ambiguous => "binding_ambiguity",
            BindingStatus::Unsupported => "representation_boundary",
        };
        cases.push(CaseResult {
            id: id.into(),
            question_sha256: sha(question.as_bytes()),
            binding_status: binding.status,
            terminal_classification: classification,
            target_candidates: binding.requested_unknown.candidates.clone(),
            symbols: binding
                .symbols
                .iter()
                .map(|symbol| symbol.symbol.clone())
                .collect(),
            constraints: binding.constraints.len(),
            first_failing_gate: first_gate.into(),
            reason: binding.reason.clone(),
            replay_verified: binding.replay_verified(),
            downstream_authorized: binding.downstream_authorized,
        });
    }
    cases.sort_by(|a, b| a.id.cmp(&b.id));
    let report = Report {
        schema_version: "phase44-hle-equation-problem-binding-shadow".into(),
        dataset_sha256: sha(&dataset_bytes),
        law_report_sha256: sha(&law_bytes),
        frozen_case_count: cases.len(),
        classification_counts: counts,
        binding_replay_verified: replay,
        downstream_authorizations: authorizations,
        cases,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    fs::write(
        "docs/phase44_hle_equation_problem_binding_shadow.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
