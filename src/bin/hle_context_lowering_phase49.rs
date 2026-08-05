//! Phase 49 shadow lowering rerun for the four frozen HLE target-context cases.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use the_machine::context_lowering::{lower_context_bundle, LoweringStatus, ProblemType};
use the_machine::target_context::{
    assemble_target_context, ContextRegion, RegionRole, TargetContextRequest,
};

const DATASET: &str = "data/hle.jsonl";
const CONTEXT_RERUN: &str = "docs/phase48_hle_target_context_rerun.json";

#[derive(Debug, Serialize)]
struct CaseResult {
    id: String,
    question_sha256: String,
    lowering_status: LoweringStatus,
    problem_type: Option<ProblemType>,
    requested_target: String,
    included_regions: Vec<String>,
    equations: Vec<String>,
    assumptions: Vec<String>,
    lowering_replay_verified: bool,
    terminal_classification: String,
    downstream_authorized: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema_version: String,
    dataset_sha256: String,
    context_rerun_sha256: String,
    case_count: usize,
    lowering_decisions: BTreeMap<String, usize>,
    terminal_classifications: BTreeMap<String, usize>,
    complete_lowered_problems: usize,
    lowering_replays: usize,
    candidate_answers: usize,
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
    let context_bytes = fs::read(CONTEXT_RERUN)?;
    let context: Value = serde_json::from_slice(&context_bytes)?;
    let ids: Vec<String> = context["cases"]
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
    let mut decisions = BTreeMap::new();
    let mut terminals = BTreeMap::new();
    let mut cases = Vec::new();
    let mut complete = 0;
    let mut replays = 0;
    for id in ids {
        let request = request_for(&id);
        let bundle = assemble_target_context(&request);
        let spec = lower_context_bundle(&bundle);
        let terminal = match spec.status {
            LoweringStatus::Complete => "complete_lowered_problem_specialist_method_gap",
            LoweringStatus::Ambiguous => "ambiguous_lowering",
            LoweringStatus::Unsupported => "unsupported_target_problem_type",
        }
        .to_string();
        *decisions.entry(format!("{:?}", spec.status)).or_insert(0) += 1;
        *terminals.entry(terminal.clone()).or_insert(0) += 1;
        complete += usize::from(spec.status == LoweringStatus::Complete);
        let replay_verified = spec.replay_verified();
        replays += usize::from(replay_verified);
        let question = questions
            .get(&id)
            .ok_or_else(|| format!("missing HLE question {id}"))?;
        cases.push(CaseResult {
            id,
            question_sha256: sha(question.as_bytes()),
            lowering_status: spec.status,
            problem_type: spec.problem_type,
            requested_target: spec.requested_target,
            included_regions: spec.provenance_region_ids,
            equations: spec.equations,
            assumptions: spec.assumptions,
            lowering_replay_verified: replay_verified,
            terminal_classification: terminal,
            downstream_authorized: spec.downstream_authorized,
        });
    }
    let report = Report {
        schema_version: "phase49-hle-context-lowering-rerun-v1".into(),
        dataset_sha256: sha(&dataset_bytes),
        context_rerun_sha256: sha(&context_bytes),
        case_count: cases.len(),
        lowering_decisions: decisions,
        terminal_classifications: terminals,
        complete_lowered_problems: complete,
        lowering_replays: replays,
        candidate_answers: 0,
        downstream_authorizations: 0,
        cases,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    fs::write(
        "docs/phase49_hle_context_lowering_rerun.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
