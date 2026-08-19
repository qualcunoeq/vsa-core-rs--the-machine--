//! Stage 337: source-selected transfer through an already validated sequence
//! language frontend.
//!
//! This probe deliberately separates two claims: the source-selected generic
//! formula frontend is tested in Stages 333/334, while this binary tests
//! whether the independently validated arithmetic-sequence language bridge
//! can consume the same selected, attributed source catalog.  It never reads
//! answer keys, authorizes production, or mutates the live manifest.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaStatus,
};
use the_machine::source_sequence_frontend::{
    formalize_sequence_terms_text, replay_verified as frontend_replay_verified,
    SequenceFrontendStatus,
};

const PLAN_PATH: &str = "docs/goal6_external_portfolio_source_plan.json";
const RELEASE_DIR: &str = "data/external_math_exam_v1";
const DEFAULT_JSON: &str = "docs/stage337_goal6_source_selected_sequence_transfer.json";
const DEFAULT_MD: &str = "docs/stage337_goal6_source_selected_sequence_transfer.md";
const ROUTE: &str = "ArithmeticSequence";
const DOMAIN: &str = "goal6_source_selected_arithmetic_sequence";

#[derive(Debug, Deserialize)]
struct SourcePlan {
    schema: String,
    input_gap_report_sha256: String,
    dataset_sha256: String,
    answer_keys_read: usize,
    source_ingestions: usize,
    promotion_proposals: usize,
    production_mutations: usize,
    manifest_unchanged: bool,
    plan_entries: Vec<PlanEntry>,
}

#[derive(Debug, Deserialize)]
struct PlanEntry {
    source_path: String,
    source_sha256: String,
    provenance_fields_present: bool,
    matching_route: Option<String>,
    executable_cases: usize,
    semantic_gate: String,
}

#[derive(Debug, Deserialize)]
struct Question {
    id: String,
    original_prompt: String,
    split: String,
}

#[derive(Debug, Serialize)]
struct Candidate {
    id: String,
    prompt_sha256: String,
    value_sha256: String,
    replay_verified: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    partition: String,
    questions_read: usize,
    selected_source_path: String,
    selected_source_sha256: String,
    selected_route: &'static str,
    source_records: usize,
    frontend_complete: usize,
    frontend_ambiguous: usize,
    frontend_missing: usize,
    frontend_unsupported: usize,
    execution_complete: usize,
    frontend_replay_verified: usize,
    execution_replay_verified: usize,
    frontend_tamper_rejected: usize,
    execution_tamper_rejected: usize,
    candidate_count: usize,
    candidates: Vec<Candidate>,
    answer_keys_read: usize,
    production_mutations: usize,
    manifest_unchanged: bool,
    source_plan_sha256: String,
    input_gap_report_sha256: String,
    dataset_sha256: String,
    report_sha256: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let plan_bytes = fs::read(PLAN_PATH)?;
    let plan: SourcePlan = serde_json::from_slice(&plan_bytes)?;
    assert_eq!(plan.schema, "goal6-external-portfolio-source-plan-v1");
    assert_eq!(plan.answer_keys_read, 0);
    assert_eq!(plan.source_ingestions, 0);
    assert_eq!(plan.promotion_proposals, 0);
    assert_eq!(plan.production_mutations, 0);
    assert!(plan.manifest_unchanged);
    let partition = env::var("GOAL6_PORTFOLIO_PARTITION").unwrap_or_else(|_| "development".into());
    assert!(matches!(partition.as_str(), "development" | "sealed"));
    if partition == "sealed" {
        assert_eq!(
            env::var("GOAL6_PORTFOLIO_PRIVILEGED_EVAL").as_deref(),
            Ok("true")
        );
    }
    let report_json = env::var("GOAL6_SOURCE_SELECTED_SEQUENCE_REPORT_JSON")
        .unwrap_or_else(|_| DEFAULT_JSON.into());
    let report_md =
        env::var("GOAL6_SOURCE_SELECTED_SEQUENCE_REPORT_MD").unwrap_or_else(|_| DEFAULT_MD.into());
    let selected = plan
        .plan_entries
        .iter()
        .find(|entry| {
            entry.provenance_fields_present
                && entry.matching_route.as_deref() == Some(ROUTE)
                && entry.semantic_gate == "complete_route_evidence"
                && entry.executable_cases > 0
        })
        .ok_or("no validated arithmetic-sequence source candidate")?;
    let source_bytes = fs::read(&selected.source_path)?;
    assert_eq!(digest_bytes(&source_bytes), selected.source_sha256);
    let source_text = String::from_utf8(source_bytes)?;
    let records = extract_formula_records(&source_text)
        .map_err(|errors| format!("selected source extraction failed: {errors:?}"))?;
    let question_bytes = fs::read(format!("{RELEASE_DIR}/questions.jsonl"))?;
    assert_eq!(digest_bytes(&question_bytes), plan.dataset_sha256);
    let questions: Vec<Question> = String::from_utf8(question_bytes)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<Vec<_>, _>>()?;
    let questions = questions
        .into_iter()
        .filter(|question| question.split == partition)
        .collect::<Vec<_>>();
    let manifest_before = breadth_first_manifest().replay_hash();
    let mut report = Report {
        schema: "stage337-goal6-source-selected-sequence-transfer-v1",
        partition: partition.clone(),
        questions_read: questions.len(),
        selected_source_path: selected.source_path.clone(),
        selected_source_sha256: selected.source_sha256.clone(),
        selected_route: ROUTE,
        source_records: records.len(),
        frontend_complete: 0,
        frontend_ambiguous: 0,
        frontend_missing: 0,
        frontend_unsupported: 0,
        execution_complete: 0,
        frontend_replay_verified: 0,
        execution_replay_verified: 0,
        frontend_tamper_rejected: 0,
        execution_tamper_rejected: 0,
        candidate_count: 0,
        candidates: Vec::new(),
        answer_keys_read: 0,
        production_mutations: 0,
        manifest_unchanged: false,
        source_plan_sha256: digest_bytes(&plan_bytes),
        input_gap_report_sha256: plan.input_gap_report_sha256,
        dataset_sha256: plan.dataset_sha256,
        report_sha256: String::new(),
    };
    for question in &questions {
        let frontend =
            formalize_sequence_terms_text(&question.original_prompt, &question.id, DOMAIN);
        match frontend.status {
            SequenceFrontendStatus::Complete => report.frontend_complete += 1,
            SequenceFrontendStatus::Ambiguous => report.frontend_ambiguous += 1,
            SequenceFrontendStatus::Missing => report.frontend_missing += 1,
            SequenceFrontendStatus::Unsupported => report.frontend_unsupported += 1,
        }
        if frontend_replay_verified(&frontend) {
            report.frontend_replay_verified += 1;
        }
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        if !frontend_replay_verified(&frontend_tampered) {
            report.frontend_tamper_rejected += 1;
        }
        let Some(request) = frontend.request.as_ref() else {
            continue;
        };
        let execution = evaluate_formula_records(request, DOMAIN, &records);
        if execution.status == FormulaStatus::Complete {
            report.execution_complete += 1;
            let value = execution.value.as_ref().expect("complete result has value");
            report.candidate_count += 1;
            report.candidates.push(Candidate {
                id: question.id.clone(),
                prompt_sha256: digest_bytes(question.original_prompt.as_bytes()),
                value_sha256: digest(value),
                replay_verified: execution.replay_verified(),
            });
        }
        if execution.replay_verified() {
            report.execution_replay_verified += 1;
        }
        let mut execution_tampered = execution.clone();
        execution_tampered.replay_hash.push('x');
        if !execution_tampered.replay_verified() {
            report.execution_tamper_rejected += 1;
        }
    }
    let manifest_after = breadth_first_manifest().replay_hash();
    report.manifest_unchanged = manifest_before == manifest_after;
    assert_eq!(report.answer_keys_read, 0);
    assert_eq!(report.production_mutations, 0);
    assert!(report.manifest_unchanged);
    let mut unsigned = serde_json::to_value(&report)?;
    unsigned["report_sha256"] = serde_json::Value::String(String::new());
    report.report_sha256 = digest(&unsigned);
    fs::write(
        &report_json,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        &report_md,
        format!(
            "# Stage 337 — source-selected sequence transfer\n\n\
             - Partition / questions read: {} / {}\n\
             - Selected source / route: `{}` / `{}`\n\
             - Source records: {}\n\
             - Frontend complete / ambiguous / missing / unsupported: {} / {} / {} / {}\n\
             - Execution complete / frontend replay / execution replay: {} / {} / {}\n\
             - Frontend / execution tamper rejection: {} / {}\n\
             - Shadow candidates: {}\n\
             - Answer keys / production mutations: {} / {}\n\
             - Manifest unchanged: {}\n\n\
             This probe reuses an independently validated sequence language bridge with the
             selected attributed source catalog; it is answer-key blind and clone-only.\n",
            report.partition,
            report.questions_read,
            report.selected_source_path,
            report.selected_route,
            report.source_records,
            report.frontend_complete,
            report.frontend_ambiguous,
            report.frontend_missing,
            report.frontend_unsupported,
            report.execution_complete,
            report.frontend_replay_verified,
            report.execution_replay_verified,
            report.frontend_tamper_rejected,
            report.execution_tamper_rejected,
            report.candidate_count,
            report.answer_keys_read,
            report.production_mutations,
            report.manifest_unchanged,
        ),
    )?;
    println!(
        "Stage 337 — partition={} questions={} candidates={} replay={}/{} false_auth=0",
        report.partition,
        report.questions_read,
        report.candidate_count,
        report.frontend_replay_verified,
        report.execution_replay_verified
    );
    Ok(())
}
