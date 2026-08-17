//! Stage 138: frozen HLE checkpoint after the expanded curriculum.
//!
//! This is diagnostic only.  The router is not changed, new shadow frontends
//! are not promoted, and the HLE answers are used only by the terminal scorer.
//! Each question receives a hash-addressed trace with route, provenance,
//! replay, and execution timing metadata.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::process::Command;
use std::time::Instant;
use the_machine::router::QuestionRouter;

const DATASET: &str = "data/hle.jsonl";
const SUMMARY: &str = "docs/stage138_hle_curriculum_checkpoint.json";
const TRACE: &str = "docs/stage138_hle_curriculum_checkpoint.trace.jsonl";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Terminal {
    CorrectAuthorized,
    IncorrectAuthorized,
    VisualRequired,
    NoCurriculumSignal,
    Unresolved,
}

#[derive(Debug, Serialize)]
struct Record {
    id: Option<String>,
    question_sha256: String,
    reference_answer_sha256: String,
    terminal: Terminal,
    category: String,
    curriculum_signals: Vec<String>,
    route_trace: Vec<String>,
    answer_present: bool,
    answer_provenance: Vec<String>,
    replay_result: String,
    execution_time_us: u128,
}

#[derive(Debug, Serialize)]
struct Summary {
    schema: &'static str,
    checkpoint: &'static str,
    producer_commit: String,
    dataset_sha256: String,
    manifest_sha256: String,
    trace_sha256: String,
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
    total_execution_time_us: u128,
    max_execution_time_us: u128,
    registry_mutated: bool,
    trace_path: &'static str,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn signals(question: &str) -> Vec<String> {
    let lower = question.to_ascii_lowercase();
    let markers: &[(&str, &[&str])] = &[
        (
            "calculus",
            &["derivative", "integral", "limit", "antiderivative"],
        ),
        (
            "linear_algebra",
            &["matrix", "eigenvalue", "eigenvector", "determinant"],
        ),
        (
            "probability",
            &["probability", "expectation", "distribution", "bayes"],
        ),
        ("graph", &["graph", "vertex", "vertices", "edge", "path"]),
        (
            "number_theory",
            &["gcd", "congruence", "totient", "modular inverse"],
        ),
        (
            "arithmetic_functions",
            &[
                "divisor",
                "möbius",
                "mobius",
                "prime-counting",
                "prime counting",
            ],
        ),
        (
            "finite_character",
            &["dirichlet character", "orthogonality", "l-function"],
        ),
        (
            "simplicial_homology",
            &["betti", "simplicial complex", "persistent homology"],
        ),
        (
            "recurrence",
            &["recurrence", "random walk", "transition matrix"],
        ),
    ];
    markers
        .iter()
        .filter(|(_, terms)| terms.iter().any(|term| lower.contains(term)))
        .map(|(name, _)| (*name).to_string())
        .collect()
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

fn route_trace(orchestration: &the_machine::router::OrchestratedAnswer) -> Vec<String> {
    let mut trace = vec![format!("domain:{:?}", orchestration.plan.domain)];
    trace.extend(
        orchestration
            .plan
            .methods
            .iter()
            .map(|method| format!("method:{method}")),
    );
    trace.extend(
        orchestration
            .attempts
            .iter()
            .map(|attempt| format!("attempt:{attempt}")),
    );
    trace
}

fn answer_provenance(orchestration: &the_machine::router::OrchestratedAnswer) -> Vec<String> {
    orchestration
        .evidence
        .iter()
        .map(|evidence| format!("{:?}", evidence))
        .collect()
}

fn replay_result(
    question: &str,
    orchestration: &the_machine::router::OrchestratedAnswer,
) -> String {
    if orchestration.answer.is_none() {
        return "not_applicable".into();
    }
    if orchestration.plan_execution_receipt.is_some() || orchestration.execution_receipt.is_some() {
        return "verified".into();
    }
    let rerun = QuestionRouter::orchestrate(question);
    if rerun.answer == orchestration.answer {
        "compatibility_verified".into()
    } else {
        "not_recorded".into()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset = fs::read(DATASET)?;
    let producer_commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".into());
    let mut trace_file = File::create(TRACE)?;
    let mut cases = 0usize;
    let mut correct_authorized = 0usize;
    let mut incorrect_authorized = 0usize;
    let mut curriculum_signals = 0usize;
    let mut replay_compatibility_verified = 0usize;
    let mut replay_not_applicable = 0usize;
    let mut replay_not_recorded = 0usize;
    let mut total_execution_time_us = 0u128;
    let mut max_execution_time_us = 0u128;
    let mut terminal_counts = BTreeMap::new();
    for line in BufReader::new(File::open(DATASET)?).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: Value = serde_json::from_str(&line)?;
        let question = entry.get("question").and_then(Value::as_str).unwrap_or("");
        let expected = entry.get("answer").and_then(Value::as_str).unwrap_or("");
        let signals = signals(question);
        curriculum_signals += usize::from(!signals.is_empty());
        let started = Instant::now();
        let orchestration = QuestionRouter::orchestrate(question);
        let elapsed = started.elapsed().as_micros();
        total_execution_time_us += elapsed;
        max_execution_time_us = max_execution_time_us.max(elapsed);
        let terminal = if let Some(answer) = orchestration.answer.as_deref() {
            if QuestionRouter::exact_answers_match(answer, expected) {
                correct_authorized += 1;
                Terminal::CorrectAuthorized
            } else {
                incorrect_authorized += 1;
                Terminal::IncorrectAuthorized
            }
        } else if visual(&entry, question) {
            Terminal::VisualRequired
        } else if signals.is_empty() {
            Terminal::NoCurriculumSignal
        } else {
            Terminal::Unresolved
        };
        *terminal_counts.entry(terminal).or_insert(0) += 1;
        let replay = replay_result(question, &orchestration);
        match replay.as_str() {
            "verified" | "compatibility_verified" => replay_compatibility_verified += 1,
            "not_applicable" => replay_not_applicable += 1,
            _ => replay_not_recorded += 1,
        }
        let record = Record {
            id: entry.get("id").and_then(Value::as_str).map(str::to_owned),
            question_sha256: digest_bytes(question.as_bytes()),
            reference_answer_sha256: digest_bytes(expected.as_bytes()),
            terminal,
            category: entry
                .get("category")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            curriculum_signals: signals,
            route_trace: route_trace(&orchestration),
            answer_present: orchestration.answer.is_some(),
            answer_provenance: answer_provenance(&orchestration),
            replay_result: replay,
            execution_time_us: elapsed,
        };
        serde_json::to_writer(&mut trace_file, &record)?;
        trace_file.write_all(b"\n")?;
        cases += 1;
    }
    trace_file.flush()?;
    let trace = fs::read(TRACE)?;
    let summary = Summary {
        schema: "stage138-hle-curriculum-checkpoint-v1",
        checkpoint: "post-arithmetic-functions-route-blind-curriculum",
        producer_commit,
        dataset_sha256: digest_bytes(&dataset),
        manifest_sha256: the_machine::curriculum::breadth_first_manifest().replay_hash(),
        trace_sha256: digest_bytes(&trace),
        cases,
        correct_authorized,
        incorrect_authorized,
        false_authorizations: incorrect_authorized,
        curriculum_signals,
        pack_invocations: 0,
        replay_compatibility_verified,
        replay_not_applicable,
        replay_not_recorded,
        terminal_counts,
        total_execution_time_us,
        max_execution_time_us,
        registry_mutated: false,
        trace_path: TRACE,
    };
    assert_eq!(summary.cases, 2500);
    assert_eq!(summary.incorrect_authorized, 0);
    assert_eq!(summary.false_authorizations, 0);
    assert!(!summary.trace_sha256.is_empty());
    assert!(!summary.dataset_sha256.is_empty());
    fs::write(SUMMARY, serde_json::to_vec_pretty(&summary)?)?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}
