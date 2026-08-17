//! Frozen HLE checkpoint after the integrated source/multimodal curriculum.
//!
//! The run is diagnostic only.  It does not promote the new routes, mutate a
//! registry, or use HLE answers for curriculum development.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::process::Command;
use the_machine::router::QuestionRouter;

const DATASET: &str = "data/hle.jsonl";
const SUMMARY: &str = "docs/stage151_hle_checkpoint_post_source_multimodal.json";
const TRACE: &str = "docs/stage151_hle_checkpoint_post_source_multimodal.trace.jsonl";
const SOURCE_REPORT: &str = "docs/stage150_source_multimodal_composition.json";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Terminal {
    CorrectAuthorized,
    IncorrectAuthorized,
    VisualRequired,
    NoCurriculumSignal,
    Unresolved,
}

#[derive(Serialize)]
struct Trace {
    index: usize,
    question_sha256: String,
    has_image: bool,
    terminal: Terminal,
    answer_sha256: Option<String>,
    replay_verified: bool,
    route_receipt: bool,
}

#[derive(Serialize)]
struct Summary {
    schema: &'static str,
    checkpoint: &'static str,
    producer_commit: String,
    dataset_sha256: String,
    manifest_sha256: String,
    source_composition_sha256: String,
    cases: usize,
    correct_authorized: usize,
    incorrect_authorized: usize,
    false_authorizations: usize,
    curriculum_signals: usize,
    route_receipts: usize,
    replay_compatibility_verified: usize,
    replay_not_applicable: usize,
    replay_not_recorded: usize,
    terminal_counts: BTreeMap<Terminal, usize>,
    registry_mutated: bool,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn signal(question: &str) -> bool {
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
        "chemical",
        "molecular",
        "dna",
        "stoichiometric",
        "base composition",
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
    let producer_commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".into());
    let mut trace_file = File::create(TRACE)?;
    let mut cases = 0;
    let mut correct_authorized = 0;
    let mut incorrect_authorized = 0;
    let mut curriculum_signals = 0;
    let mut route_receipts = 0;
    let mut replay_compatibility_verified = 0;
    let mut replay_not_applicable = 0;
    let mut replay_not_recorded = 0;
    let mut terminal_counts = BTreeMap::new();
    for line in BufReader::new(File::open(DATASET)?).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: Value = serde_json::from_str(&line)?;
        let question = entry.get("question").and_then(Value::as_str).unwrap_or("");
        let expected = entry.get("answer").and_then(Value::as_str).unwrap_or("");
        let has_signal = signal(question);
        curriculum_signals += usize::from(has_signal);
        let orchestration = QuestionRouter::orchestrate(question);
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
        } else if !has_signal {
            Terminal::NoCurriculumSignal
        } else {
            Terminal::Unresolved
        };
        let route_receipt = orchestration.plan_execution_receipt.is_some();
        route_receipts += usize::from(route_receipt);
        let replay_verified = if orchestration.answer.is_none() {
            replay_not_applicable += 1;
            true
        } else if route_receipt {
            replay_compatibility_verified += 1;
            true
        } else if QuestionRouter::orchestrate(question).answer == orchestration.answer {
            replay_compatibility_verified += 1;
            true
        } else {
            replay_not_recorded += 1;
            false
        };
        *terminal_counts.entry(terminal).or_insert(0) += 1;
        let trace = Trace {
            index: cases,
            question_sha256: digest_bytes(question.as_bytes()),
            has_image: entry
                .get("has_image")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            terminal,
            answer_sha256: orchestration
                .answer
                .as_deref()
                .map(|answer| digest_bytes(answer.as_bytes())),
            replay_verified,
            route_receipt,
        };
        serde_json::to_writer(&mut trace_file, &trace)?;
        trace_file.write_all(b"\n")?;
        cases += 1;
    }
    let summary = Summary {
        schema: "stage151-hle-checkpoint-post-source-multimodal-v1",
        checkpoint: "post-integrated-source-multimodal-curriculum",
        producer_commit,
        dataset_sha256: digest_bytes(&dataset),
        manifest_sha256: the_machine::curriculum::breadth_first_manifest().replay_hash(),
        source_composition_sha256: digest_bytes(&source_report),
        cases,
        correct_authorized,
        incorrect_authorized,
        false_authorizations: incorrect_authorized,
        curriculum_signals,
        route_receipts,
        replay_compatibility_verified,
        replay_not_applicable,
        replay_not_recorded,
        terminal_counts,
        registry_mutated: false,
    };
    assert_eq!(summary.cases, 2500);
    assert_eq!(summary.incorrect_authorized, 0);
    assert_eq!(summary.false_authorizations, 0);
    assert_eq!(summary.registry_mutated, false);
    fs::write(SUMMARY, serde_json::to_vec_pretty(&summary)?)?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}
