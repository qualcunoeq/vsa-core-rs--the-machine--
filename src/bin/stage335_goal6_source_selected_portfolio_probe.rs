//! Stage 335: route a selected source-derived frontend over the external
//! portfolio without reading answer keys or mutating the live registry.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use the_machine::curriculum::breadth_first_manifest;
use the_machine::source_formula_frontend::formalize_source_formula_text;
use the_machine::source_formula_pack::{
    evaluate_formula_records, extract_formula_records, FormulaStatus,
};

const PLAN_PATH: &str = "docs/goal6_external_portfolio_source_plan.json";
const QUESTIONS_PATH: &str = "data/external_math_exam_v1/questions.jsonl";
const DEFAULT_JSON: &str = "docs/stage335_goal6_source_selected_portfolio_probe.json";
const DEFAULT_MD: &str = "docs/stage335_goal6_source_selected_portfolio_probe.md";
const DOMAIN: &str = "goal6_source_selected_finite_statistics";

#[derive(Debug, Deserialize)]
struct Plan {
    schema: String,
    input_gap_report_sha256: String,
    dataset_sha256: String,
    answer_keys_read: usize,
    source_ingestions: usize,
    promotion_proposals: usize,
    production_mutations: usize,
    manifest_unchanged: bool,
    plan_entries: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
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
    question_id: String,
    formula_id: String,
    candidate_hash: String,
    frontend_replay_verified: bool,
    frontend_tamper_rejected: bool,
    execution_replay_verified: bool,
    execution_tamper_rejected: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    partition: String,
    source_plan_sha256: String,
    input_gap_report_sha256: String,
    dataset_sha256: String,
    selected_source_path: String,
    selected_source_sha256: String,
    selected_route: String,
    questions_read: usize,
    frontend_complete: usize,
    frontend_ambiguous: usize,
    frontend_missing: usize,
    frontend_unsupported: usize,
    frontend_replay_verified: usize,
    frontend_tamper_rejected: usize,
    execution_complete: usize,
    execution_replay_verified: usize,
    execution_tamper_rejected: usize,
    candidate_count: usize,
    candidates: Vec<Candidate>,
    answer_keys_read: usize,
    plaintext_answers_read: usize,
    production_mutations: usize,
    manifest_unchanged: bool,
    report_sha256: String,
}

fn digest<T: Serialize + ?Sized>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let partition = env::var("GOAL6_PORTFOLIO_PARTITION").unwrap_or_else(|_| "development".into());
    assert!(matches!(partition.as_str(), "development" | "sealed"));
    if partition == "sealed" {
        assert_eq!(
            env::var("GOAL6_PORTFOLIO_PRIVILEGED_EVAL").ok().as_deref(),
            Some("true"),
            "sealed probing requires explicit privileged evaluation"
        );
    }
    let report_json =
        env::var("GOAL6_SOURCE_SELECTED_REPORT_JSON").unwrap_or_else(|_| DEFAULT_JSON.into());
    let report_md =
        env::var("GOAL6_SOURCE_SELECTED_REPORT_MD").unwrap_or_else(|_| DEFAULT_MD.into());
    let plan_bytes = fs::read(PLAN_PATH)?;
    let plan: Plan = serde_json::from_slice(&plan_bytes)?;
    assert_eq!(plan.schema, "goal6-external-portfolio-source-plan-v1");
    assert_eq!(plan.answer_keys_read, 0);
    assert_eq!(plan.source_ingestions, 0);
    assert_eq!(plan.promotion_proposals, 0);
    assert_eq!(plan.production_mutations, 0);
    assert!(plan.manifest_unchanged);
    let selected = plan
        .plan_entries
        .iter()
        .filter(|entry| {
            entry.provenance_fields_present
                && entry.matching_route.as_deref() == Some("FiniteListMean")
                && entry.semantic_gate == "complete_route_evidence"
                && entry.executable_cases > 0
        })
        .max_by_key(|entry| entry.executable_cases)
        .ok_or("no source candidate with complete provenance and route evidence")?;
    let source_bytes = fs::read(&selected.source_path)?;
    assert_eq!(digest_bytes(&source_bytes), selected.source_sha256);
    let records = extract_formula_records(std::str::from_utf8(&source_bytes)?)
        .map_err(|errors| format!("source extraction failed: {errors:?}"))?;
    let question_bytes = fs::read(QUESTIONS_PATH)?;
    assert_eq!(digest_bytes(&question_bytes), plan.dataset_sha256);
    let questions: Vec<Question> = std::str::from_utf8(&question_bytes)?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<Vec<_>, _>>()?;
    let questions: Vec<Question> = questions
        .into_iter()
        .filter(|question| question.split == partition)
        .collect();
    let manifest_before = breadth_first_manifest().replay_hash();
    let mut frontend_complete = 0;
    let mut frontend_ambiguous = 0;
    let mut frontend_missing = 0;
    let mut frontend_unsupported = 0;
    let mut frontend_replay_verified = 0;
    let mut frontend_tamper_rejected = 0;
    let mut execution_complete = 0;
    let mut execution_replay_verified = 0;
    let mut execution_tamper_rejected = 0;
    let mut candidates = Vec::new();
    for question in &questions {
        let frontend = formalize_source_formula_text(&question.original_prompt, DOMAIN, &records);
        match frontend.status {
            the_machine::source_formula_frontend::FrontendStatus::Complete => {
                frontend_complete += 1
            }
            the_machine::source_formula_frontend::FrontendStatus::Ambiguous => {
                frontend_ambiguous += 1
            }
            the_machine::source_formula_frontend::FrontendStatus::Missing => frontend_missing += 1,
            the_machine::source_formula_frontend::FrontendStatus::Unsupported => {
                frontend_unsupported += 1
            }
        }
        frontend_replay_verified += usize::from(frontend.replay_verified());
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        frontend_tamper_rejected += usize::from(!frontend_tampered.replay_verified());
        let Some(request) = frontend.request.as_ref() else {
            continue;
        };
        let execution = evaluate_formula_records(request, DOMAIN, &records);
        if execution.status != FormulaStatus::Complete {
            continue;
        }
        execution_complete += 1;
        execution_replay_verified += usize::from(execution.replay_verified());
        let mut execution_tampered = execution.clone();
        execution_tampered.replay_hash.push('x');
        execution_tamper_rejected += usize::from(!execution_tampered.replay_verified());
        if execution.replay_verified() && !execution_tampered.replay_verified() {
            candidates.push(Candidate {
                question_id: question.id.clone(),
                formula_id: request.formula.clone(),
                candidate_hash: digest(&execution.value),
                frontend_replay_verified: frontend.replay_verified(),
                frontend_tamper_rejected: !frontend_tampered.replay_verified(),
                execution_replay_verified: execution.replay_verified(),
                execution_tamper_rejected: !execution_tampered.replay_verified(),
            });
        }
    }
    let manifest_after = breadth_first_manifest().replay_hash();
    let mut report = Report {
        schema: "stage335-goal6-source-selected-portfolio-probe-v1",
        partition,
        source_plan_sha256: digest_bytes(&plan_bytes),
        input_gap_report_sha256: plan.input_gap_report_sha256,
        dataset_sha256: plan.dataset_sha256,
        selected_source_path: selected.source_path.clone(),
        selected_source_sha256: selected.source_sha256.clone(),
        selected_route: selected.matching_route.clone().unwrap_or_default(),
        questions_read: questions.len(),
        frontend_complete,
        frontend_ambiguous,
        frontend_missing,
        frontend_unsupported,
        frontend_replay_verified,
        frontend_tamper_rejected,
        execution_complete,
        execution_replay_verified,
        execution_tamper_rejected,
        candidate_count: candidates.len(),
        candidates,
        answer_keys_read: 0,
        plaintext_answers_read: 0,
        production_mutations: 0,
        manifest_unchanged: manifest_before == manifest_after,
        report_sha256: String::new(),
    };
    let mut unsigned = serde_json::to_value(&report)?;
    unsigned["report_sha256"] = serde_json::Value::String(String::new());
    report.report_sha256 = digest(&unsigned);
    assert_eq!(report.answer_keys_read, 0);
    assert_eq!(report.plaintext_answers_read, 0);
    assert_eq!(report.production_mutations, 0);
    assert!(report.manifest_unchanged);
    fs::write(
        &report_json,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    fs::write(
        &report_md,
        format!(
            "# Stage 335 — source-selected external portfolio probe\n\n\
             - Partition / questions read: {} / {}\n\
             - Selected source / route: `{}` / `{}`\n\
             - Frontend complete / ambiguous / missing / unsupported: {} / {} / {} / {}\n\
             - Frontend replay / tamper: {} / {}\n\
             - Execution complete / replay / tamper: {} / {} / {}\n\
             - Shadow candidates: {}\n\
             - Answer keys / plaintext answers / production mutations: {} / {} / {}\n\
             - Manifest unchanged: {}\n\n\
             This probe is answer-key blind and shadow-only. Candidate values are \
             recorded only as hashes; no candidate authorizes production routing.\n",
            report.partition,
            report.questions_read,
            report.selected_source_path,
            report.selected_route,
            report.frontend_complete,
            report.frontend_ambiguous,
            report.frontend_missing,
            report.frontend_unsupported,
            report.frontend_replay_verified,
            report.frontend_tamper_rejected,
            report.execution_complete,
            report.execution_replay_verified,
            report.execution_tamper_rejected,
            report.candidate_count,
            report.answer_keys_read,
            report.plaintext_answers_read,
            report.production_mutations,
            report.manifest_unchanged,
        ),
    )?;
    println!(
        "Stage 335 — partition={} questions={} candidates={} replay={}/{} false_auth=0",
        report.partition,
        report.questions_read,
        report.candidate_count,
        report.frontend_replay_verified,
        report.execution_replay_verified
    );
    Ok(())
}
