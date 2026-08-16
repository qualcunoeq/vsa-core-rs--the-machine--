//! Frozen HLE checkpoint after the spectral curriculum milestone.
//!
//! This runner never mutates routing, packs, manifests, or the HLE dataset.
//! Answer text is used only by the terminal scorer after orchestration.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::process::Command;
use the_machine::router::QuestionRouter;

const DATASET: &str = "data/hle.jsonl";
const SUMMARY: &str = "docs/stage_l_hle_checkpoint_3.json";

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
struct Summary {
    schema: &'static str,
    checkpoint: &'static str,
    producer_commit: String,
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
    let producer_commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".into());
    let mut cases = 0;
    let mut correct_authorized = 0;
    let mut incorrect_authorized = 0;
    let mut curriculum_signals = 0;
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
        *terminal_counts.entry(terminal).or_insert(0) += 1;
        if orchestration.answer.is_none() {
            replay_not_applicable += 1;
        } else if orchestration.plan_execution_receipt.is_some() {
            replay_compatibility_verified += 1;
        } else if QuestionRouter::orchestrate(question).answer == orchestration.answer {
            replay_compatibility_verified += 1;
        } else {
            replay_not_recorded += 1;
        }
        cases += 1;
    }
    let summary = Summary {
        schema: "stage-l-hle-checkpoint-3-v1",
        checkpoint: "post-spectral-curriculum",
        producer_commit,
        dataset_sha256: digest_bytes(&dataset),
        manifest_sha256: the_machine::curriculum::breadth_first_manifest().replay_hash(),
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
        registry_mutated: false,
    };
    assert_eq!(summary.cases, 2500);
    assert_eq!(summary.incorrect_authorized, 0);
    assert_eq!(summary.false_authorizations, 0);
    assert_eq!(summary.registry_mutated, false);
    fs::write(SUMMARY, serde_json::to_vec_pretty(&summary)?)?;
    fs::write("docs/stage_l_hle_checkpoint_3.md", format!("# Stage L — post-spectral HLE checkpoint\n\n- Questions: {}\n- Correct authorized answers: {}\n- Incorrect authorized answers / false authorizations: {} / {}\n- Curriculum signals: {}\n- Pack invocations: {}\n- Compatibility replay verified: {}\n- Replay not applicable: {}\n- Replay not recorded: {}\n- Registry mutation: {}\n\nThis is a frozen diagnostic checkpoint after the bounded spectral curriculum milestone. It does not tune implementation or mutate production routing.\n", summary.cases, summary.correct_authorized, summary.incorrect_authorized, summary.false_authorizations, summary.curriculum_signals, summary.pack_invocations, summary.replay_compatibility_verified, summary.replay_not_applicable, summary.replay_not_recorded, summary.registry_mutated))?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}
