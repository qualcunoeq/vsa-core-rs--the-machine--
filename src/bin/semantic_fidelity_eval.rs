//! Offline evaluator for the Phase 6 semantic-fidelity set.
//!
//! Runs the frozen gold corpus against the frozen stored proposals, replaying
//! the recorded bytes (`stored_output_replay`) without contacting a model and
//! without reading any answer key that the typed consumers do not need.  It
//! emits per-case records and a versioned report, and it fails the process if
//! the explicit wrong-answer limit is exceeded.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use the_machine::semantic_shadow::{
    compare_coverage, evaluate_shadow_record, frozen, stored_input, summarize, CoverageComparison,
    ReplayMode, ShadowInput, ShadowRecord, ShadowReport,
};
use the_machine::semantic_worker::SemanticWorker;

fn digest<T: Serialize>(value: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("eval value serializes"))
    )
}

const OUTPUT_PATH: &str = "docs/semantic_fidelity_eval_v1.jsonl";
const REPORT_PATH: &str = "docs/semantic_fidelity_eval_v1.report.json";
const MARKDOWN_PATH: &str = "docs/semantic_fidelity_eval_v1.md";
const WRONG_ANSWER_LIMIT: usize = 0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let worker = SemanticWorker::new(frozen_worker_config())?;
    let corpus = frozen::corpus();
    if !corpus.is_valid() {
        return Err(format!("frozen corpus invalid: {:?}", corpus.validation_errors()).into());
    }

    let mut records: Vec<ShadowRecord> = Vec::new();
    let mut jsonl = String::new();
    for case in &corpus.cases {
        let candidate = frozen::candidate_for(case);
        let raw_output = serde_json::to_string(&vec![candidate])?;
        let stored: ShadowInput = stored_input(&worker, &case.id, &case.prompt, raw_output);
        let expected_solver = frozen::expected_solver(case);
        let record = evaluate_shadow_record(&worker, &stored, case, expected_solver);
        jsonl.push_str(&serde_json::to_string(&record)?);
        jsonl.push('\n');
        records.push(record);
    }

    let report: ShadowReport = summarize(&records, WRONG_ANSWER_LIMIT);

    // Independent language-coverage measurement: the deterministic binder
    // abstains on these word problems, while the worker-assisted path reaches
    // a faithful, handoff-accepted interpretation.
    let coverage_corpus = frozen::coverage_corpus();
    if !coverage_corpus.is_valid() {
        return Err(format!(
            "coverage corpus invalid: {:?}",
            coverage_corpus.validation_errors()
        )
        .into());
    }
    let mut coverage_prompts = Vec::new();
    let mut coverage_records = Vec::new();
    for case in &coverage_corpus.cases {
        let candidate = frozen::coverage_candidate(case);
        let raw_output = serde_json::to_string(&vec![candidate])?;
        let stored = stored_input(&worker, &case.id, &case.prompt, raw_output);
        coverage_prompts.push(case.prompt.clone());
        coverage_records.push(evaluate_shadow_record(
            &worker,
            &stored,
            case,
            frozen::expected_solver(case),
        ));
    }
    let coverage: CoverageComparison = compare_coverage(&coverage_prompts, &coverage_records);
    if !coverage.worker_improves() {
        return Err(format!(
            "worker did not improve measured coverage: {coverage:?}"
        )
        .into());
    }
    let coverage_report = summarize(&coverage_records, WRONG_ANSWER_LIMIT);

    fs::write(OUTPUT_PATH, &jsonl)?;
    let report_json = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_PATH, format!("{report_json}\n"))?;

    let clarification = records
        .iter()
        .find_map(|record| record.clarification.clone())
        .unwrap_or_else(|| "(none)".into());
    let markdown = format!(
        "# Phase 6 — semantic-fidelity evaluation\n\n\
Offline replay of the frozen stored proposals against the frozen gold corpus. \
Stored-output replay is a re-decode of recorded bytes, not a model regeneration.\n\n\
* records: {records}\n\
* structurally accepted: {accepted}\n\
* interpretation accuracy: {interp}/{records} ({interp_pct:.1}%)\n\
* solver accuracy (faithful cases only): {solved}/{scored}\n\
* infidelities / structurally unusable: {infid} / {unusable}\n\
* verdicts proceed / clarify / reject: {proceed} / {clarify} / {reject}\n\
* silent wrong answers: {wrong} (limit {limit})\n\
* downstream authorizations: {auth}\n\
* replay mode: {mode}\n\
* coverage (baseline -> worker): {base_cov}/{cov_cases} -> {work_cov}/{cov_cases} (lift {lift})\n\
* coverage faithful answers: {work_faithful} (silent wrong {cov_wrong})\n\
* example clarification: {clarification}\n\
* records SHA-256: `{records_hash}`\n\
* report SHA-256: `{report_hash}`\n",
        records = report.records,
        accepted = report.structurally_accepted,
        interp = report.interpretations_correct,
        interp_pct = report.interpretation_accuracy() * 100.0,
        solved = report.solver_routes_correct,
        scored = report.solver_routes_scored,
        infid = report.infidelities,
        unusable = report.structurally_unusable,
        proceed = report.verdicts_proceeded,
        clarify = report.verdicts_clarified,
        reject = report.verdicts_rejected,
        wrong = report.silent_wrong_answers,
        limit = report.wrong_answer_limit,
        auth = report.downstream_authorizations,
        mode = report.replay_mode.label(),
        base_cov = coverage.baseline_covered,
        work_cov = coverage.worker_covered,
        cov_cases = coverage.cases,
        lift = coverage.coverage_lift,
        work_faithful = coverage.worker_faithful,
        cov_wrong = coverage_report.silent_wrong_answers,
        clarification = clarification,
        records_hash = digest(&jsonl),
        report_hash = report.report_hash,
    );
    fs::write(MARKDOWN_PATH, markdown)?;

    eprintln!(
        "records={} interpretation_correct={} solver_correct={}/{} infidelities={} silent_wrong={} limit={} report={REPORT_PATH}",
        report.records,
        report.interpretations_correct,
        report.solver_routes_correct,
        report.solver_routes_scored,
        report.infidelities,
        report.silent_wrong_answers,
        report.wrong_answer_limit,
    );

    if !report.within_wrong_answer_limit() {
        return Err(format!(
            "wrong-answer limit exceeded: {} > {}",
            report.silent_wrong_answers, report.wrong_answer_limit
        )
        .into());
    }
    if report.downstream_authorizations != 0 {
        return Err(format!(
            "shadow evaluation authorized {} downstream result(s); must be zero",
            report.downstream_authorizations
        )
        .into());
    }
    if report.replay_mode != ReplayMode::StoredOutputReplay {
        return Err("evaluator must report stored_output_replay".into());
    }
    Ok(())
}

fn frozen_worker_config() -> the_machine::semantic_worker::SemanticWorkerConfig {
    the_machine::semantic_shadow::evaluator_worker_config("frozen-model".into())
}
