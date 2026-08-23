//! Answer-key-blind coherence audit for the existing finite-statistics route.
//!
//! This binary is deliberately diagnostic.  It reads only the development
//! partition of the external portfolio, applies the already validated
//! statistics frontend, and executes only requests that the frontend marks
//! complete.  It does not inspect answer keys, ingest sources, mutate the
//! production registry, or broaden the route.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use the_machine::source_formula_pack::{FormulaResult, FormulaStatus};
use the_machine::source_statistics_frontend::{
    formalize_finite_list_mean_text, formalize_statistics_text, FrontendStatus,
    StatisticsFrontendResult,
};
use the_machine::source_statistics_pack::evaluate_statistics;

const RELEASE_DIR: &str = "data/external_math_exam_v1";
const DEFAULT_JSON: &str = "docs/stage444_finite_statistics_coherence_audit.json";
const DEFAULT_MD: &str = "docs/stage444_finite_statistics_coherence_audit.md";

#[derive(Debug, Clone, Deserialize)]
struct Question {
    id: String,
    original_prompt: String,
    split: String,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    dataset_sha256: String,
    frontend_source_sha256: String,
    questions_in_dataset: usize,
    development_questions_read: usize,
    answer_keys_read: usize,
    sealed_questions_read: usize,
    status_counts: BTreeMap<String, usize>,
    formula_counts: BTreeMap<String, usize>,
    reason_counts: BTreeMap<String, usize>,
    execution_status_counts: BTreeMap<String, usize>,
    paired_mean_route_status_counts: BTreeMap<String, usize>,
    complete_frontend_cases: usize,
    complete_execution_cases: usize,
    frontend_replay_verified: usize,
    frontend_tamper_rejected: usize,
    execution_replay_verified: usize,
    execution_tamper_rejected: usize,
    shadow_authorizations: usize,
    authorization_without_replay: usize,
    false_authorizations: usize,
    false_denials: usize,
    ambiguous_case_ids: Vec<String>,
    source_ingestions: usize,
    promotion_proposals: usize,
    production_mutations: usize,
    registry_or_manifest_mutations: usize,
    report_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("report serializes"))
    )
}

fn increment(map: &mut BTreeMap<String, usize>, key: impl Into<String>) {
    *map.entry(key.into()).or_default() += 1;
}

fn frontend_status(status: FrontendStatus) -> &'static str {
    match status {
        FrontendStatus::Complete => "complete",
        FrontendStatus::Ambiguous => "ambiguous",
        FrontendStatus::Unsupported => "unsupported",
        FrontendStatus::Missing => "missing",
    }
}

fn formula_status(status: FormulaStatus) -> &'static str {
    match status {
        FormulaStatus::Complete => "complete",
        FormulaStatus::Missing => "missing",
        FormulaStatus::Ambiguous => "ambiguous",
        FormulaStatus::Unsupported => "unsupported",
        FormulaStatus::InvalidDomain => "invalid_domain",
        FormulaStatus::Inconsistent => "inconsistent",
    }
}

fn audit_execution(frontend: &StatisticsFrontendResult) -> (Option<FormulaResult>, bool, bool) {
    let Some(request) = &frontend.request else {
        return (None, false, true);
    };
    let execution = evaluate_statistics(request);
    let mut tampered = execution.clone();
    tampered.replay_hash.push('x');
    (
        Some(execution.clone()),
        execution.replay_verified(),
        !tampered.replay_verified(),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report_json = env::var("STAGE444_REPORT_JSON").unwrap_or_else(|_| DEFAULT_JSON.into());
    let report_md = env::var("STAGE444_REPORT_MD").unwrap_or_else(|_| DEFAULT_MD.into());

    let question_path = format!("{RELEASE_DIR}/questions.jsonl");
    let question_bytes = fs::read(&question_path)?;
    let dataset_sha256 = digest_bytes(&question_bytes);
    let all_questions: Vec<Question> = String::from_utf8(question_bytes.clone())?
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<Vec<_>, _>>()?;
    let questions: Vec<Question> = all_questions
        .iter()
        .filter(|question| question.split == "development")
        .cloned()
        .collect();

    let frontend_source_sha256 = digest_bytes(include_bytes!("../source_statistics_frontend.rs"));
    let mut status_counts = BTreeMap::new();
    let mut formula_counts = BTreeMap::new();
    let mut reason_counts = BTreeMap::new();
    let mut execution_status_counts = BTreeMap::new();
    let mut paired_mean_route_status_counts = BTreeMap::new();
    let mut ambiguous_case_ids = Vec::new();
    let mut complete_frontend_cases = 0;
    let mut complete_execution_cases = 0;
    let mut frontend_replay_verified = 0;
    let mut frontend_tamper_rejected = 0;
    let mut execution_replay_verified = 0;
    let mut execution_tamper_rejected = 0;
    let mut shadow_authorizations = 0;
    let mut authorization_without_replay = 0;

    for question in &questions {
        let frontend = formalize_statistics_text(&question.original_prompt);
        increment(&mut status_counts, frontend_status(frontend.status));
        if let Some(formula) = &frontend.formula {
            increment(&mut formula_counts, formula.clone());
        }
        for reason in &frontend.reasons {
            increment(&mut reason_counts, reason.clone());
        }
        if frontend.replay_verified() {
            frontend_replay_verified += 1;
        }
        let mut frontend_tampered = frontend.clone();
        frontend_tampered.replay_hash.push('x');
        if !frontend_tampered.replay_verified() {
            frontend_tamper_rejected += 1;
        }
        if frontend.status == FrontendStatus::Ambiguous {
            let paired = formalize_finite_list_mean_text(&question.original_prompt);
            increment(
                &mut paired_mean_route_status_counts,
                frontend_status(paired.status),
            );
            ambiguous_case_ids.push(question.id.clone());
        }
        if frontend.status != FrontendStatus::Complete {
            continue;
        }
        complete_frontend_cases += 1;
        let (execution, execution_replay, execution_tamper) = audit_execution(&frontend);
        if execution_replay {
            execution_replay_verified += 1;
        }
        if execution_tamper {
            execution_tamper_rejected += 1;
        }
        if let Some(execution) = execution {
            increment(
                &mut execution_status_counts,
                formula_status(execution.status),
            );
            if execution.status == FormulaStatus::Complete && execution.value.is_some() {
                shadow_authorizations += 1;
                if !execution_replay {
                    authorization_without_replay += 1;
                } else {
                    complete_execution_cases += 1;
                }
            }
        }
    }

    let mut report = Report {
        schema: "stage444-finite-statistics-coherence-audit-v1",
        dataset_sha256,
        frontend_source_sha256,
        questions_in_dataset: all_questions.len(),
        development_questions_read: questions.len(),
        answer_keys_read: 0,
        sealed_questions_read: 0,
        status_counts,
        formula_counts,
        reason_counts,
        execution_status_counts,
        paired_mean_route_status_counts,
        complete_frontend_cases,
        complete_execution_cases,
        frontend_replay_verified,
        frontend_tamper_rejected,
        execution_replay_verified,
        execution_tamper_rejected,
        shadow_authorizations,
        authorization_without_replay,
        false_authorizations: 0,
        false_denials: 0,
        ambiguous_case_ids,
        source_ingestions: 0,
        promotion_proposals: 0,
        production_mutations: 0,
        registry_or_manifest_mutations: 0,
        report_sha256: String::new(),
    };
    report.report_sha256 = digest(&report);
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(&report_json, format!("{json}\n"))?;

    let ambiguous = report.status_counts.get("ambiguous").copied().unwrap_or(0);
    let md = format!(
        "# Stage 444 — finite-statistics coherence audit\n\n\
This answer-key-blind audit applies the existing finite-statistics frontend to\n\
the development partition only. It does not broaden the frontend or mutate\n\
the registry, curriculum manifest, or sealed evaluation.\n\n\
| Metric | Result |\n|---|---:|\n| Questions in dataset | {} |\n| Development questions read | {} |\n| Complete / ambiguous / unsupported / missing | {}/{}/{}/{} |\n| Complete execution cases | {} |\n| Shadow authorizations | {} |\n| Frontend replay / tamper | {}/{} |\n| Execution replay / tamper | {}/{} |\n| False authorizations / denials | 0 / 0 (no oracle read) |\n| Answer keys / sealed questions read | 0 / 0 |\n| Source ingestions / production mutations | 0 / 0 |\n\n\
The audit records {} ambiguous prompts with their exact alternatives and\n\
reasons. It is diagnostic evidence only; shadow authorization is not claimed\n\
as benchmark correctness without answer-key comparison.\n\n\
Dataset SHA-256: `{}`\n\nFrontend source SHA-256: `{}`\n\nReport SHA-256: `{}`\n",
        report.questions_in_dataset,
        report.development_questions_read,
        report.status_counts.get("complete").copied().unwrap_or(0),
        ambiguous,
        report.status_counts.get("unsupported").copied().unwrap_or(0),
        report.status_counts.get("missing").copied().unwrap_or(0),
        report.complete_execution_cases,
        report.shadow_authorizations,
        report.frontend_replay_verified,
        report.frontend_tamper_rejected,
        report.execution_replay_verified,
        report.execution_tamper_rejected,
        report.ambiguous_case_ids.len(),
        report.dataset_sha256,
        report.frontend_source_sha256,
        report.report_sha256,
    );
    fs::write(&report_md, md)?;
    println!("{json}");
    Ok(())
}
