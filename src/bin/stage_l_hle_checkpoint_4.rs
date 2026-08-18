//! Stage L checkpoint 4: full current-curriculum HLE diagnostic.
//!
//! This evaluator runs the existing router without changing its route set,
//! curriculum manifest, registry, or authorization policy.  It records a
//! per-question trace so that the post-Stage-300 result is reproducible and
//! distinguishable from the bounded shadow-only checkpoint.

use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::process::Command;
use std::time::Instant;
use the_machine::router::{AbstentionReason, OrchestratedAnswer, QuestionRouter};

const DATASET: &str = "data/hle.jsonl";
const SUMMARY: &str = "docs/stage_l_hle_checkpoint_4.json";
const TRACE: &str = "/tmp/hle_stage_l_checkpoint_4.trace.jsonl";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Terminal {
    CorrectAuthorizedAnswer,
    IncorrectAuthorizedAnswer,
    VisualInputRequired,
    LanguageNormalizationFailure,
    MissingFactualKnowledge,
    MissingReasoningMethod,
    UnsupportedOrAmbiguous,
    NoCurriculumSignal,
}

#[derive(Debug, Serialize)]
struct TraceRecord {
    index: usize,
    question_sha256: String,
    category: String,
    terminal: Terminal,
    answer: Option<String>,
    exact_reference_match: bool,
    route: String,
    route_trace: Vec<String>,
    required_capabilities: Vec<String>,
    abstention_reason: Option<String>,
    verification: String,
    receipt: Value,
    replay_result: String,
    execution_time_ms: f64,
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
    correct_authorized_answers: usize,
    incorrect_authorized_answers: usize,
    false_authorizations: usize,
    terminal_counts: BTreeMap<Terminal, usize>,
    route_counts: BTreeMap<String, usize>,
    pack_receipt_invocations: usize,
    replay_verified: usize,
    replay_not_applicable: usize,
    replay_failed: usize,
    total_execution_time_ms: f64,
    max_execution_time_ms: f64,
    registry_mutated: bool,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn visual(entry: &Value, question: &str) -> bool {
    let has_image = entry
        .get("has_image")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    has_image
        && [
            "attached image",
            "image shows",
            "shown in the image",
            "figure",
            "diagram",
            "pictured",
            "graph shows",
            "chart shows",
        ]
        .iter()
        .any(|marker| question.to_ascii_lowercase().contains(marker))
}

fn terminal(entry: &Value, orchestration: &OrchestratedAnswer, correct: bool) -> Terminal {
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
    match orchestration.abstention_reason {
        Some(AbstentionReason::ProblemParseFailed)
        | Some(AbstentionReason::TargetNotIdentified)
        | Some(AbstentionReason::SymbolBindingFailed) => Terminal::LanguageNormalizationFailure,
        Some(AbstentionReason::InsufficientEvidence) => Terminal::MissingFactualKnowledge,
        Some(AbstentionReason::NoApplicableMethod)
        | Some(AbstentionReason::VerificationFailed) => Terminal::MissingReasoningMethod,
        Some(AbstentionReason::UnsupportedDomain)
        | Some(AbstentionReason::SolverUnsupportedOperation)
        | Some(AbstentionReason::MissingRequiredGiven)
        | Some(AbstentionReason::RequiredAssumptionMissing)
        | Some(AbstentionReason::RequiredAssumptionContradicted)
        | Some(AbstentionReason::MultipleUnresolvedMethods)
        | Some(AbstentionReason::ConflictingPlans)
        | Some(AbstentionReason::AnswerFormatFailed)
        | Some(AbstentionReason::IntermediateNotDerivable)
        | Some(AbstentionReason::IntermediateSemanticMismatch)
        | Some(AbstentionReason::IntermediateValueKindMismatch)
        | Some(AbstentionReason::IntermediateQualifierMismatch)
        | Some(AbstentionReason::IntermediateConstraintConflict)
        | Some(AbstentionReason::PlanCycleDetected)
        | Some(AbstentionReason::PlanDepthExceeded)
        | Some(AbstentionReason::PlanExecutionFailed)
        | Some(AbstentionReason::PlanVerificationFailed)
        | Some(AbstentionReason::MissingAttachment)
        | None => Terminal::UnsupportedOrAmbiguous,
    }
}

fn replay(question: &str, orchestration: &OrchestratedAnswer) -> &'static str {
    if orchestration.answer.is_none() {
        return "not_applicable";
    }
    if orchestration
        .plan_execution_receipt
        .as_ref()
        .is_some_and(|receipt| receipt.final_verification.passed)
    {
        return "verified";
    }
    let rerun = QuestionRouter::orchestrate(question);
    if rerun.answer == orchestration.answer
        && rerun.evidence == orchestration.evidence
        && rerun.verification == orchestration.verification
    {
        "verified"
    } else {
        "failed"
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
    let manifest_before = the_machine::curriculum::breadth_first_manifest().replay_hash();
    let trace_path = env::var("MACHINE_HLE_TRACE").unwrap_or_else(|_| TRACE.into());
    let mut trace = File::create(&trace_path)?;
    let mut cases = 0;
    let mut correct = 0;
    let mut incorrect = 0;
    let mut replay_verified = 0;
    let mut replay_not_applicable = 0;
    let mut replay_failed = 0;
    let mut total_ms = 0.0;
    let mut max_ms: f64 = 0.0;
    let mut terminal_counts = BTreeMap::new();
    let mut route_counts = BTreeMap::new();
    let mut pack_receipt_invocations = 0;

    for (index, line) in BufReader::new(File::open(DATASET)?).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: Value = serde_json::from_str(&line)?;
        let question = entry.get("question").and_then(Value::as_str).unwrap_or("");
        let expected = entry.get("answer").and_then(Value::as_str).unwrap_or("");
        let started = Instant::now();
        let orchestration = QuestionRouter::orchestrate(question);
        let elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
        let correct_answer = orchestration
            .answer
            .as_deref()
            .is_some_and(|answer| QuestionRouter::exact_answers_match(answer, expected));
        let class = terminal(&entry, &orchestration, correct_answer);
        let replay_result = replay(question, &orchestration);
        let route = format!("{:?}", orchestration.plan.domain);
        *terminal_counts.entry(class).or_insert(0) += 1;
        *route_counts.entry(route.clone()).or_insert(0) += 1;
        correct += usize::from(correct_answer);
        incorrect += usize::from(orchestration.answer.is_some() && !correct_answer);
        pack_receipt_invocations += usize::from(orchestration.plan_execution_receipt.is_some());
        match replay_result {
            "verified" => replay_verified += 1,
            "not_applicable" => replay_not_applicable += 1,
            _ => replay_failed += 1,
        }
        total_ms += elapsed_ms;
        max_ms = max_ms.max(elapsed_ms);
        let capabilities = orchestration
            .plan
            .required_capabilities
            .iter()
            .map(|capability| format!("{:?}", capability))
            .collect::<Vec<_>>();
        let receipt = json!({
            "domain": format!("{:?}", orchestration.plan.domain),
            "verification": orchestration.verification,
            "abstention_reason": orchestration.abstention_reason.map(|reason| format!("{reason:?}")),
            "execution_receipt": orchestration.execution_receipt,
            "plan_execution_receipt": orchestration.plan_execution_receipt,
            "rejected_candidates": orchestration.rejected_candidates,
        });
        serde_json::to_writer(
            &mut trace,
            &TraceRecord {
                index,
                question_sha256: digest_bytes(question.as_bytes()),
                category: entry
                    .get("category")
                    .and_then(Value::as_str)
                    .unwrap_or("uncategorized")
                    .into(),
                terminal: class,
                answer: orchestration.answer.clone(),
                exact_reference_match: correct_answer,
                route_trace: orchestration.attempts.clone(),
                route,
                required_capabilities: capabilities,
                abstention_reason: orchestration
                    .abstention_reason
                    .map(|reason| format!("{reason:?}")),
                verification: orchestration.verification.clone(),
                receipt,
                replay_result: replay_result.into(),
                execution_time_ms: elapsed_ms,
            },
        )?;
        writeln!(trace)?;
        cases += 1;
    }
    drop(trace);
    let manifest_after = the_machine::curriculum::breadth_first_manifest().replay_hash();
    let report = Summary {
        schema: "stage-l-hle-checkpoint-4-v1",
        checkpoint: "post-stage300-multimodal-curriculum",
        producer_commit,
        dataset_sha256: digest_bytes(&dataset),
        manifest_sha256: manifest_after.clone(),
        trace_sha256: digest_bytes(&fs::read(&trace_path)?),
        cases,
        correct_authorized_answers: correct,
        incorrect_authorized_answers: incorrect,
        false_authorizations: incorrect,
        terminal_counts,
        route_counts,
        pack_receipt_invocations,
        replay_verified,
        replay_not_applicable,
        replay_failed,
        total_execution_time_ms: total_ms,
        max_execution_time_ms: max_ms,
        registry_mutated: manifest_before != manifest_after,
    };
    assert_eq!(report.cases, 2500);
    assert_eq!(report.incorrect_authorized_answers, 0);
    assert_eq!(report.false_authorizations, 0);
    assert_eq!(report.registry_mutated, false);
    fs::write(SUMMARY, serde_json::to_vec_pretty(&report)?)?;
    fs::write(
        "docs/stage_l_hle_checkpoint_4.md",
        format!(
            "# Stage L — post-Stage-300 HLE checkpoint\n\n- Questions: {}\n- Correct authorized answers: {}\n- Incorrect authorized answers / false authorizations: {} / {}\n- Pack receipts: {}\n- Replay verified / not applicable / failed: {} / {} / {}\n- Trace SHA-256: `{}`\n- Dataset SHA-256: `{}`\n- Manifest SHA-256: `{}`\n- Registry mutation: {}\n\nThis is a frozen, full-orchestration diagnostic. It records route traces and receipts but does not expose HLE answers to curriculum acquisition or mutate production routing.\n",
            report.cases,
            report.correct_authorized_answers,
            report.incorrect_authorized_answers,
            report.false_authorizations,
            report.pack_receipt_invocations,
            report.replay_verified,
            report.replay_not_applicable,
            report.replay_failed,
            report.trace_sha256,
            report.dataset_sha256,
            report.manifest_sha256,
            report.registry_mutated,
        ),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
