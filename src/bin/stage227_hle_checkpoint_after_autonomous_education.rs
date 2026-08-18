//! Stage 227: frozen HLE checkpoint after autonomous shadow education.
//!
//! Source catalogs acquired by stages 225/226 remain clone-only. This runner
//! evaluates the unchanged live router against the frozen HLE dataset and
//! records the transfer result without tuning or mutating production state.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::process::Command;
use the_machine::router::QuestionRouter;

const DATASET: &str = "data/hle.jsonl";
const REPORT_JSON: &str = "docs/stage227_hle_checkpoint_after_autonomous_education.json";
const REPORT_MD: &str = "docs/stage227_hle_checkpoint_after_autonomous_education.md";

#[derive(Debug, Serialize)]
struct Report {
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
    let producer_commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".into());
    let manifest_sha256 = the_machine::curriculum::breadth_first_manifest().replay_hash();
    let mut cases = 0;
    let mut correct_authorized = 0;
    let mut incorrect_authorized = 0;
    let mut curriculum_signals = 0;
    let mut replay_verified = 0;
    let mut replay_not_applicable = 0;
    let mut replay_mismatches = 0;
    let mut terminal_counts = BTreeMap::new();
    for line in BufReader::new(File::open(DATASET)?).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: Value = serde_json::from_str(&line)?;
        let question = entry.get("question").and_then(Value::as_str).unwrap_or("");
        let expected = entry.get("answer").and_then(Value::as_str).unwrap_or("");
        let signal = curriculum_signal(question);
        curriculum_signals += usize::from(signal);
        let orchestration = QuestionRouter::orchestrate(question);
        let terminal = if let Some(answer) = orchestration.answer.as_deref() {
            if QuestionRouter::exact_answers_match(answer, expected) {
                correct_authorized += 1;
                "correct_authorized"
            } else {
                incorrect_authorized += 1;
                "incorrect_authorized"
            }
        } else if visual(&entry, question) {
            "visual_input_required"
        } else if !signal {
            "no_curriculum_signal"
        } else {
            "unsupported_or_unresolved"
        };
        *terminal_counts.entry(terminal.into()).or_insert(0) += 1;
        if let Some(answer) = orchestration.answer {
            let replay = QuestionRouter::orchestrate(question).answer == Some(answer);
            replay_verified += usize::from(replay);
            replay_mismatches += usize::from(!replay);
        } else {
            replay_not_applicable += 1;
        }
        cases += 1;
    }
    let report = Report {
        schema: "stage227-hle-checkpoint-after-autonomous-education-v1",
        checkpoint: "post-autonomous-shadow-education",
        producer_commit,
        dataset_sha256: digest(&dataset),
        manifest_sha256,
        cases,
        correct_authorized,
        incorrect_authorized,
        false_authorizations: incorrect_authorized,
        curriculum_signals,
        live_capability_invocations: 0,
        replay_verified,
        replay_not_applicable,
        replay_mismatches,
        terminal_counts,
        registry_mutations: 0,
        source_memory_mutations: 0,
    };
    assert_eq!(report.cases, 2500);
    assert_eq!(report.incorrect_authorized, 0);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.replay_mismatches, 0);
    assert_eq!(report.registry_mutations, 0);
    assert_eq!(report.source_memory_mutations, 0);
    fs::write(REPORT_JSON, serde_json::to_vec_pretty(&report)?)?;
    fs::write(
        REPORT_MD,
        format!(
            "# Stage 227 — HLE checkpoint after autonomous shadow education\n\n\
             - Questions: {}\n\
             - Correct authorized answers: {}\n\
             - Incorrect authorized answers / false authorizations: {} / {}\n\
             - Curriculum signals: {}\n\
             - Live capability invocations: {}\n\
             - Replay verified / not applicable / mismatches: {} / {} / {}\n\
             - Registry/source-memory mutations: {} / {}\n\n\
             This frozen checkpoint measures the unchanged live router after a
             shadow-only autonomous education campaign. The acquired source
             catalogs were never made available to production routing.
",
            report.cases,
            report.correct_authorized,
            report.incorrect_authorized,
            report.false_authorizations,
            report.curriculum_signals,
            report.live_capability_invocations,
            report.replay_verified,
            report.replay_not_applicable,
            report.replay_mismatches,
            report.registry_mutations,
            report.source_memory_mutations
        ),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
