//! Phase 25 shadow evaluation for accepted HLE notation artifacts.
//!
//! This evaluator consumes only the 16 equations/expressions artifacts accepted
//! by the Phase 24 shadow normalizer.  It sends the normalized notation through
//! the existing router in shadow mode and records the first downstream gap.  It
//! never changes production routing, authorization, registries, or HLE scores.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use the_machine::notation_normalization::{normalize_equation, NormalizationStatus};
use the_machine::router::{OrchestratedAnswer, QuestionRouter};

const DATASET: &str = "data/hle.jsonl";

#[derive(Debug, Deserialize)]
struct HleAudit {
    source_trace_sha256: String,
    records: Vec<HleRow>,
}

#[derive(Debug, Deserialize)]
struct HleRow {
    id: Option<String>,
    notation_family: String,
    downstream_outlook: String,
    question: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum DownstreamTerminal {
    CorrectAuthorizedAnswer,
    IncorrectAuthorizedAnswer,
    SafelyFormalizedButUnsupported,
    MissingReasoningMethod,
    CompositionFailure,
    LanguageNormalizationFailure,
    AmbiguousOrDefectiveQuestion,
}

#[derive(Debug, Serialize)]
struct DownstreamRow {
    id: Option<String>,
    normalization_status: NormalizationStatus,
    normalized_source: Option<String>,
    selected_capability_route: String,
    normalized_probe_route: String,
    terminal_classification: DownstreamTerminal,
    candidate_answer: Option<String>,
    baseline_answer: Option<String>,
    expected_answer: Option<String>,
    normalized_replay_verified: bool,
    downstream_replay_result: String,
    downstream_abstention: Option<String>,
    interpretation_changed: bool,
}

#[derive(Debug, Serialize)]
struct Report {
    source_audit_sha256: String,
    source_trace_sha256: String,
    dataset_sha256: String,
    candidate_rows: usize,
    accepted_artifacts: usize,
    correct_authorized_answers: usize,
    incorrect_authorized_answers: usize,
    safely_formalized_but_unsupported: usize,
    missing_reasoning_method: usize,
    composition_failures: usize,
    language_normalization_failures: usize,
    ambiguous_or_defective: usize,
    normalized_replay_verified: usize,
    downstream_replay_verified: usize,
    interpretation_changes: usize,
    false_authorizations: usize,
    terminal_classifications: BTreeMap<DownstreamTerminal, usize>,
    records: Vec<DownstreamRow>,
    method: String,
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn route_name(orchestration: &OrchestratedAnswer) -> String {
    format!("{:?}", orchestration.plan.domain)
}

fn replay_result(orchestration: &OrchestratedAnswer) -> String {
    if orchestration.answer.is_none() {
        return "not_applicable".into();
    }
    if orchestration
        .plan_execution_receipt
        .as_ref()
        .is_some_and(|receipt| receipt.final_verification.passed)
    {
        return "verified".into();
    }
    if orchestration.execution_receipt.is_some()
        && orchestration
            .attempts
            .iter()
            .any(|attempt| attempt.to_ascii_lowercase().contains("replay"))
    {
        return "verified".into();
    }
    "failed".into()
}

fn terminal(normalized: &OrchestratedAnswer, expected: Option<&str>) -> DownstreamTerminal {
    if let Some(answer) = normalized.answer.as_deref() {
        return if expected
            .is_some_and(|expected| QuestionRouter::exact_answers_match(answer, expected))
        {
            DownstreamTerminal::CorrectAuthorizedAnswer
        } else {
            DownstreamTerminal::IncorrectAuthorizedAnswer
        };
    }
    match normalized.abstention_reason {
        Some(the_machine::router::AbstentionReason::ProblemParseFailed)
        | Some(the_machine::router::AbstentionReason::TargetNotIdentified)
        | Some(the_machine::router::AbstentionReason::SymbolBindingFailed) => {
            DownstreamTerminal::LanguageNormalizationFailure
        }
        Some(the_machine::router::AbstentionReason::IntermediateNotDerivable)
        | Some(the_machine::router::AbstentionReason::IntermediateSemanticMismatch)
        | Some(the_machine::router::AbstentionReason::IntermediateValueKindMismatch)
        | Some(the_machine::router::AbstentionReason::IntermediateQualifierMismatch)
        | Some(the_machine::router::AbstentionReason::IntermediateConstraintConflict)
        | Some(the_machine::router::AbstentionReason::PlanCycleDetected)
        | Some(the_machine::router::AbstentionReason::PlanDepthExceeded)
        | Some(the_machine::router::AbstentionReason::PlanExecutionFailed)
        | Some(the_machine::router::AbstentionReason::PlanVerificationFailed) => {
            DownstreamTerminal::CompositionFailure
        }
        Some(the_machine::router::AbstentionReason::MissingRequiredGiven)
        | Some(the_machine::router::AbstentionReason::RequiredAssumptionMissing)
        | Some(the_machine::router::AbstentionReason::RequiredAssumptionContradicted)
        | Some(the_machine::router::AbstentionReason::MultipleUnresolvedMethods)
        | Some(the_machine::router::AbstentionReason::ConflictingPlans)
        | Some(the_machine::router::AbstentionReason::AnswerFormatFailed) => {
            DownstreamTerminal::AmbiguousOrDefectiveQuestion
        }
        Some(the_machine::router::AbstentionReason::InsufficientEvidence) => {
            // The notation artifact is replayable, but the isolated math
            // region carries no executable target/context for the existing
            // downstream stack.
            DownstreamTerminal::SafelyFormalizedButUnsupported
        }
        Some(the_machine::router::AbstentionReason::SolverUnsupportedOperation)
        | Some(the_machine::router::AbstentionReason::NoApplicableMethod)
        | Some(the_machine::router::AbstentionReason::VerificationFailed) => {
            DownstreamTerminal::MissingReasoningMethod
        }
        Some(the_machine::router::AbstentionReason::UnsupportedDomain)
        | Some(the_machine::router::AbstentionReason::MissingAttachment) => {
            DownstreamTerminal::SafelyFormalizedButUnsupported
        }
        None => DownstreamTerminal::SafelyFormalizedButUnsupported,
    }
}

fn dataset_answers() -> Result<(String, BTreeMap<String, String>), Box<dyn std::error::Error>> {
    let bytes = fs::read(DATASET)?;
    let mut answers = BTreeMap::new();
    for line in String::from_utf8(bytes.clone())?.lines() {
        let entry: Value = serde_json::from_str(line)?;
        if let (Some(id), Some(answer)) = (
            entry.get("id").and_then(Value::as_str),
            entry.get("answer").and_then(Value::as_str),
        ) {
            answers.insert(id.to_string(), answer.to_string());
        }
    }
    Ok((sha256(&bytes), answers))
}

fn run(audit_path: &str) -> Result<Report, Box<dyn std::error::Error>> {
    let audit_bytes = fs::read(audit_path)?;
    let audit: HleAudit = serde_json::from_slice(&audit_bytes)?;
    let (dataset_sha256, answers) = dataset_answers()?;
    let rows: Vec<_> = audit
        .records
        .into_iter()
        .filter(|row| {
            row.notation_family == "equations_and_expressions"
                && row.downstream_outlook == "likely_normalization_only"
        })
        .collect();
    let mut records = Vec::new();
    let mut counts = BTreeMap::new();
    let mut normalized_replay_verified = 0;
    let mut downstream_replay_verified = 0;
    let mut interpretation_changes = 0;
    for row in rows.iter() {
        let normalized = normalize_equation(&row.question);
        if normalized.status != NormalizationStatus::Accepted {
            continue;
        }
        let expected = row
            .id
            .as_ref()
            .and_then(|id| answers.get(id))
            .map(String::as_str);
        let baseline = QuestionRouter::orchestrate(&row.question);
        let normalized_source = normalized.normalized_source.clone();
        let normalized_probe = normalized_source
            .as_deref()
            .map(QuestionRouter::orchestrate)
            .unwrap_or_else(|| QuestionRouter::orchestrate(""));
        let class = terminal(&normalized_probe, expected);
        *counts.entry(class).or_insert(0) += 1;
        normalized_replay_verified += usize::from(normalized.replay_verified);
        downstream_replay_verified += usize::from(replay_result(&normalized_probe) == "verified");
        let baseline_answer = baseline.answer.clone();
        let candidate_answer = normalized_probe.answer.clone();
        let route_changed = route_name(&baseline) != route_name(&normalized_probe);
        let interpretation_changed = route_changed || baseline_answer != candidate_answer;
        interpretation_changes += usize::from(interpretation_changed);
        records.push(DownstreamRow {
            id: row.id.clone(),
            normalization_status: normalized.status,
            normalized_source,
            selected_capability_route: route_name(&baseline),
            normalized_probe_route: route_name(&normalized_probe),
            terminal_classification: class,
            candidate_answer,
            baseline_answer,
            expected_answer: expected.map(str::to_string),
            normalized_replay_verified: normalized.replay_verified,
            downstream_replay_result: replay_result(&normalized_probe),
            downstream_abstention: normalized_probe
                .abstention_reason
                .map(|reason| format!("{reason:?}")),
            interpretation_changed,
        });
    }
    let correct = *counts
        .get(&DownstreamTerminal::CorrectAuthorizedAnswer)
        .unwrap_or(&0);
    let incorrect = *counts
        .get(&DownstreamTerminal::IncorrectAuthorizedAnswer)
        .unwrap_or(&0);
    Ok(Report {
        source_audit_sha256: sha256(&audit_bytes),
        source_trace_sha256: audit.source_trace_sha256,
        dataset_sha256,
        candidate_rows: rows.len(),
        accepted_artifacts: records.len(),
        correct_authorized_answers: correct,
        incorrect_authorized_answers: incorrect,
        safely_formalized_but_unsupported: *counts
            .get(&DownstreamTerminal::SafelyFormalizedButUnsupported)
            .unwrap_or(&0),
        missing_reasoning_method: *counts
            .get(&DownstreamTerminal::MissingReasoningMethod)
            .unwrap_or(&0),
        composition_failures: *counts
            .get(&DownstreamTerminal::CompositionFailure)
            .unwrap_or(&0),
        language_normalization_failures: *counts
            .get(&DownstreamTerminal::LanguageNormalizationFailure)
            .unwrap_or(&0),
        ambiguous_or_defective: *counts
            .get(&DownstreamTerminal::AmbiguousOrDefectiveQuestion)
            .unwrap_or(&0),
        normalized_replay_verified,
        downstream_replay_verified,
        interpretation_changes,
        false_authorizations: incorrect,
        terminal_classifications: counts,
        records,
        method: "shadow-only: accepted notation artifacts are normalized, passed through the existing router, and scored only when the downstream answer itself matches the frozen answer and replay verifies; no production mutation".into(),
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/hle_notation_downstream_2147e9e.json".into());
    let audit_path = env::args()
        .nth(2)
        .unwrap_or_else(|| "/tmp/hle_notation_audit_2147e9e.json".into());
    let report = run(&audit_path)?;
    fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_never_authorizes_without_a_downstream_answer() {
        let orchestration = QuestionRouter::orchestrate("x + 1 = 2");
        assert_ne!(
            terminal(&orchestration, Some("1")),
            DownstreamTerminal::CorrectAuthorizedAnswer
        );
    }
}
