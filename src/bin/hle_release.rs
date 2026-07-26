//! Frozen HLE release-candidate evaluation.
//!
//! This binary is deliberately an evaluator, not a capability or routing
//! change.  It runs the checked-in release candidate at commit 2147e9e over
//! the complete HLE export and emits one auditable terminal classification per
//! question.  The evaluator never writes to the registry, ontology, or
//! regression corpus.

use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;
use the_machine::router::{AbstentionReason, OrchestratedAnswer, QuestionRouter, Tool};

const FROZEN_COMMIT: &str = "2147e9e";
const DATASET: &str = "data/hle.jsonl";
const REGISTRY_VERSION: &str = "machine-release-candidate-19";
const ONTOLOGY_VERSION: &str = "ontology-phases-11-17";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum TerminalClassification {
    CorrectAuthorizedAnswer,
    IncorrectAuthorizedAnswer,
    SafelyFormalizedButUnsupported,
    MissingFactualKnowledge,
    MissingReasoningMethod,
    MissingOntology,
    CompositionFailure,
    LanguageNormalizationFailure,
    VisualInputRequired,
    AmbiguousOrDefectiveQuestion,
}

#[derive(Debug, Serialize)]
struct QuestionRecord {
    id: Option<String>,
    category: String,
    question_sha256: String,
    question: String,
    expected: String,
    terminal_classification: TerminalClassification,
    answer: Option<String>,
    route: String,
    route_trace: Vec<String>,
    required_capabilities: Vec<String>,
    registry_version: String,
    ontology_version: String,
    abstention_or_authorization_receipt: Value,
    answer_provenance: Vec<String>,
    replay_result: String,
    execution_time_ms: f64,
    resource_usage: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize)]
struct Summary {
    release: String,
    frozen_commit: String,
    dataset: String,
    dataset_sha256: String,
    cases: usize,
    correct_authorized_answers: usize,
    incorrect_authorized_answers: usize,
    false_authorizations: usize,
    terminal_classifications: BTreeMap<TerminalClassification, usize>,
    replay_verified: usize,
    replay_not_applicable: usize,
    replay_not_recorded: usize,
    replay_failed: usize,
    total_execution_time_ms: f64,
    max_execution_time_ms: f64,
    trace_path: String,
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn classify(
    entry: &Value,
    orchestration: &OrchestratedAnswer,
    correct: bool,
) -> TerminalClassification {
    if orchestration.answer.is_some() {
        return if correct {
            TerminalClassification::CorrectAuthorizedAnswer
        } else {
            TerminalClassification::IncorrectAuthorizedAnswer
        };
    }

    let has_image = entry.get("has_image").and_then(Value::as_bool).unwrap_or(false);
    let lower_question = entry
        .get("question")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    let visual_dependency = has_image
        && [
            "attached image",
            "image shows",
            "shown in the image",
            "shown in image",
            "consider an image",
            "figure below",
            "diagram",
            "pictured",
            "photo",
            "graph shows",
            "chart shows",
        ]
        .iter()
        .any(|marker| lower_question.contains(marker));
    let reason = orchestration.abstention_reason;
    if visual_dependency && orchestration.answer.is_none() {
        return TerminalClassification::VisualInputRequired;
    }
    if has_image && matches!(reason, Some(AbstentionReason::MissingAttachment)) {
        return TerminalClassification::VisualInputRequired;
    }

    match reason {
        Some(AbstentionReason::ProblemParseFailed)
        | Some(AbstentionReason::TargetNotIdentified)
        | Some(AbstentionReason::SymbolBindingFailed) => {
            TerminalClassification::LanguageNormalizationFailure
        }
        Some(AbstentionReason::IntermediateNotDerivable)
        | Some(AbstentionReason::IntermediateSemanticMismatch)
        | Some(AbstentionReason::IntermediateValueKindMismatch)
        | Some(AbstentionReason::IntermediateQualifierMismatch)
        | Some(AbstentionReason::IntermediateConstraintConflict)
        | Some(AbstentionReason::PlanCycleDetected)
        | Some(AbstentionReason::PlanDepthExceeded)
        | Some(AbstentionReason::PlanExecutionFailed)
        | Some(AbstentionReason::PlanVerificationFailed) => {
            TerminalClassification::CompositionFailure
        }
        Some(AbstentionReason::MissingRequiredGiven)
        | Some(AbstentionReason::RequiredAssumptionMissing)
        | Some(AbstentionReason::RequiredAssumptionContradicted)
        | Some(AbstentionReason::MultipleUnresolvedMethods)
        | Some(AbstentionReason::ConflictingPlans)
        | Some(AbstentionReason::AnswerFormatFailed) => {
            TerminalClassification::AmbiguousOrDefectiveQuestion
        }
        Some(AbstentionReason::InsufficientEvidence)
            if matches!(orchestration.plan.domain, Tool::FactualQA | Tool::LifeScience) =>
        {
            TerminalClassification::MissingFactualKnowledge
        }
        Some(AbstentionReason::InsufficientEvidence) => {
            TerminalClassification::AmbiguousOrDefectiveQuestion
        }
        Some(AbstentionReason::UnsupportedDomain) => TerminalClassification::MissingOntology,
        Some(AbstentionReason::NoApplicableMethod)
        | Some(AbstentionReason::VerificationFailed) => TerminalClassification::MissingReasoningMethod,
        Some(AbstentionReason::SolverUnsupportedOperation) => {
            TerminalClassification::SafelyFormalizedButUnsupported
        }
        Some(AbstentionReason::MissingAttachment) => TerminalClassification::VisualInputRequired,
        None if orchestration.plan.domain == Tool::FactualQA => {
            TerminalClassification::MissingFactualKnowledge
        }
        None if orchestration.plan.problem.unresolved.is_empty() => {
            TerminalClassification::SafelyFormalizedButUnsupported
        }
        None => TerminalClassification::LanguageNormalizationFailure,
    }
}

fn replay_result(orchestration: &OrchestratedAnswer) -> String {
        if orchestration.answer.is_none() {
        return "not_applicable".to_string();
    }
    if let Some(receipt) = &orchestration.plan_execution_receipt {
        return if receipt.final_verification.passed {
            "verified".to_string()
        } else {
            "failed".to_string()
        };
    }
    if orchestration.execution_receipt.is_some()
        && orchestration
            .attempts
            .iter()
            .any(|attempt| attempt.to_ascii_lowercase().contains("replay"))
    {
        return "verified".to_string();
    }
    if !orchestration.evidence.is_empty() {
        "not_recorded".to_string()
    } else {
        "failed".to_string()
    }
}

fn receipt(orchestration: &OrchestratedAnswer) -> Value {
    json!({
        "domain": format!("{:?}", orchestration.plan.domain),
        "verification": orchestration.verification,
        "abstention_reason": orchestration.abstention_reason.map(|reason| format!("{:?}", reason)),
        "planned_derivation": orchestration.planned_derivation,
        "execution_receipt": orchestration.execution_receipt,
        "plan_execution_receipt": orchestration.plan_execution_receipt,
        "rejected_candidates": orchestration.rejected_candidates,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let trace_path = PathBuf::from(
        env::args()
            .nth(1)
            .unwrap_or_else(|| "/tmp/hle_release_candidate_2147e9e.traces.jsonl".into()),
    );
    let summary_path = PathBuf::from(
        env::args()
            .nth(2)
            .unwrap_or_else(|| "/tmp/hle_release_candidate_2147e9e.summary.json".into()),
    );
    let bytes = fs::read(DATASET)?;
    let dataset_sha256 = sha256(&bytes);
    let actual_commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_default();
    let implementation_unchanged = if actual_commit == FROZEN_COMMIT {
        true
    } else {
        Command::new("git")
            .args(["diff", "--name-only", FROZEN_COMMIT, "HEAD", "--"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .filter(|path| !path.is_empty())
                    .all(|path| {
                        matches!(
                            path,
                            "Cargo.toml"
                                | "src/bin/hle_release.rs"
                                | "docs/phase20_hle_release_candidate.md"
                        )
                    })
            })
            .unwrap_or(false)
    };
    if !implementation_unchanged {
        return Err(format!(
            "frozen HLE evaluation requires implementation unchanged since {FROZEN_COMMIT}; found HEAD {actual_commit}"
        )
        .into());
    }
    let file = BufReader::new(File::open(DATASET)?);
    if let Some(parent) = trace_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = summary_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut traces = File::create(&trace_path)?;
    let mut classifications = BTreeMap::new();
    let mut cases = 0;
    let mut correct_authorized_answers = 0;
    let mut incorrect_authorized_answers = 0;
    let mut replay_verified = 0;
    let mut replay_not_applicable = 0;
    let mut replay_not_recorded = 0;
    let mut replay_failed = 0;
    let mut total_execution_time_ms: f64 = 0.0;
    let mut max_execution_time_ms: f64 = 0.0;

    for line in file.lines() {
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
        let correct = orchestration.answer.as_deref().is_some_and(|answer| {
            QuestionRouter::exact_answers_match(answer, expected)
        });
        let class = classify(&entry, &orchestration, correct);
        let replay = replay_result(&orchestration);
        let capabilities: Vec<String> = orchestration
            .plan
            .required_capabilities
            .iter()
            .map(|capability| format!("{:?}", capability))
            .collect();
        let mut usage = BTreeMap::new();
        usage.insert("question_bytes".into(), question.len());
        usage.insert("attempts".into(), orchestration.attempts.len());
        usage.insert("givens".into(), orchestration.plan.problem.givens.len());
        usage.insert("methods".into(), orchestration.plan.methods.len());
        usage.insert("required_capabilities".into(), capabilities.len());
        usage.insert("rejected_candidates".into(), orchestration.rejected_candidates.len());
        let record = QuestionRecord {
            id: entry.get("id").and_then(Value::as_str).map(str::to_string),
            category: entry
                .get("category")
                .and_then(Value::as_str)
                .unwrap_or("uncategorized")
                .to_string(),
            question_sha256: sha256(question.as_bytes()),
            question: question.to_string(),
            expected: expected.to_string(),
            terminal_classification: class,
            answer: orchestration.answer.clone(),
            route: format!("{:?}", orchestration.plan.domain),
            route_trace: orchestration.attempts.clone(),
            required_capabilities: capabilities,
            registry_version: REGISTRY_VERSION.to_string(),
            ontology_version: ONTOLOGY_VERSION.to_string(),
            abstention_or_authorization_receipt: receipt(&orchestration),
            answer_provenance: orchestration
                .evidence
                .iter()
                .map(the_machine::router::VerificationEvidence::summary)
                .collect(),
            replay_result: replay.clone(),
            execution_time_ms: elapsed_ms,
            resource_usage: usage,
        };
        serde_json::to_writer(&mut traces, &record)?;
        writeln!(traces)?;
        *classifications.entry(class).or_insert(0) += 1;
        cases += 1;
        correct_authorized_answers += usize::from(class == TerminalClassification::CorrectAuthorizedAnswer);
        incorrect_authorized_answers += usize::from(class == TerminalClassification::IncorrectAuthorizedAnswer);
        match replay.as_str() {
            "verified" => replay_verified += 1,
            "not_applicable" => replay_not_applicable += 1,
            "not_recorded" => replay_not_recorded += 1,
            _ => replay_failed += 1,
        }
        total_execution_time_ms += elapsed_ms;
        max_execution_time_ms = max_execution_time_ms.max(elapsed_ms);
    }

    let summary = Summary {
        release: REGISTRY_VERSION.to_string(),
        frozen_commit: FROZEN_COMMIT.to_string(),
        dataset: DATASET.to_string(),
        dataset_sha256,
        cases,
        correct_authorized_answers,
        incorrect_authorized_answers,
        false_authorizations: incorrect_authorized_answers,
        terminal_classifications: classifications,
        replay_verified,
        replay_not_applicable,
        replay_not_recorded,
        replay_failed,
        total_execution_time_ms,
        max_execution_time_ms,
        trace_path: trace_path.display().to_string(),
    };
    fs::write(&summary_path, serde_json::to_vec_pretty(&summary)?)?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}
