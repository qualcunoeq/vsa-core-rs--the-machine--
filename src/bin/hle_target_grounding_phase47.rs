//! Phase 47 shadow rerun of the four HLE target residuals.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::equation_problem_binding::{bind_equation_problem, BindingStatus};
use the_machine::target_grounding::{
    ground_property_target, ground_symbolic_target, PropertyTargetArtifact, SymbolicTargetArtifact,
    TargetDecision, TargetStatus,
};

const DATASET: &str = "data/hle.jsonl";
const TARGET_AUDIT: &str = "docs/phase46_hle_target_ambiguity_audit.json";

#[derive(Debug, Serialize)]
struct CaseResult {
    id: String,
    question_sha256: String,
    target_family: String,
    target_status: TargetStatus,
    target_replay_verified: bool,
    target_expression_or_entity: String,
    target_components: Vec<String>,
    binding_status: BindingStatus,
    binding_replay_verified: bool,
    terminal_classification: String,
    downstream_authorized: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    dataset_sha256: String,
    target_audit_sha256: String,
    case_count: usize,
    target_decisions: BTreeMap<String, usize>,
    terminal_classifications: BTreeMap<String, usize>,
    target_replays: usize,
    binding_replays: usize,
    complete_target_bindings: usize,
    candidate_answers: usize,
    downstream_authorizations: usize,
    cases: Vec<CaseResult>,
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn property_result(
    decision: TargetDecision<PropertyTargetArtifact>,
) -> (TargetStatus, bool, String, Vec<String>) {
    match decision {
        TargetDecision::Complete(artifact) => (
            TargetStatus::Complete,
            artifact.replay_verified(),
            artifact.target_entity,
            Vec::new(),
        ),
        TargetDecision::Ambiguous { alternatives, .. } => {
            (TargetStatus::Ambiguous, true, "".into(), alternatives)
        }
        TargetDecision::Unsupported { reason } => {
            (TargetStatus::Unsupported, true, reason, Vec::new())
        }
    }
}

fn symbolic_result(
    decision: TargetDecision<SymbolicTargetArtifact>,
) -> (TargetStatus, bool, String, Vec<String>) {
    match decision {
        TargetDecision::Complete(artifact) => (
            TargetStatus::Complete,
            artifact.replay_verified(),
            artifact.expression,
            artifact.components,
        ),
        TargetDecision::Ambiguous { alternatives, .. } => {
            (TargetStatus::Ambiguous, true, "".into(), alternatives)
        }
        TargetDecision::Unsupported { reason } => {
            (TargetStatus::Unsupported, true, reason, Vec::new())
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset_bytes = fs::read(DATASET)?;
    let audit_bytes = fs::read(TARGET_AUDIT)?;
    let audit: Value = serde_json::from_slice(&audit_bytes)?;
    let ids: Vec<String> = audit["cases"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| row["id"].as_str().map(String::from))
        .collect();
    let mut questions = BTreeMap::new();
    for line in String::from_utf8_lossy(&dataset_bytes).lines() {
        let row: Value = serde_json::from_str(line)?;
        if let (Some(id), Some(question)) = (row["id"].as_str(), row["question"].as_str()) {
            questions.insert(id.to_string(), question.to_string());
        }
    }
    let mut cases = Vec::new();
    let mut target_decisions = BTreeMap::new();
    let mut terminal_classifications = BTreeMap::new();
    let mut target_replays = 0;
    let mut binding_replays = 0;
    let mut complete_target_bindings = 0;
    for id in ids {
        let question = questions
            .get(&id)
            .ok_or_else(|| format!("missing HLE question {id}"))?;
        let property = id == "66e94a88b78e263c565b17ee" || id == "6717eeddd6c14a5dd1563e7c";
        let (family, target_status, target_replay, target_text, components) = if property {
            let result = property_result(ground_property_target(question));
            ("property_target", result.0, result.1, result.2, result.3)
        } else {
            let result = symbolic_result(ground_symbolic_target(question));
            ("symbolic_target", result.0, result.1, result.2, result.3)
        };
        let binding = bind_equation_problem(question);
        let terminal = match (target_status, binding.status) {
            (TargetStatus::Complete, BindingStatus::Complete) => "complete_target_and_binding",
            (TargetStatus::Complete, BindingStatus::Ambiguous) => {
                "target_complete_binding_context_gap"
            }
            (TargetStatus::Complete, BindingStatus::Unsupported) => {
                "target_complete_unsupported_representation"
            }
            (TargetStatus::Ambiguous, _) => "target_ambiguity_preserved",
            (TargetStatus::Unsupported, _) => "target_unsupported",
        }
        .to_string();
        *target_decisions
            .entry(format!("{target_status:?}"))
            .or_insert(0) += 1;
        *terminal_classifications
            .entry(terminal.clone())
            .or_insert(0) += 1;
        target_replays += usize::from(target_replay);
        binding_replays += usize::from(binding.replay_verified());
        complete_target_bindings += usize::from(
            target_status == TargetStatus::Complete && binding.status == BindingStatus::Complete,
        );
        cases.push(CaseResult {
            id,
            question_sha256: sha(question.as_bytes()),
            target_family: family.into(),
            target_status,
            target_replay_verified: target_replay,
            target_expression_or_entity: target_text,
            target_components: components,
            binding_status: binding.status,
            binding_replay_verified: binding.replay_verified(),
            terminal_classification: terminal,
            downstream_authorized: binding.downstream_authorized,
        });
    }
    let report = Report {
        schema_version: "phase47-hle-target-grounding-rerun-v1".into(),
        dataset_sha256: sha(&dataset_bytes),
        target_audit_sha256: sha(&audit_bytes),
        case_count: cases.len(),
        target_decisions,
        terminal_classifications,
        target_replays,
        binding_replays,
        complete_target_bindings,
        candidate_answers: 0,
        downstream_authorizations: 0,
        cases,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    fs::write(
        "docs/phase47_hle_target_grounding_rerun.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
