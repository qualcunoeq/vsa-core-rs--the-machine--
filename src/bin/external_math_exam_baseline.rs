//! Baseline evaluation for the independently sourced MATH release.
//!
//! The release contains original prompts and answer hashes only.  This
//! evaluator is the privileged scorer: development code receives prompts but
//! never receives plaintext sealed answers.  Every case gets one terminal
//! classification, a first-failing-gate label, and a replay status.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::process::Command;
use std::time::Instant;
use the_machine::router::{AbstentionReason, OrchestratedAnswer, QuestionRouter};

const RELEASE_DIR: &str = "data/external_math_exam_v1";

#[derive(Debug, Clone, Deserialize)]
struct QuestionRecord {
    id: String,
    source_id: String,
    source_item_id: String,
    split: String,
    category: String,
    level: String,
    original_prompt: String,
}

#[derive(Debug, Clone, Deserialize)]
struct OracleRecord {
    id: String,
    expected_outcome: String,
    answer_sha256: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ExamSplit {
    Development,
    Sealed,
}

impl ExamSplit {
    fn as_str(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Sealed => "sealed",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Terminal {
    CorrectAuthorized,
    IncorrectAuthorized,
    FalseAbstention,
}

#[derive(Debug, Serialize)]
struct TraceRecord {
    index: usize,
    id: String,
    source_id: String,
    source_item_id: String,
    split: ExamSplit,
    category: String,
    level: String,
    question_sha256: String,
    terminal: Terminal,
    first_gate: String,
    abstention_reason: Option<String>,
    answer_sha256: Option<String>,
    expected_answer_sha256: String,
    exact_reference_match: bool,
    route: String,
    route_trace: Vec<String>,
    required_capabilities: Vec<String>,
    verification: String,
    replay_result: String,
    execution_time_ms: f64,
}

#[derive(Debug, Serialize)]
struct Summary {
    schema: &'static str,
    release_id: &'static str,
    producer_commit: String,
    split: String,
    cases: usize,
    terminal_counts: BTreeMap<Terminal, usize>,
    first_gate_counts: BTreeMap<String, usize>,
    correct_authorized: usize,
    incorrect_authorized: usize,
    false_authorizations: usize,
    false_abstentions: usize,
    replay_verified: usize,
    replay_not_applicable: usize,
    replay_failed: usize,
    registry_mutated: bool,
    source_manifest_sha256: String,
    questions_sha256: String,
    oracle_sha256: String,
    curriculum_manifest_sha256: String,
    trace_sha256: String,
    total_execution_time_ms: f64,
    max_execution_time_ms: f64,
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn digest_file(path: &str) -> Result<String, Box<dyn std::error::Error>> {
    Ok(digest_bytes(&fs::read(path)?))
}

fn parse_split(value: &str) -> Result<Option<ExamSplit>, Box<dyn std::error::Error>> {
    match value {
        "all" => Ok(None),
        "development" => Ok(Some(ExamSplit::Development)),
        "sealed" => Ok(Some(ExamSplit::Sealed)),
        other => Err(format!("invalid split {other}; expected all, development, or sealed").into()),
    }
}

fn read_jsonl<T: for<'de> Deserialize<'de>>(
    path: &str,
) -> Result<Vec<T>, Box<dyn std::error::Error>> {
    let mut records = Vec::new();
    for line in BufReader::new(File::open(path)?).lines() {
        let line = line?;
        if !line.trim().is_empty() {
            records.push(serde_json::from_str(&line)?);
        }
    }
    Ok(records)
}

fn first_gate(reason: Option<AbstentionReason>) -> &'static str {
    match reason {
        Some(AbstentionReason::ProblemParseFailed)
        | Some(AbstentionReason::TargetNotIdentified)
        | Some(AbstentionReason::SymbolBindingFailed)
        | Some(AbstentionReason::AnswerFormatFailed) => "language_normalization_failure",
        Some(AbstentionReason::MissingRequiredGiven) => "missing_prerequisite",
        Some(AbstentionReason::RequiredAssumptionMissing)
        | Some(AbstentionReason::RequiredAssumptionContradicted) => "missing_prerequisite",
        Some(AbstentionReason::InsufficientEvidence) => "missing_knowledge",
        Some(AbstentionReason::NoApplicableMethod) | Some(AbstentionReason::VerificationFailed) => {
            "missing_method"
        }
        Some(AbstentionReason::UnsupportedDomain)
        | Some(AbstentionReason::SolverUnsupportedOperation)
        | Some(AbstentionReason::IntermediateSemanticMismatch)
        | Some(AbstentionReason::IntermediateValueKindMismatch)
        | Some(AbstentionReason::IntermediateQualifierMismatch)
        | Some(AbstentionReason::IntermediateConstraintConflict) => "representation_gap",
        Some(AbstentionReason::MultipleUnresolvedMethods)
        | Some(AbstentionReason::ConflictingPlans) => "ambiguous",
        Some(AbstentionReason::MissingAttachment) => "visual_dependency",
        Some(AbstentionReason::PlanDepthExceeded)
        | Some(AbstentionReason::PlanExecutionFailed)
        | Some(AbstentionReason::PlanCycleDetected) => "budget_or_execution_boundary",
        Some(AbstentionReason::PlanVerificationFailed)
        | Some(AbstentionReason::IntermediateNotDerivable) => "unsupported_target",
        None => "unsupported_target",
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
    let split_filter =
        parse_split(&env::var("EXTERNAL_MATH_EXAM_SPLIT").unwrap_or_else(|_| "all".into()))?;
    let output = env::var("EXTERNAL_MATH_EXAM_SUMMARY")
        .unwrap_or_else(|_| "/tmp/external_math_exam_baseline.json".into());
    let trace_path = env::var("EXTERNAL_MATH_EXAM_TRACE")
        .unwrap_or_else(|_| "/tmp/external_math_exam_baseline.trace.jsonl".into());
    let questions_path = format!("{RELEASE_DIR}/questions.jsonl");
    let oracle_path = format!(
        "{RELEASE_DIR}/oracle_{}.jsonl",
        split_filter.map_or("development", ExamSplit::as_str)
    );
    let questions_bytes = fs::read(&questions_path)?;
    let oracle_bytes = fs::read(&oracle_path)?;
    let manifest_hash = digest_file(&format!("{RELEASE_DIR}/manifest.json"))?;
    let questions: Vec<QuestionRecord> = read_jsonl(&questions_path)?;
    let oracle: Vec<OracleRecord> = read_jsonl(&oracle_path)?;
    let expected = oracle
        .into_iter()
        .map(|record| (record.id.clone(), record))
        .collect::<BTreeMap<_, _>>();
    let selected = questions
        .into_iter()
        .filter(|record| split_filter.map_or(true, |split| record.split == split.as_str()))
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err("no selected questions".into());
    }
    if selected
        .iter()
        .any(|record| !expected.contains_key(&record.id))
    {
        return Err("question/oracle ids do not match".into());
    }
    if expected
        .values()
        .any(|record| record.expected_outcome != "supported")
    {
        return Err("external MATH baseline expects every selected case to be supported".into());
    }
    let producer_commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".into());
    let manifest_before = the_machine::curriculum::breadth_first_manifest().replay_hash();
    let mut trace = File::create(&trace_path)?;
    let mut terminal_counts = BTreeMap::new();
    let mut first_gate_counts = BTreeMap::new();
    let mut correct_authorized = 0;
    let mut incorrect_authorized = 0;
    let mut false_abstentions = 0;
    let mut replay_verified = 0;
    let mut replay_not_applicable = 0;
    let mut replay_failed = 0;
    let mut total_ms = 0.0;
    let mut max_ms: f64 = 0.0;

    for (index, record) in selected.iter().enumerate() {
        let expected_record = expected.get(&record.id).expect("validated oracle id");
        let started = Instant::now();
        let orchestration = QuestionRouter::orchestrate(&record.original_prompt);
        let elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
        let candidate_hash = orchestration
            .answer
            .as_deref()
            .map(|answer| digest_bytes(answer.as_bytes()));
        let exact_match = candidate_hash
            .as_deref()
            .is_some_and(|hash| hash == expected_record.answer_sha256);
        let terminal = if orchestration.answer.is_some() {
            if exact_match {
                correct_authorized += 1;
                Terminal::CorrectAuthorized
            } else {
                incorrect_authorized += 1;
                Terminal::IncorrectAuthorized
            }
        } else {
            false_abstentions += 1;
            Terminal::FalseAbstention
        };
        let gate = if orchestration.answer.is_some() {
            "authorized"
        } else {
            first_gate(orchestration.abstention_reason)
        };
        *terminal_counts.entry(terminal).or_insert(0) += 1;
        *first_gate_counts.entry(gate.to_string()).or_insert(0) += 1;
        let replay_result = replay(&record.original_prompt, &orchestration);
        match replay_result {
            "verified" => replay_verified += 1,
            "not_applicable" => replay_not_applicable += 1,
            _ => replay_failed += 1,
        }
        total_ms += elapsed_ms;
        max_ms = max_ms.max(elapsed_ms);
        let route = format!("{:?}", orchestration.plan.domain);
        let required_capabilities = orchestration
            .plan
            .required_capabilities
            .iter()
            .map(|capability| format!("{capability:?}"))
            .collect::<Vec<_>>();
        serde_json::to_writer(
            &mut trace,
            &TraceRecord {
                index,
                id: record.id.clone(),
                source_id: record.source_id.clone(),
                source_item_id: record.source_item_id.clone(),
                split: if record.split == "development" {
                    ExamSplit::Development
                } else {
                    ExamSplit::Sealed
                },
                category: record.category.clone(),
                level: record.level.clone(),
                question_sha256: digest_bytes(record.original_prompt.as_bytes()),
                terminal,
                first_gate: gate.to_string(),
                abstention_reason: orchestration
                    .abstention_reason
                    .map(|reason| format!("{reason:?}")),
                answer_sha256: candidate_hash,
                expected_answer_sha256: expected_record.answer_sha256.clone(),
                exact_reference_match: exact_match,
                route,
                route_trace: orchestration.attempts.clone(),
                required_capabilities,
                verification: orchestration.verification.clone(),
                replay_result: replay_result.into(),
                execution_time_ms: elapsed_ms,
            },
        )?;
        writeln!(trace)?;
        if index % 250 == 0 {
            eprintln!("evaluated {}/{}", index + 1, selected.len());
        }
    }
    drop(trace);
    let manifest_after = the_machine::curriculum::breadth_first_manifest().replay_hash();
    let trace_sha256 = digest_file(&trace_path)?;
    let summary = Summary {
        schema: "external-math-exam-baseline-v1",
        release_id: "external-math-exam-v1",
        producer_commit,
        split: split_filter.map_or("all".into(), |split| split.as_str().into()),
        cases: selected.len(),
        terminal_counts,
        first_gate_counts,
        correct_authorized,
        incorrect_authorized,
        false_authorizations: incorrect_authorized,
        false_abstentions,
        replay_verified,
        replay_not_applicable,
        replay_failed,
        registry_mutated: manifest_before != manifest_after,
        source_manifest_sha256: manifest_hash,
        questions_sha256: digest_bytes(&questions_bytes),
        oracle_sha256: digest_bytes(&oracle_bytes),
        curriculum_manifest_sha256: manifest_before,
        trace_sha256,
        total_execution_time_ms: total_ms,
        max_execution_time_ms: max_ms,
    };
    if summary.incorrect_authorized != 0 || summary.false_authorizations != 0 {
        return Err("baseline produced an incorrect authorization".into());
    }
    if summary.registry_mutated || summary.replay_failed != 0 {
        return Err("baseline governance invariant failed".into());
    }
    fs::write(&output, serde_json::to_vec_pretty(&summary)?)?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}
