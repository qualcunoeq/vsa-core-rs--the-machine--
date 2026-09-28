//! Phase 9 conversation evaluation driver.
//!
//! Runs the versioned conversation corpora (a frozen regression set and an
//! untouched evaluation set), captures and diffs traces, computes the metrics
//! the goal names, runs every mechanism ablation as a paired run, and writes:
//!
//! * `docs/phase9_conversation_eval_v1.report.json`
//! * `docs/phase9_conversation_eval_v1.md`
//! * `docs/phase9_conversation_eval_v1.trace.json` (regression trace)
//! * `docs/phase9_conversation_eval_holdout_v1.trace.json` (holdout trace)
//!
//! It exits non-zero if a held contract fails: regression drift, an
//! unsupported assertion beyond the declared limit, or an ablation that does
//! not preserve safety.
//!
//! The Phase 10 controlled-autonomy suite is run in the same driver (`task`
//! mode) and writes `docs/phase10_autonomy_v1.report.json` / `.md`; it exits
//! non-zero if any task exceeded its declared authority or budget, or failed to
//! explain its result.
//!
//! Modes:
//! * `score` (default) — full conversation run and contract gate;
//! * `capture` — (re)write the frozen traces from the current build;
//! * `replay`  — diff the current build against the frozen traces only;
//! * `task`    — run the controlled-autonomy task suite and gate it.

use std::fs;
use std::path::Path;

use the_machine::autonomy_task::run_task_suite;
use the_machine::conversation_eval::{
    capture_trace, compute_metrics, detect_drift, estimate_memory, run_ablations,
    run_corpus, ConversationEvalReport, Trace, EVAL_SCHEMA,
};

const REGRESSION_CORPUS: &str = "data/conversation_eval_v1.json";
const HOLDOUT_CORPUS: &str = "data/conversation_eval_holdout_v1.json";
const REPORT_JSON: &str = "docs/phase9_conversation_eval_v1.report.json";
const REPORT_MD: &str = "docs/phase9_conversation_eval_v1.md";
const REGRESSION_TRACE: &str = "docs/phase9_conversation_eval_v1.trace.json";
const HOLDOUT_TRACE: &str = "docs/phase9_conversation_eval_holdout_v1.trace.json";
const TASK_REPORT_JSON: &str = "docs/phase10_autonomy_v1.report.json";
const TASK_REPORT_MD: &str = "docs/phase10_autonomy_v1.md";
const DB_ROOT: &str = "data/conversation_eval";
const MEMORY_BUDGET_BYTES: usize = 64 * 1024 * 1024;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "score".to_string());
    let _ = fs::create_dir_all(DB_ROOT);

    if mode == "task" {
        return run_task_mode();
    }

    let regression = the_machine::conversation_eval::Corpus::from_path(REGRESSION_CORPUS)?;
    let holdout = the_machine::conversation_eval::Corpus::from_path(HOLDOUT_CORPUS)?;

    match mode.as_str() {
        "capture" => {
            let trace = capture_trace(&regression, Path::new(DB_ROOT))?;
            the_machine::conversation_eval::write_trace(&trace, REGRESSION_TRACE)?;
            let holdout_trace = capture_trace(&holdout, Path::new(DB_ROOT))?;
            the_machine::conversation_eval::write_trace(&holdout_trace, HOLDOUT_TRACE)?;
            eprintln!(
                "captured regression trace ({} turns, sha {}) and holdout trace ({} turns, sha {})",
                trace.turns.len(),
                trace.digest(),
                holdout_trace.turns.len(),
                holdout_trace.digest()
            );
            return Ok(());
        }
        "replay" => {
            let frozen = Trace::from_path(REGRESSION_TRACE)?;
            let current = capture_trace(&regression, Path::new(DB_ROOT))?;
            let drift = detect_drift(&frozen, &current);
            if drift.is_empty() {
                eprintln!("regression trace replay: no drift ({} turns)", current.turns.len());
                return Ok(());
            }
            for item in &drift {
                eprintln!(
                    "drift {} step {} {}: expected {:?}, got {:?}",
                    item.case, item.step, item.field, item.expected, item.actual
                );
            }
            return Err(format!("regression trace drifted in {} place(s)", drift.len()).into());
        }
        "score" => {}
        other => {
            return Err(format!(
                "unknown mode {other:?}; expected score, capture, replay, or task"
            )
            .into())
        }
    }

    // ── Regression run: metrics + trace drift ────────────────────────────
    let regression_trace = capture_trace(&regression, Path::new(DB_ROOT))?;
    let regression_drift = match Trace::from_path(REGRESSION_TRACE) {
        Ok(frozen) => detect_drift(&frozen, &regression_trace),
        Err(_) => Vec::new(),
    };
    let regression_turns = run_corpus(&regression, Path::new(DB_ROOT))?;
    let (regression_memory, _) = estimate_memory(&regression_turns, MEMORY_BUDGET_BYTES);
    let regression_metrics = compute_metrics(
        regression.case_count(),
        &regression_turns,
        regression_memory,
        MEMORY_BUDGET_BYTES,
    );

    // ── Holdout run: metrics only (never gated on score) ─────────────────
    let holdout_trace = capture_trace(&holdout, Path::new(DB_ROOT))?;
    let holdout_turns = run_corpus(&holdout, Path::new(DB_ROOT))?;
    let (holdout_memory, _) = estimate_memory(&holdout_turns, MEMORY_BUDGET_BYTES);
    let holdout_metrics = compute_metrics(
        holdout.case_count(),
        &holdout_turns,
        holdout_memory,
        MEMORY_BUDGET_BYTES,
    );

    // ── Paired ablations on the regression set ───────────────────────────
    let ablations = run_ablations(&regression, Path::new(DB_ROOT), MEMORY_BUDGET_BYTES)?;

    let report = ConversationEvalReport {
        schema: EVAL_SCHEMA.to_string(),
        regression: regression_metrics,
        holdout: holdout_metrics,
        regression_drift,
        ablations,
        trace_sha256: regression_trace.digest(),
        holdout_trace_sha256: holdout_trace.digest(),
    };

    write_artifacts(&report)?;

    eprintln!(
        "regression: turns={} answered={} coverage={:.3} oracle(scored/correct/wrong)={}/{}/{} unsupported={} drift={}",
        report.regression.turns,
        report.regression.answered,
        report.regression.coverage_rate,
        report.regression.oracle_scored,
        report.regression.oracle_correct,
        report.regression.oracle_wrong,
        report.regression.unsupported_assertions,
        report.regression_drift.len(),
    );
    eprintln!(
        "holdout:    turns={} answered={} coverage={:.3} oracle(scored/correct/wrong)={}/{}/{}",
        report.holdout.turns,
        report.holdout.answered,
        report.holdout.coverage_rate,
        report.holdout.oracle_scored,
        report.holdout.oracle_correct,
        report.holdout.oracle_wrong,
    );
    eprintln!(
        "ablations={} all_safety_preserved={} report={REPORT_JSON}",
        report.ablations.len(),
        report.ablations.iter().all(|a| a.safety_preserved),
    );

    // ── Contract gate ────────────────────────────────────────────────────
    report.contracts_hold()?;
    Ok(())
}

fn write_artifacts(report: &ConversationEvalReport) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(REPORT_JSON, format!("{}\n", report.to_json()))?;
    fs::write(REPORT_MD, report.to_markdown())?;
    Ok(())
}

/// Run the Phase 10 controlled-autonomy suite, write its artifacts, and gate.
fn run_task_mode() -> Result<(), Box<dyn std::error::Error>> {
    // The conversation initiates the work: a live service supplies its own
    // session/turn state to the consolidation task through the explicit
    // adapter. The service holds no threads and the tasks reach no shell.
    let mut service = the_machine::conversation::ConversationService::with_in_memory_storage()?;
    let session = service.open_session("autonomy-eval");
    service.handle_turn(&session, "Who manages the observatory?");
    service.handle_turn(&session, "The telescope is calibrated nightly.");

    let mut host = the_machine::autonomy_task::ConversationTaskHost::new(
        &mut service,
        the_machine::reliability::MemoryBudget::default(),
    );
    let report = run_task_suite(&mut host);

    fs::write(TASK_REPORT_JSON, format!("{}\n", report.to_json()))?;
    fs::write(TASK_REPORT_MD, report.to_markdown())?;

    eprintln!(
        "tasks: scenarios={} completed={} refused={} paused_resumed={} within_authority={} within_budget={} contracts={}",
        report.scenarios,
        report.completed,
        report.refused,
        report.paused_and_resumed,
        report.all_within_authority,
        report.all_within_budget,
        report.all_contracts_satisfied,
    );
    for outcome in &report.outcomes {
        eprintln!(
            "  {} ({}) :: {} :: {}",
            outcome.id,
            outcome.kind.label(),
            outcome.state.label(),
            outcome
                .result
                .as_ref()
                .map(|result| result.explain())
                .unwrap_or_else(|| outcome
                    .stop_reason
                    .clone()
                    .unwrap_or_else(|| "no result".to_string())),
        );
    }
    eprintln!("report={TASK_REPORT_JSON}");

    report.contracts_hold()?;
    Ok(())
}
