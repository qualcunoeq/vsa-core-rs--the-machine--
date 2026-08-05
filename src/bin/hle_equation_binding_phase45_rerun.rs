//! Phase 45 completion check for the four structurally recoverable HLE cases.
//! This runs binding only and never authorizes a downstream answer.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::equation_problem_binding::{
    bind_equation_problem, BindingStatus, ParenthesizedForm,
};

const DATASET: &str = "data/hle.jsonl";
const AUDIT: &str = "docs/phase45_equation_binding_ambiguity_audit.json";

#[derive(Debug, Serialize)]
struct CaseResult {
    id: String,
    question_sha256: String,
    parenthesis_forms: Vec<ParenthesizedForm>,
    binding_status: BindingStatus,
    unique_binding: bool,
    typed_downstream_artifact: bool,
    existing_method_available: bool,
    candidate_answer: Option<String>,
    terminal_outcome: String,
    requested_candidates: Vec<String>,
    reason: String,
    replay_verified: bool,
    downstream_authorized: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    dataset_sha256: String,
    audit_sha256: String,
    repaired_case_count: usize,
    terminal_counts: BTreeMap<String, usize>,
    unique_bindings: usize,
    typed_artifacts: usize,
    candidate_answers: usize,
    downstream_authorizations: usize,
    specialist_residual_count: usize,
    specialist_residuals_unchanged: bool,
    cases: Vec<CaseResult>,
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset_bytes = fs::read(DATASET)?;
    let audit_bytes = fs::read(AUDIT)?;
    let audit: Value = serde_json::from_slice(&audit_bytes)?;
    let ids: Vec<String> = audit["cases"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|row| row["mechanism"].as_str() == Some("overbroad_parenthesis_function_detection"))
        .filter_map(|row| row["id"].as_str().map(String::from))
        .collect();
    let specialist_residual_count = audit["mechanism_counts"]
        ["domain_dependent_function_or_operator_semantics"]
        .as_u64()
        .unwrap_or(0) as usize;
    let mut questions = BTreeMap::new();
    for line in String::from_utf8_lossy(&dataset_bytes).lines() {
        let row: Value = serde_json::from_str(line)?;
        if let (Some(id), Some(question)) = (row["id"].as_str(), row["question"].as_str()) {
            questions.insert(id.to_string(), question.to_string());
        }
    }
    let mut terminal_counts = BTreeMap::new();
    let mut cases = Vec::new();
    let mut unique_bindings = 0;
    let mut typed_artifacts = 0;
    let candidate_answers = 0;
    let mut authorizations = 0;
    for id in ids {
        let question = questions
            .get(&id)
            .ok_or_else(|| format!("missing HLE question {id}"))?;
        let binding = bind_equation_problem(question);
        let unique = binding.status == BindingStatus::Complete;
        let typed = unique && binding.replay_verified();
        let terminal = match binding.status {
            BindingStatus::Complete => "binding_complete_method_or_prerequisite_gap",
            BindingStatus::Ambiguous => "target_or_context_ambiguity",
            BindingStatus::Unsupported => "unsupported_representation",
        }
        .to_string();
        *terminal_counts.entry(terminal.clone()).or_insert(0) += 1;
        unique_bindings += usize::from(unique);
        typed_artifacts += usize::from(typed);
        authorizations += usize::from(binding.downstream_authorized);
        cases.push(CaseResult {
            id,
            question_sha256: sha(question.as_bytes()),
            parenthesis_forms: binding
                .parenthesized_candidates
                .iter()
                .map(|candidate| candidate.form)
                .collect(),
            binding_status: binding.status,
            unique_binding: unique,
            typed_downstream_artifact: typed,
            existing_method_available: false,
            candidate_answer: None,
            terminal_outcome: terminal,
            requested_candidates: binding.requested_unknown.candidates.clone(),
            reason: binding.reason.clone(),
            replay_verified: binding.replay_verified(),
            downstream_authorized: binding.downstream_authorized,
        });
    }
    let report = Report {
        schema_version: "phase45-hle-parenthesis-rerun-v1".into(),
        dataset_sha256: sha(&dataset_bytes),
        audit_sha256: sha(&audit_bytes),
        repaired_case_count: cases.len(),
        terminal_counts,
        unique_bindings,
        typed_artifacts,
        candidate_answers,
        downstream_authorizations: authorizations,
        specialist_residual_count,
        specialist_residuals_unchanged: specialist_residual_count == 6,
        cases,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    fs::write(
        "docs/phase45_hle_parenthesis_rerun.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
