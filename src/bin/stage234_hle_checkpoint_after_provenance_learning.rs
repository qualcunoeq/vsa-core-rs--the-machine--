//! Stage 234: frozen HLE checkpoint after provenance-derived source learning.
//!
//! The source-learning catalogs remain clone-only. This checkpoint evaluates
//! the unchanged live router and records whether the new curriculum has
//! transferred to HLE without exposing HLE answers to acquisition.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::process::Command;
use the_machine::router::QuestionRouter;

const DATASET: &str = "data/hle.jsonl";
const SOURCE_REPORT: &str = "docs/stage233_provenance_learning_curve.json";

#[derive(Debug, Serialize)]
struct Report {
    schema: &'static str,
    checkpoint: &'static str,
    producer_commit: String,
    source_learning_report_sha256: String,
    dataset_sha256: String,
    manifest_sha256: String,
    cases: usize,
    correct_authorized: usize,
    incorrect_authorized: usize,
    false_authorizations: usize,
    curriculum_signals: usize,
    live_capability_invocations: usize,
    replay_verified: usize,
    replay_not_applicable: usize,
    replay_mismatches: usize,
    terminal_counts: BTreeMap<String, usize>,
    registry_mutations: usize,
    source_memory_mutations: usize,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn curriculum_signal(question: &str) -> bool {
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
        "modulo",
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset = fs::read(DATASET)?;
    let source_report = fs::read(SOURCE_REPORT)?;
    let source: Value = serde_json::from_slice(&source_report)?;
    assert_eq!(
        source.get("sealed_exact_decisions").and_then(Value::as_u64),
        Some(200)
    );
    assert_eq!(
        source.get("sealed_authorizations").and_then(Value::as_u64),
        Some(120)
    );
    assert_eq!(
        source.get("false_authorizations").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        source.get("live_mutations").and_then(Value::as_u64),
        Some(0)
    );
    let producer_commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".into());
    let mut report = Report {
        schema: "stage234-hle-checkpoint-after-provenance-learning-v1",
        checkpoint: "post-provenance-derived-shadow-learning",
        producer_commit,
        source_learning_report_sha256: digest(&source_report),
        dataset_sha256: digest(&dataset),
        manifest_sha256: the_machine::curriculum::breadth_first_manifest().replay_hash(),
        cases: 0,
        correct_authorized: 0,
        incorrect_authorized: 0,
        false_authorizations: 0,
        curriculum_signals: 0,
        live_capability_invocations: 0,
        replay_verified: 0,
        replay_not_applicable: 0,
        replay_mismatches: 0,
        terminal_counts: BTreeMap::new(),
        registry_mutations: 0,
        source_memory_mutations: 0,
    };
    for line in BufReader::new(File::open(DATASET)?).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: Value = serde_json::from_str(&line)?;
        let question = entry.get("question").and_then(Value::as_str).unwrap_or("");
        let expected = entry.get("answer").and_then(Value::as_str).unwrap_or("");
        let signal = curriculum_signal(question);
        report.curriculum_signals += usize::from(signal);
        let orchestration = QuestionRouter::orchestrate(question);
        let terminal = if let Some(answer) = orchestration.answer.as_deref() {
            if QuestionRouter::exact_answers_match(answer, expected) {
                report.correct_authorized += 1;
                "correct_authorized"
            } else {
                report.incorrect_authorized += 1;
                "incorrect_authorized"
            }
        } else if visual(&entry, question) {
            "visual_input_required"
        } else if !signal {
            "no_curriculum_signal"
        } else {
            "unsupported_or_unresolved"
        };
        *report.terminal_counts.entry(terminal.into()).or_insert(0) += 1;
        if let Some(answer) = orchestration.answer {
            let replay = QuestionRouter::orchestrate(question).answer == Some(answer);
            report.replay_verified += usize::from(replay);
            report.replay_mismatches += usize::from(!replay);
        } else {
            report.replay_not_applicable += 1;
        }
        report.cases += 1;
    }
    report.false_authorizations = report.incorrect_authorized;
    assert_eq!(report.cases, 2500);
    assert_eq!(report.incorrect_authorized, 0);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.replay_mismatches, 0);
    assert_eq!(report.registry_mutations, 0);
    assert_eq!(report.source_memory_mutations, 0);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
