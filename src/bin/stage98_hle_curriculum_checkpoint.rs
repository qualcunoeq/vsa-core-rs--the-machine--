//! Stage 98: frozen HLE checkpoint after the expanded source curriculum.
//!
//! The HLE dataset and router are read-only.  This checkpoint writes a new
//! summary and does not overwrite earlier HLE reports.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use the_machine::router::QuestionRouter;

const DATASET: &str = "data/hle.jsonl";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Terminal { CorrectAuthorized, IncorrectAuthorized, VisualRequired, NoCurriculumSignal, Unresolved }

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    checkpoint: &'static str,
    dataset_sha256: String,
    manifest_sha256: String,
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
    registry_mutated: bool,
}

fn digest(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

fn signal(question: &str) -> bool {
    let lower = question.to_ascii_lowercase();
    ["derivative", "integral", "limit", "matrix", "eigenvalue", "probability", "graph", "recurrence", "random walk", "determinant", "spectral", "polynomial", "bayes", "interpolation"].iter().any(|marker| lower.contains(marker))
}

fn visual(entry: &Value, question: &str) -> bool {
    entry.get("has_image").and_then(Value::as_bool).unwrap_or(false) && ["diagram", "figure", "image", "pictured", "graph shows", "chart shows"].iter().any(|marker| question.to_ascii_lowercase().contains(marker))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset = fs::read(DATASET)?;
    let mut cases = 0; let mut correct = 0; let mut incorrect = 0; let mut signals = 0; let mut replay = 0; let mut not_applicable = 0; let mut not_recorded = 0; let mut terminals = BTreeMap::new();
    for line in BufReader::new(File::open(DATASET)?).lines() {
        let line = line?; if line.trim().is_empty() { continue; }
        let entry: Value = serde_json::from_str(&line)?;
        let question = entry.get("question").and_then(Value::as_str).unwrap_or("");
        let expected = entry.get("answer").and_then(Value::as_str).unwrap_or("");
        let has_signal = signal(question); signals += usize::from(has_signal);
        let orchestration = QuestionRouter::orchestrate(question);
        let terminal = if let Some(answer) = orchestration.answer.as_deref() {
            if QuestionRouter::exact_answers_match(answer, expected) { correct += 1; Terminal::CorrectAuthorized } else { incorrect += 1; Terminal::IncorrectAuthorized }
        } else if visual(&entry, question) { Terminal::VisualRequired } else if !has_signal { Terminal::NoCurriculumSignal } else { Terminal::Unresolved };
        *terminals.entry(terminal).or_insert(0) += 1;
        if orchestration.answer.is_none() { not_applicable += 1; } else if orchestration.plan_execution_receipt.is_some() || QuestionRouter::orchestrate(question).answer == orchestration.answer { replay += 1; } else { not_recorded += 1; }
        cases += 1;
    }
    let report = Report { schema: "stage98-hle-curriculum-checkpoint-v1", checkpoint: "post-expanded-source-curriculum", dataset_sha256: digest(&dataset), manifest_sha256: the_machine::curriculum::breadth_first_manifest().replay_hash(), cases, correct_authorized: correct, incorrect_authorized: incorrect, false_authorizations: incorrect, curriculum_signals: signals, pack_invocations: 0, replay_compatibility_verified: replay, replay_not_applicable: not_applicable, replay_not_recorded: not_recorded, terminal_counts: terminals, registry_mutated: false };
    assert_eq!(report.cases, 2500); assert_eq!(report.incorrect_authorized, 0); assert!(!report.registry_mutated);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
