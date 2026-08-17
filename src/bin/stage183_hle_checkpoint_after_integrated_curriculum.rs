//! Frozen HLE checkpoint after the integrated curriculum.
//!
//! This is diagnostic only: it evaluates the unchanged router against the
//! frozen HLE export, records per-question hashes and replay status, and does
//! not route curriculum packs into production.

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
const TRACE: &str = "/tmp/stage183_hle_checkpoint.trace.jsonl";
const REPORT_JSON: &str = "docs/stage183_hle_checkpoint_after_integrated_curriculum.json";
const REPORT_MD: &str = "docs/stage183_hle_checkpoint_after_integrated_curriculum.md";
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
struct Report {
    schema: &'static str,
    checkpoint: &'static str,
    producer_commit: String,
    dataset: &'static str,
    dataset_sha256: String,
    manifest_before_sha256: String,
    manifest_after_sha256: String,
    registry_version: &'static str,
    registry_mutations: usize,
    cases: usize,
    correct_authorized_answers: usize,
    incorrect_authorized_answers: usize,
    false_authorizations: usize,
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
    report_sha256: String,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest<T: Serialize>(value: &T) -> String {
    digest_bytes(&serde_json::to_vec(value).expect("serializable checkpoint value"))
}

fn producer_commit() -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned())
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

fn curriculum_signals(question: &str) -> Vec<String> {
    let lower = question.to_ascii_lowercase();
    let groups: [(&str, &[&str]); 6] = [
        (
            "calculus",
            &["derivative", "integral", "limit", "continuous"],
        ),
        (
            "real_analysis",
            &["monotonic", "bounded on", "converges", "convergence"],
        ),
        (
            "linear_algebra",
            &["matrix", "eigenvalue", "eigenvector", "determinant"],
        ),
        (
            "probability",
            &[
                "probability",
                "random variable",
                "expectation",
                "distribution",
            ],
        ),
        ("graph_theory", &["graph", "vertex", "vertices", "edge"]),
        (
            "discrete_dynamics",
            &["recurrence", "random walk", "transition matrix", "iterates"],
        ),
    ];
    groups
        .iter()
        .filter(|(_, markers)| markers.iter().any(|marker| lower.contains(marker)))
        .map(|(name, _)| (*name).to_owned())
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
        | Some(AbstentionReason::PlanVerificationFailed)
        | Some(AbstentionReason::AnswerFormatFailed)
        | None => Terminal::AmbiguousOrUnresolved,
        Some(AbstentionReason::MissingAttachment) => Terminal::VisualInputRequired,
    }
}

fn replay_status(
    question: &str,
    orchestration: &the_machine::router::OrchestratedAnswer,
) -> &'static str {
    if orchestration.answer.is_none() {
        "not_applicable"
    } else if orchestration.plan_execution_receipt.is_some() {
        "native_verified"
    } else if QuestionRouter::orchestrate(question).answer == orchestration.answer {
        "compatibility_verified"
    } else {
        "not_recorded"
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dataset_bytes = fs::read(DATASET)?;
    let dataset_sha256 = digest_bytes(&dataset_bytes);
    let manifest_before_sha256 = the_machine::curriculum::breadth_first_manifest().replay_hash();
    let commit = producer_commit();
    let mut trace = File::create(TRACE)?;
    let mut terminal_counts = BTreeMap::new();
    let (mut cases, mut correct, mut incorrect, mut candidates, mut invocations) = (0, 0, 0, 0, 0);
    let (mut replay_verified, mut native, mut compatibility, mut not_applicable, mut not_recorded) =
        (0, 0, 0, 0, 0);
    let (mut total_ms, mut max_ms) = (0.0, 0.0_f64);

    for line in BufReader::new(File::open(DATASET)?).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: Value = serde_json::from_str(&line)?;
        let question = entry.get("question").and_then(Value::as_str).unwrap_or("");
        let expected = entry.get("answer").and_then(Value::as_str).unwrap_or("");
        let detected = curriculum_signals(question);
        candidates += usize::from(!detected.is_empty());
        let started = Instant::now();
        let orchestration = QuestionRouter::orchestrate(question);
        let elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
        let is_correct = orchestration
            .answer
            .as_deref()
            .is_some_and(|answer| QuestionRouter::exact_answers_match(answer, expected));
        let class = terminal(&entry, &orchestration, is_correct, !detected.is_empty());
        *terminal_counts.entry(class).or_insert(0) += 1;
        correct += usize::from(class == Terminal::CorrectAuthorizedAnswer);
        incorrect += usize::from(class == Terminal::IncorrectAuthorizedAnswer);
        let pack_invoked = orchestration.plan_execution_receipt.is_some();
        invocations += usize::from(pack_invoked);
        match replay_status(question, &orchestration) {
            "native_verified" => {
                replay_verified += 1;
                native += 1;
            }
            "compatibility_verified" => {
                replay_verified += 1;
                compatibility += 1;
            }
            "not_applicable" => not_applicable += 1,
            _ => not_recorded += 1,
        }
        total_ms += elapsed_ms;
        max_ms = max_ms.max(elapsed_ms);
        let receipt = json!({
            "question_id": entry.get("id").and_then(Value::as_str),
            "question_sha256": digest_bytes(question.as_bytes()),
            "expected_answer_sha256": digest_bytes(expected.as_bytes()),
            "answer_sha256": orchestration.answer.as_deref().map(|answer| digest_bytes(answer.as_bytes())),
            "terminal": class,
            "curriculum_signals": detected,
            "route": "shadow_only",
            "pack_invoked": pack_invoked,
            "attempts": orchestration.attempts,
            "abstention_reason": orchestration.abstention_reason.map(|reason| format!("{reason:?}")),
            "replay_result": replay_status(question, &orchestration),
            "registry_version": REGISTRY,
            "manifest_sha256": manifest_before_sha256,
            "execution_time_ms": elapsed_ms,
        });
        serde_json::to_writer(&mut trace, &receipt)?;
        writeln!(trace)?;
        cases += 1;
    }
    trace.flush()?;
    let manifest_after_sha256 = the_machine::curriculum::breadth_first_manifest().replay_hash();
    let trace_sha256 = digest_bytes(&fs::read(TRACE)?);
    let preliminary = json!({
        "schema": "stage183-hle-checkpoint-after-integrated-curriculum-v1",
        "checkpoint": "stage183-hle-checkpoint-after-integrated-curriculum",
        "producer_commit": commit,
        "dataset_sha256": dataset_sha256,
        "manifest_before_sha256": manifest_before_sha256,
        "manifest_after_sha256": manifest_after_sha256,
        "cases": cases,
        "correct_authorized_answers": correct,
        "incorrect_authorized_answers": incorrect,
        "false_authorizations": incorrect,
        "curriculum_candidates": candidates,
        "pack_invocations": invocations,
        "replay_verified": replay_verified,
        "replay_native_verified": native,
        "replay_compatibility_verified": compatibility,
        "replay_not_applicable": not_applicable,
        "replay_not_recorded": not_recorded,
        "terminal_counts": terminal_counts,
        "trace_sha256": trace_sha256,
    });
    let report_sha256 = digest(&preliminary);
    let report = Report {
        schema: "stage183-hle-checkpoint-after-integrated-curriculum-v1",
        checkpoint: "stage183-hle-checkpoint-after-integrated-curriculum",
        producer_commit: commit,
        dataset: DATASET,
        dataset_sha256,
        manifest_before_sha256: manifest_before_sha256.clone(),
        manifest_after_sha256: manifest_after_sha256.clone(),
        registry_version: REGISTRY,
        registry_mutations: 0,
        cases,
        correct_authorized_answers: correct,
        incorrect_authorized_answers: incorrect,
        false_authorizations: incorrect,
        curriculum_candidates: candidates,
        pack_invocations: invocations,
        replay_verified,
        replay_native_verified: native,
        replay_compatibility_verified: compatibility,
        replay_not_applicable: not_applicable,
        replay_not_recorded: not_recorded,
        total_execution_time_ms: total_ms,
        max_execution_time_ms: max_ms,
        terminal_counts,
        trace_path: TRACE,
        trace_sha256,
        report_sha256,
    };
    assert_eq!(cases, 2_500, "frozen HLE case count changed");
    assert_eq!(incorrect, 0, "incorrect authorization detected");
    assert_eq!(
        &manifest_before_sha256, &manifest_after_sha256,
        "curriculum manifest mutated"
    );
    assert_eq!(invocations, 0, "production/shadow pack invocation detected");
    assert_eq!(
        not_recorded, 0,
        "accepted answer lacked replay verification"
    );
    let serialized = serde_json::to_string_pretty(&report)?;
    fs::write(REPORT_JSON, format!("{serialized}\n"))?;
    let markdown =
        format!("# Stage 183: post-curriculum HLE checkpoint\n\n```json\n{serialized}\n```\n");
    fs::write(REPORT_MD, markdown)?;
    println!("{serialized}");
    Ok(())
}
