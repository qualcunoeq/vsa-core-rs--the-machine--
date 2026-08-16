//! Frozen post-curriculum HLE diagnostic checkpoint.
//!
//! The HLE export is evaluated without changing implementation, curriculum,
//! or routing.  This binary records the first terminal obstruction and a
//! replay receipt for every question; curriculum packs remain shadow-only.

use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::process::Command;
use std::time::Instant;
use the_machine::router::{AbstentionReason, QuestionRouter};

const DATASET: &str = "data/hle.jsonl";
const TRACE: &str = "/tmp/hle_curriculum_checkpoint_2.jsonl";
const SUMMARY: &str = "docs/stage_l_hle_checkpoint_2.json";
const REGISTRY: &str = "shadow-only-no-production-mutation";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Terminal {
    CorrectAuthorizedAnswer,
    IncorrectAuthorizedAnswer,
    VisualInputRequired,
    NoCurriculumSignal,
    LanguageNormalizationFailure,
    MissingFactualKnowledge,
    MissingReasoningMethod,
    UnsupportedTarget,
    AmbiguousOrUnresolved,
}

#[derive(Debug, Serialize)]
struct SummaryReport {
    schema: &'static str,
    checkpoint: &'static str,
    producer_commit: String,
    dataset: &'static str,
    dataset_sha256: String,
    manifest_sha256: String,
    registry_version: &'static str,
    cases: usize,
    correct_authorized_answers: usize,
    incorrect_authorized_answers: usize,
    false_authorizations: usize,
    safely_formalized_but_unsupported: usize,
    curriculum_candidates: usize,
    pack_invocations: usize,
    replay_verified: usize,
    replay_native_verified: usize,
    replay_compatibility_verified: usize,
    replay_not_applicable: usize,
    replay_not_recorded: usize,
    total_execution_time_ms: f64,
    max_execution_time_ms: f64,
    terminal_counts: BTreeMap<Terminal, usize>,
    trace_path: &'static str,
    trace_sha256: String,
    summary_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    digest_bytes(&serde_json::to_vec(value).expect("checkpoint serializes"))
}

fn visual(entry: &Value, question: &str) -> bool {
    let has_image = entry
        .get("has_image")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    has_image
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

fn signals(question: &str) -> Vec<String> {
    let lower = question.to_ascii_lowercase();
    let groups = [
        (
            "calculus",
            ["derivative", "integral", "limit", "continuous"] as [&str; 4],
        ),
        (
            "real_analysis",
            ["monotonic", "bounded on", "converges", "convergence"],
        ),
        (
            "linear_algebra",
            ["matrix", "eigenvalue", "eigenvector", "determinant"],
        ),
        (
            "probability",
            [
                "probability",
                "random variable",
                "expectation",
                "distribution",
            ],
        ),
        ("graph_theory", ["graph", "vertex", "vertices", "edge"]),
        (
            "discrete_dynamics",
            ["recurrence", "random walk", "transition matrix", "iterates"],
        ),
    ];
    groups
        .iter()
        .filter(|(_, markers)| markers.iter().any(|marker| lower.contains(marker)))
        .map(|(name, _)| (*name).into())
        .collect()
}

fn terminal(
    entry: &Value,
    orchestration: &the_machine::router::OrchestratedAnswer,
    correct: bool,
    has_signal: bool,
) -> Terminal {
    if orchestration.answer.is_some() {
        return if correct {
            Terminal::CorrectAuthorizedAnswer
        } else {
            Terminal::IncorrectAuthorizedAnswer
        };
    }
    let question = entry.get("question").and_then(Value::as_str).unwrap_or("");
    if visual(entry, question) {
        return Terminal::VisualInputRequired;
    }
    if !has_signal {
        return Terminal::NoCurriculumSignal;
    }
    match orchestration.abstention_reason {
        Some(AbstentionReason::ProblemParseFailed)
        | Some(AbstentionReason::TargetNotIdentified)
        | Some(AbstentionReason::SymbolBindingFailed) => Terminal::LanguageNormalizationFailure,
        Some(AbstentionReason::InsufficientEvidence) => Terminal::MissingFactualKnowledge,
        Some(AbstentionReason::NoApplicableMethod) | Some(AbstentionReason::VerificationFailed) => {
            Terminal::MissingReasoningMethod
        }
        Some(AbstentionReason::UnsupportedDomain)
        | Some(AbstentionReason::ConflictingPlans)
        | Some(AbstentionReason::MultipleUnresolvedMethods) => Terminal::UnsupportedTarget,
        Some(AbstentionReason::MissingRequiredGiven)
        | Some(AbstentionReason::RequiredAssumptionMissing)
        | Some(AbstentionReason::RequiredAssumptionContradicted)
        | Some(AbstentionReason::SolverUnsupportedOperation)
        | Some(AbstentionReason::IntermediateNotDerivable)
        | Some(AbstentionReason::IntermediateSemanticMismatch)
        | Some(AbstentionReason::IntermediateValueKindMismatch)
        | Some(AbstentionReason::IntermediateQualifierMismatch)
        | Some(AbstentionReason::IntermediateConstraintConflict)
        | Some(AbstentionReason::PlanCycleDetected)
        | Some(AbstentionReason::PlanDepthExceeded)
        | Some(AbstentionReason::PlanExecutionFailed)
        | Some(AbstentionReason::PlanVerificationFailed) => Terminal::AmbiguousOrUnresolved,
        Some(AbstentionReason::AnswerFormatFailed) => Terminal::AmbiguousOrUnresolved,
        Some(AbstentionReason::MissingAttachment) => Terminal::VisualInputRequired,
        None => Terminal::AmbiguousOrUnresolved,
    }
}

fn replay_status(
    question: &str,
    orchestration: &the_machine::router::OrchestratedAnswer,
) -> &'static str {
    if orchestration.answer.is_none() {
        "not_applicable"
    } else if orchestration.plan_execution_receipt.is_some() {
        "verified"
    } else {
        let replay = QuestionRouter::orchestrate(question);
        if replay.answer == orchestration.answer {
            "compatibility_verified"
        } else {
            "not_recorded"
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset_bytes = fs::read(DATASET)?;
    let dataset_sha256 = digest_bytes(&dataset_bytes);
    let manifest_sha256 = the_machine::curriculum::breadth_first_manifest().replay_hash();
    let producer_commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    let mut trace = File::create(TRACE)?;
    let mut terminal_counts = BTreeMap::new();
    let mut cases = 0;
    let mut correct = 0;
    let mut incorrect = 0;
    let mut candidates = 0;
    let mut replay_verified = 0;
    let mut replay_native_verified = 0;
    let mut replay_compatibility_verified = 0;
    let mut replay_not_applicable = 0;
    let mut replay_not_recorded = 0;
    let mut total_ms = 0.0;
    let mut max_ms: f64 = 0.0;
    for line in BufReader::new(File::open(DATASET)?).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: Value = serde_json::from_str(&line)?;
        let question = entry.get("question").and_then(Value::as_str).unwrap_or("");
        let expected = entry.get("answer").and_then(Value::as_str).unwrap_or("");
        let detected = signals(question);
        candidates += usize::from(!detected.is_empty());
        let started = Instant::now();
        let orchestration = QuestionRouter::orchestrate(question);
        let elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
        let is_correct = orchestration
            .answer
            .as_deref()
            .is_some_and(|answer| QuestionRouter::exact_answers_match(answer, expected));
        let classification = terminal(&entry, &orchestration, is_correct, !detected.is_empty());
        *terminal_counts.entry(classification).or_insert(0) += 1;
        if classification == Terminal::CorrectAuthorizedAnswer {
            correct += 1;
        }
        if classification == Terminal::IncorrectAuthorizedAnswer {
            incorrect += 1;
        }
        let replay_result = replay_status(question, &orchestration);
        match replay_result {
            "verified" => {
                replay_verified += 1;
                replay_native_verified += 1;
            }
            "compatibility_verified" => {
                replay_verified += 1;
                replay_compatibility_verified += 1;
            }
            "not_applicable" => replay_not_applicable += 1,
            _ => replay_not_recorded += 1,
        }
        total_ms += elapsed_ms;
        max_ms = max_ms.max(elapsed_ms);
        let receipt = json!({
            "question_id": entry.get("id").and_then(Value::as_str),
            "question_sha256": digest_bytes(question.as_bytes()),
            "expected_answer_sha256": digest_bytes(expected.as_bytes()),
            "terminal": classification,
            "curriculum_signals": detected,
            "curriculum_route": "shadow_only",
            "pack_invoked": false,
            "answer": orchestration.answer,
            "attempts": orchestration.attempts,
            "abstention_reason": orchestration.abstention_reason.map(|reason| format!("{reason:?}")),
            "replay_result": replay_result,
            "registry_version": REGISTRY,
            "manifest_sha256": manifest_sha256,
            "execution_time_ms": elapsed_ms,
        });
        serde_json::to_writer(&mut trace, &receipt)?;
        writeln!(trace)?;
        cases += 1;
    }
    trace.flush()?;
    let trace_sha256 = digest_bytes(&fs::read(TRACE)?);
    let summary_without_hash = json!({
        "checkpoint": "stage-l-hle-checkpoint-2",
        "producer_commit": producer_commit,
        "dataset_sha256": dataset_sha256,
        "manifest_sha256": manifest_sha256,
        "cases": cases,
        "correct_authorized_answers": correct,
        "incorrect_authorized_answers": incorrect,
        "false_authorizations": incorrect,
        "curriculum_candidates": candidates,
        "pack_invocations": 0,
        "replay_verified": replay_verified,
        "replay_native_verified": replay_native_verified,
        "replay_compatibility_verified": replay_compatibility_verified,
        "replay_not_applicable": replay_not_applicable,
        "replay_not_recorded": replay_not_recorded,
        "terminal_counts": terminal_counts,
        "trace_sha256": trace_sha256,
    });
    let summary_sha256 = digest(&summary_without_hash);
    let report = SummaryReport {
        schema: "stage-l-hle-checkpoint-2-v1",
        checkpoint: "stage-l-hle-checkpoint-2",
        producer_commit,
        dataset: DATASET,
        dataset_sha256,
        manifest_sha256,
        registry_version: REGISTRY,
        cases,
        correct_authorized_answers: correct,
        incorrect_authorized_answers: incorrect,
        false_authorizations: incorrect,
        safely_formalized_but_unsupported: 0,
        curriculum_candidates: candidates,
        pack_invocations: 0,
        replay_verified,
        replay_native_verified,
        replay_compatibility_verified,
        replay_not_applicable,
        replay_not_recorded,
        total_execution_time_ms: total_ms,
        max_execution_time_ms: max_ms,
        terminal_counts,
        trace_path: TRACE,
        trace_sha256,
        summary_sha256,
    };
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(SUMMARY, format!("{serialized}\n"))?;
    println!("{serialized}");
    Ok(())
}
