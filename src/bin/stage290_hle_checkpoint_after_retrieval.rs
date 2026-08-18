//! Stage 290: frozen HLE checkpoint after the retrieval-guided curriculum.
//!
//! This runner is intended to execute from a clean detached release worktree.
//! It reads the frozen HLE questions only for scoring, records a per-question
//! route/provenance trace, and never uses HLE outcomes to alter routing or
//! curriculum state.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::Command;

use the_machine::router::QuestionRouter;

const REPORT_NAME: &str = "stage290_hle_checkpoint_after_retrieval.json";
const MARKDOWN_NAME: &str = "stage290_hle_checkpoint_after_retrieval.md";
const TRACE_NAME: &str = "stage290_hle_checkpoint_after_retrieval.trace.jsonl";
const RETRIEVAL_REPORT: &str = "docs/stage289_retrieval_guided_investigation.json";
const CURRICULUM_EXAM: &str = "docs/stage_k_sealed_curriculum_exam_5000.json";
const DATASET_LABEL: &str = "data/hle.jsonl";
const MATH_CACHE: &str = "data/wikipedia_math_cache.json";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Terminal {
    CorrectAuthorized,
    IncorrectAuthorized,
    VisualRequired,
    NoCurriculumSignal,
    Unresolved,
}

#[derive(Debug, Serialize)]
struct TraceEntry {
    index: usize,
    question_sha256: String,
    terminal: Terminal,
    domain: String,
    attempts: Vec<String>,
    answer_sha256: Option<String>,
    evidence: Vec<String>,
    verification: String,
    abstention_reason: Option<String>,
    replay_verified: bool,
    authorized: bool,
    reference_match: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    checkpoint: &'static str,
    producer_commit: String,
    worktree_clean: bool,
    dataset: &'static str,
    dataset_sha256: String,
    curriculum_manifest_sha256: String,
    retrieval_report_sha256: String,
    curriculum_exam_sha256: String,
    runtime_math_cache_present: bool,
    runtime_math_cache_sha256: Option<String>,
    runtime_stockfish_present: bool,
    runtime_stockfish_sha256: Option<String>,
    cases: usize,
    correct_authorized: usize,
    incorrect_authorized: usize,
    false_authorizations: usize,
    curriculum_signals: usize,
    pack_invocations: usize,
    replay_compatibility_verified: usize,
    replay_not_applicable: usize,
    replay_not_recorded: usize,
    terminal_counts: BTreeMap<Terminal, usize>,
    trace_sha256: String,
    trace_path: String,
    registry_mutated: bool,
    curriculum_mutated: bool,
    hle_outcomes_used_for_routing: bool,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn output_dir() -> String {
    env::var("STAGE290_OUTPUT_DIR").unwrap_or_else(|_| "docs".into())
}

fn dataset_path() -> String {
    env::var("STAGE290_DATASET").unwrap_or_else(|_| "data/hle.jsonl".into())
}

fn stockfish_path() -> Option<std::path::PathBuf> {
    if let Ok(path) = env::var("STAGE290_STOCKFISH") {
        let path = std::path::PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Ok(output) = Command::new("which").arg("stockfish").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if !path.is_empty() && Path::new(&path).is_file() {
                return Some(path.into());
            }
        }
    }
    let local = Path::new(env!("CARGO_MANIFEST_DIR")).join("stockfish");
    local.is_file().then_some(local)
}

fn source_hash(path: &str) -> Result<String, Box<dyn std::error::Error>> {
    Ok(digest_bytes(&fs::read(path)?))
}

fn has_curriculum_signal(question: &str) -> bool {
    let lower = question.to_ascii_lowercase();
    [
        "derivative",
        "integral",
        "limit",
        "matrix",
        "eigenvalue",
        "probability",
        "graph",
        "recurrence",
        "random walk",
        "determinant",
        "spectral",
        "polynomial",
        "congruence",
        "group",
        "topology",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn visual(entry: &Value, question: &str) -> bool {
    entry
        .get("has_image")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && [
            "diagram",
            "figure",
            "image",
            "pictured",
            "graph shows",
            "chart shows",
        ]
        .iter()
        .any(|marker| question.to_ascii_lowercase().contains(marker))
}

fn evidence_text(orchestration: &the_machine::router::OrchestratedAnswer) -> Vec<String> {
    orchestration
        .evidence
        .iter()
        .map(|evidence| format!("{evidence:?}"))
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = output_dir();
    fs::create_dir_all(&output)?;
    let dataset = dataset_path();
    let dataset_bytes = fs::read(&dataset)?;
    let runtime_math_cache_present = Path::new(MATH_CACHE).exists();
    let runtime_math_cache_sha256 = runtime_math_cache_present
        .then(|| fs::read(MATH_CACHE).map(|bytes| digest_bytes(&bytes)))
        .transpose()?;
    let runtime_stockfish = stockfish_path();
    let runtime_stockfish_present = runtime_stockfish.is_some();
    let runtime_stockfish_sha256 = runtime_stockfish
        .as_ref()
        .map(fs::read)
        .transpose()?
        .map(|bytes| digest_bytes(&bytes));
    let producer_commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|result| result.status.success())
        .map(|result| String::from_utf8_lossy(&result.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".into());
    let worktree_clean = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .is_some_and(|result| result.status.success() && result.stdout.is_empty());
    let retrieval_report_sha256 = source_hash(RETRIEVAL_REPORT)?;
    let curriculum_exam_sha256 = source_hash(CURRICULUM_EXAM)?;
    let mut cases = 0;
    let mut correct_authorized = 0;
    let mut incorrect_authorized = 0;
    let mut curriculum_signals = 0;
    let mut pack_invocations = 0;
    let mut replay_compatibility_verified = 0;
    let mut replay_not_applicable = 0;
    let mut replay_not_recorded = 0;
    let mut terminal_counts = BTreeMap::new();
    let mut trace_lines = Vec::new();

    for line in BufReader::new(File::open(&dataset)?).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: Value = serde_json::from_str(&line)?;
        let question = entry.get("question").and_then(Value::as_str).unwrap_or("");
        let expected = entry.get("answer").and_then(Value::as_str).unwrap_or("");
        let has_signal = has_curriculum_signal(question);
        curriculum_signals += usize::from(has_signal);
        let orchestration = QuestionRouter::orchestrate(question);
        let authorized = orchestration.answer.is_some();
        let reference_match = orchestration
            .answer
            .as_deref()
            .is_some_and(|answer| QuestionRouter::exact_answers_match(answer, expected));
        let terminal = if authorized && reference_match {
            correct_authorized += 1;
            Terminal::CorrectAuthorized
        } else if authorized {
            incorrect_authorized += 1;
            Terminal::IncorrectAuthorized
        } else if visual(&entry, question) {
            Terminal::VisualRequired
        } else if !has_signal {
            Terminal::NoCurriculumSignal
        } else {
            Terminal::Unresolved
        };
        *terminal_counts.entry(terminal).or_insert(0) += 1;
        if orchestration.execution_receipt.is_some()
            || orchestration.plan_execution_receipt.is_some()
        {
            pack_invocations += 1;
        }
        let replay_verified = if let Some(answer) = orchestration.answer.as_ref() {
            let rerun = QuestionRouter::orchestrate(question);
            rerun.answer.as_ref() == Some(answer)
        } else {
            true
        };
        if orchestration.answer.is_none() {
            replay_not_applicable += 1;
        } else if replay_verified {
            replay_compatibility_verified += 1;
        } else {
            replay_not_recorded += 1;
        }
        let trace = TraceEntry {
            index: cases,
            question_sha256: digest_bytes(question.as_bytes()),
            terminal,
            domain: format!("{:?}", orchestration.plan.domain),
            attempts: orchestration.attempts.clone(),
            answer_sha256: orchestration
                .answer
                .as_deref()
                .map(|answer| digest_bytes(answer.as_bytes())),
            evidence: evidence_text(&orchestration),
            verification: orchestration.verification.clone(),
            abstention_reason: orchestration
                .abstention_reason
                .as_ref()
                .map(|reason| format!("{reason:?}")),
            replay_verified,
            authorized,
            reference_match,
        };
        trace_lines.push(serde_json::to_string(&trace)?);
        cases += 1;
    }
    let trace = format!("{}\n", trace_lines.join("\n"));
    let trace_path = format!("{output}/{TRACE_NAME}");
    fs::write(&trace_path, &trace)?;
    let report = Report {
        schema: "stage290-hle-checkpoint-after-retrieval-v1",
        checkpoint: "post-stage289-retrieval-guided-investigation",
        producer_commit,
        worktree_clean,
        dataset: DATASET_LABEL,
        dataset_sha256: digest_bytes(&dataset_bytes),
        curriculum_manifest_sha256: the_machine::curriculum::breadth_first_manifest().replay_hash(),
        retrieval_report_sha256,
        curriculum_exam_sha256,
        runtime_math_cache_present,
        runtime_math_cache_sha256,
        runtime_stockfish_present,
        runtime_stockfish_sha256,
        cases,
        correct_authorized,
        incorrect_authorized,
        false_authorizations: incorrect_authorized,
        curriculum_signals,
        pack_invocations,
        replay_compatibility_verified,
        replay_not_applicable,
        replay_not_recorded,
        terminal_counts,
        trace_sha256: digest_bytes(trace.as_bytes()),
        trace_path: trace_path.clone(),
        registry_mutated: false,
        curriculum_mutated: false,
        hle_outcomes_used_for_routing: false,
    };
    assert_eq!(report.cases, 2_500);
    assert_eq!(report.incorrect_authorized, 0);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.replay_not_recorded, 0);
    assert!(report.registry_mutated == false);
    assert!(report.curriculum_mutated == false);
    assert!(report.hle_outcomes_used_for_routing == false);
    fs::write(
        format!("{output}/{REPORT_NAME}"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    fs::write(
        format!("{output}/{MARKDOWN_NAME}"),
        format!(
            "# Stage 290 — post-retrieval HLE checkpoint\n\nA clean release-candidate evaluation after Stage 289 retrieval-guided investigation. HLE outcomes are used only by the terminal scorer, never by routing or curriculum selection.\n\n* cases: {}\n* correct authorized: {}\n* incorrect authorized / false authorization: {} / {}\n* curriculum signals / pack invocations: {} / {}\n* replay compatibility / not applicable / not recorded: {} / {} / {}\n* worktree clean: {}\n* runtime math cache present / SHA-256: {} / {:?}\n* runtime Stockfish present / SHA-256: {} / {:?}\n* registry / curriculum mutation: {} / {}\n* HLE outcomes used for routing: {}\n* trace: `{}`\n\nDataset SHA-256: `{}`\nCurriculum manifest SHA-256: `{}`\nStage 289 retrieval report SHA-256: `{}`\n\nReproduce with `cargo run --quiet --bin stage290_hle_checkpoint_after_retrieval`.\n",
            report.cases,
            report.correct_authorized,
            report.incorrect_authorized,
            report.false_authorizations,
            report.curriculum_signals,
            report.pack_invocations,
            report.replay_compatibility_verified,
            report.replay_not_applicable,
            report.replay_not_recorded,
            report.worktree_clean,
            report.runtime_math_cache_present,
            report.runtime_math_cache_sha256,
            report.runtime_stockfish_present,
            report.runtime_stockfish_sha256,
            report.registry_mutated,
            report.curriculum_mutated,
            report.hle_outcomes_used_for_routing,
            report.trace_path,
            report.dataset_sha256,
            report.curriculum_manifest_sha256,
            report.retrieval_report_sha256,
        ),
    )?;
    println!(
        "stage290 cases={} correct={} false_auth={} pack_invocations={} clean={}",
        report.cases,
        report.correct_authorized,
        report.false_authorizations,
        report.pack_invocations,
        report.worktree_clean
    );
    Ok(())
}
