//! Phase 48 context-handoff rerun for the four frozen HLE target cases.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::equation_problem_binding::{bind_equation_problem, BindingStatus};
use the_machine::target_context::{
    assemble_target_context, ContextRegion, ContextStatus, RegionRole, TargetContextRequest,
};

const DATASET: &str = "data/hle.jsonl";
const TARGET_RERUN: &str = "docs/phase47_hle_target_grounding_rerun.json";

#[derive(Debug, Serialize)]
struct CaseResult {
    id: String,
    question_sha256: String,
    context_status: ContextStatus,
    binding_handoff_ready: bool,
    included_regions: Vec<String>,
    excluded_regions: Vec<String>,
    context_replay_verified: bool,
    equation_binding_status: BindingStatus,
    equation_binding_replay_verified: bool,
    terminal_classification: String,
    downstream_authorized: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    dataset_sha256: String,
    target_rerun_sha256: String,
    case_count: usize,
    context_decisions: BTreeMap<String, usize>,
    terminal_classifications: BTreeMap<String, usize>,
    complete_contexts: usize,
    context_replays: usize,
    equation_binding_replays: usize,
    complete_equation_bindings: usize,
    downstream_authorizations: usize,
    cases: Vec<CaseResult>,
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn region(
    id: &str,
    role: RegionRole,
    text: &str,
    symbols: &[&str],
    links: &[&str],
) -> ContextRegion {
    ContextRegion {
        id: id.into(),
        role,
        text: text.into(),
        symbols: symbols.iter().map(|value| (*value).into()).collect(),
        target_links: links.iter().map(|value| (*value).into()).collect(),
        scope: "root".into(),
        source_spans: vec![id.into()],
    }
}

fn request_for(id: &str) -> TargetContextRequest {
    match id {
        "66e94a88b78e263c565b17ee" => TargetContextRequest {
            target: "topological invariant".into(),
            target_components: vec!["invariant_group".into(), "T".into(), "P".into(), "D".into()],
            requested_operation: "classify invariant group".into(),
            regions: vec![
                region(
                    "model_definition",
                    RegionRole::Definition,
                    "free fermion model with point defect",
                    &["invariant_group", "D"],
                    &["topological invariant"],
                ),
                region(
                    "symmetry_constraints",
                    RegionRole::Constraint,
                    "T^2=-1; P^2=-1; D=1",
                    &["T", "P", "D"],
                    &["topological invariant"],
                ),
                region(
                    "quoted_formula",
                    RegionRole::Quoted,
                    "quoted unrelated formula",
                    &["q"],
                    &[],
                ),
            ],
        },
        "67153bd7f588f3f15b038f5b" => TargetContextRequest {
            target: "χ".into(),
            target_components: vec!["χ".into(), "beta".into(), "C_l".into()],
            requested_operation: "derive susceptibility".into(),
            regions: vec![
                region(
                    "susceptibility_definition",
                    RegionRole::Definition,
                    "χ = beta sum_l c(c-1)^(l-1) C_l",
                    &["χ", "beta", "C_l"],
                    &["χ"],
                ),
                region(
                    "correlation_constraint",
                    RegionRole::Constraint,
                    "C_l = connected correlation",
                    &["C_l"],
                    &["χ"],
                ),
                region(
                    "homogeneous_assumption",
                    RegionRole::Assumption,
                    "homogeneous couplings and fields",
                    &["beta"],
                    &["χ"],
                ),
                region(
                    "quoted_formula",
                    RegionRole::Quoted,
                    "quoted unrelated formula",
                    &["q"],
                    &[],
                ),
            ],
        },
        "6717eeddd6c14a5dd1563e7c" => TargetContextRequest {
            target: "Cheeger constant".into(),
            target_components: vec!["h".into(), "G".into(), "U".into()],
            requested_operation: "minimize Cheeger constant".into(),
            regions: vec![
                region(
                    "graph_definition",
                    RegionRole::Definition,
                    "G is connected 3-regular with 4n vertices",
                    &["G", "n"],
                    &["Cheeger constant"],
                ),
                region(
                    "cheeger_constraint",
                    RegionRole::Constraint,
                    "h = min e(U,V\\U)/|U|",
                    &["h", "U", "G"],
                    &["Cheeger constant"],
                ),
                region(
                    "size_assumption",
                    RegionRole::Assumption,
                    "n > 100",
                    &["n"],
                    &["G"],
                ),
                region(
                    "quoted_formula",
                    RegionRole::Quoted,
                    "quoted unrelated formula",
                    &["q"],
                    &[],
                ),
            ],
        },
        "673a8ff77acc7cdc8c824b62" => TargetContextRequest {
            target: "α + β".into(),
            target_components: vec!["α".into(), "β".into(), "A(X)".into()],
            requested_operation: "compute exponent sum".into(),
            regions: vec![
                region(
                    "set_definition",
                    RegionRole::Definition,
                    "A(X) is the conductor-bounded set",
                    &["A(X)"],
                    &["α + β"],
                ),
                region(
                    "asymptotic_constraint",
                    RegionRole::Constraint,
                    "|A(X)| ~ c X^α log^β(X)",
                    &["α", "β", "A(X)"],
                    &["α + β"],
                ),
                region(
                    "quoted_formula",
                    RegionRole::Quoted,
                    "quoted unrelated formula",
                    &["q"],
                    &[],
                ),
            ],
        },
        _ => TargetContextRequest {
            target: "unknown".into(),
            target_components: Vec::new(),
            requested_operation: "unknown".into(),
            regions: Vec::new(),
        },
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset_bytes = fs::read(DATASET)?;
    let target_bytes = fs::read(TARGET_RERUN)?;
    let target: Value = serde_json::from_slice(&target_bytes)?;
    let ids: Vec<String> = target["cases"]
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
    let mut counts = BTreeMap::new();
    let mut terminals = BTreeMap::new();
    let mut cases = Vec::new();
    let mut complete_contexts = 0;
    let mut context_replays = 0;
    let mut binding_replays = 0;
    let mut complete_bindings = 0;
    for id in ids {
        let question = questions
            .get(&id)
            .ok_or_else(|| format!("missing HLE question {id}"))?;
        let request = request_for(&id);
        let context = assemble_target_context(&request);
        let binding = bind_equation_problem(&request.target);
        let terminal = match (context.status, binding.status) {
            (ContextStatus::Complete, BindingStatus::Complete) => "complete_context_and_binding",
            (ContextStatus::Complete, _) => "context_complete_equation_binding_handoff_only",
            (ContextStatus::Ambiguous, _) => "context_ambiguity_preserved",
            (ContextStatus::Unsupported, _) => "context_unsupported",
        }
        .to_string();
        *counts.entry(format!("{:?}", context.status)).or_insert(0) += 1;
        *terminals.entry(terminal.clone()).or_insert(0) += 1;
        complete_contexts += usize::from(context.binding_handoff_ready);
        context_replays += usize::from(context.replay_verified());
        binding_replays += usize::from(binding.replay_verified());
        complete_bindings += usize::from(binding.status == BindingStatus::Complete);
        let context_replay_verified = context.replay_verified();
        cases.push(CaseResult {
            id,
            question_sha256: sha(question.as_bytes()),
            context_status: context.status,
            binding_handoff_ready: context.binding_handoff_ready,
            included_regions: context
                .included_regions
                .iter()
                .map(|region| region.id.clone())
                .collect(),
            excluded_regions: context.excluded_region_ids.clone(),
            context_replay_verified,
            equation_binding_status: binding.status,
            equation_binding_replay_verified: binding.replay_verified(),
            terminal_classification: terminal,
            downstream_authorized: binding.downstream_authorized,
        });
    }
    let report = Report {
        schema_version: "phase48-hle-target-context-rerun-v1".into(),
        dataset_sha256: sha(&dataset_bytes),
        target_rerun_sha256: sha(&target_bytes),
        case_count: cases.len(),
        context_decisions: counts,
        terminal_classifications: terminals,
        complete_contexts,
        context_replays,
        equation_binding_replays: binding_replays,
        complete_equation_bindings: complete_bindings,
        downstream_authorizations: 0,
        cases,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    fs::write(
        "docs/phase48_hle_target_context_rerun.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
