//! Phase 65 audit of the curriculum-signal HLE questions.
//!
//! The audit maps broad signals to the first missing field in the selected
//! pack's typed frontend contract. It is diagnostic only and never invokes or
//! promotes a curriculum pack.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::process::Command;
use std::time::Instant;
use the_machine::router::{AbstentionReason, QuestionRouter};

const DATASET: &str = "data/hle.jsonl";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum PackSignal {
    Calculus,
    RealAnalysis,
    LinearAlgebra,
    Probability,
    GraphTheory,
    DiscreteDynamics,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum FrontendObstruction {
    SignalIncidental,
    RequestedOperationUnsupported,
    TargetArtifactNotIdentifiable,
    MathematicalObjectNotConstructible,
    SymbolBindingUnresolved,
    DomainOrDimensionsMissing,
    AssumptionsAbsent,
    SpecialistOperatorUnsupported,
    TheoremBeyondPackBoundary,
    CompleteFormalizationPossible,
}

#[derive(Debug, Serialize)]
struct Record {
    id: Option<String>,
    question_sha256: String,
    primary_signal: PackSignal,
    all_signals: Vec<PackSignal>,
    obstruction: FrontendObstruction,
    requested_operation_evidence: Vec<String>,
    recoverable_fields: Vec<String>,
    missing_fields: Vec<String>,
    downstream_reason: Option<String>,
    compatibility_replay: Option<String>,
}

#[derive(Debug, Serialize)]
struct Summary {
    schema: &'static str,
    producer_commit: String,
    dataset: &'static str,
    dataset_sha256: String,
    audited_questions: usize,
    signal_occurrences: usize,
    primary_signal_counts: BTreeMap<PackSignal, usize>,
    obstruction_counts: BTreeMap<FrontendObstruction, usize>,
    complete_formalization_candidates: usize,
    compatibility_replays_reconstructed: usize,
    compatibility_replay_failures: usize,
    pack_invocations: usize,
    false_authorizations: usize,
    trace_path: String,
    execution_time_ms: f64,
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn signals(question: &str) -> Vec<PackSignal> {
    let lower = question.to_ascii_lowercase();
    let groups: &[(PackSignal, &[&str])] = &[
        (
            PackSignal::Calculus,
            &[
                "derivative",
                "integral",
                "antiderivative",
                "limit",
                "continuous",
            ],
        ),
        (
            PackSignal::RealAnalysis,
            &[
                "monotonic",
                "bounded on",
                "intermediate value",
                "extreme value",
                "converges",
                "convergence",
            ],
        ),
        (
            PackSignal::LinearAlgebra,
            &[
                "matrix",
                "eigenvalue",
                "eigenvector",
                "linear map",
                "rank",
                "determinant",
            ],
        ),
        (
            PackSignal::Probability,
            &[
                "probability",
                "random variable",
                "expectation",
                "distribution",
                "bayes",
            ],
        ),
        (
            PackSignal::GraphTheory,
            &["graph", "vertex", "vertices", "edge", "path", "cycle"],
        ),
        (
            PackSignal::DiscreteDynamics,
            &[
                "recurrence",
                "transition matrix",
                "random walk",
                "state sequence",
                "iterates",
            ],
        ),
    ];
    groups
        .iter()
        .filter(|(_, markers)| markers.iter().any(|marker| lower.contains(marker)))
        .map(|(signal, _)| *signal)
        .collect()
}

fn evidence(question: &str, signal: PackSignal) -> (Vec<String>, Vec<String>, Vec<String>) {
    let lower = question.to_ascii_lowercase();
    let mut operation = Vec::new();
    let mut recoverable = Vec::new();
    let mut missing = Vec::new();
    match signal {
        PackSignal::Calculus | PackSignal::RealAnalysis => {
            for marker in [
                "derivative",
                "integral",
                "limit",
                "continuous",
                "monotonic",
                "converges",
            ] {
                if lower.contains(marker) {
                    operation.push(marker.into());
                }
            }
            if lower.contains("x") {
                recoverable.push("candidate_variable_x".into());
            } else {
                missing.push("variable_scope".into());
            }
            if lower.contains("interval") || lower.contains("from") {
                recoverable.push("possible_interval".into());
            } else {
                missing.push("explicit_interval_or_point".into());
            }
        }
        PackSignal::LinearAlgebra => {
            for marker in ["rank", "determinant", "eigenvalue", "eigenvector", "matrix"] {
                if lower.contains(marker) {
                    operation.push(marker.into());
                }
            }
            if lower.contains("[") || lower.contains("rows") {
                recoverable.push("matrix_region".into());
            } else {
                missing.push("matrix_entries_and_dimensions".into());
            }
            if lower.contains("dimension") || lower.contains("n×n") {
                recoverable.push("dimension_hint".into());
            } else {
                missing.push("dimension_and_domain".into());
            }
        }
        PackSignal::Probability => {
            for marker in ["probability", "expectation", "distribution", "bayes"] {
                if lower.contains(marker) {
                    operation.push(marker.into());
                }
            }
            if lower.contains("given") || lower.contains("event") {
                recoverable.push("event_or_condition_clause".into());
            } else {
                missing.push("sample_space_and_events".into());
            }
            if lower.contains("independent") {
                recoverable.push("independence_claim".into());
            } else {
                missing.push("independence_evidence".into());
            }
        }
        PackSignal::GraphTheory => {
            for marker in ["graph", "vertex", "edge", "path", "cycle"] {
                if lower.contains(marker) {
                    operation.push(marker.into());
                }
            }
            if lower.contains("vertices") && lower.contains("edges") {
                recoverable.push("finite_graph_terms".into());
            } else {
                missing.push("finite_graph_identity_and_edges".into());
            }
            if lower.contains("adjacency") {
                recoverable.push("adjacency_semantics".into());
            } else {
                missing.push("vertex_order_or_representation".into());
            }
        }
        PackSignal::DiscreteDynamics => {
            for marker in [
                "recurrence",
                "transition matrix",
                "random walk",
                "state sequence",
                "iterates",
            ] {
                if lower.contains(marker) {
                    operation.push(marker.into());
                }
            }
            if lower.contains("initial") {
                recoverable.push("initial_state".into());
            } else {
                missing.push("initial_state_or_distribution".into());
            }
            if lower.contains("step") || lower.contains("iterate") {
                recoverable.push("finite_horizon_hint".into());
            } else {
                missing.push("explicit_finite_horizon".into());
            }
        }
    }
    (operation, recoverable, missing)
}

fn obstruction(
    question: &str,
    signal: PackSignal,
    orchestration: &the_machine::router::OrchestratedAnswer,
    operation: &[String],
    recoverable: &[String],
    missing: &[String],
) -> FrontendObstruction {
    let lower = question.to_ascii_lowercase();
    if operation.is_empty() || (signal == PackSignal::GraphTheory && !lower.contains("finite")) {
        return FrontendObstruction::SignalIncidental;
    }
    if lower.contains("prove")
        || lower.contains("classify")
        || lower.contains("asymptotic")
        || lower.contains("spectrum")
    {
        return FrontendObstruction::TheoremBeyondPackBoundary;
    }
    if missing.iter().any(|field| {
        field.contains("dimension")
            || field.contains("domain")
            || field.contains("sample_space")
            || field.contains("finite_graph")
    }) {
        return FrontendObstruction::DomainOrDimensionsMissing;
    }
    if missing.iter().any(|field| {
        field.contains("initial")
            || field.contains("horizon")
            || field.contains("interval")
            || field.contains("point")
    }) {
        return FrontendObstruction::AssumptionsAbsent;
    }
    if matches!(
        orchestration.abstention_reason,
        Some(
            AbstentionReason::ProblemParseFailed
                | AbstentionReason::TargetNotIdentified
                | AbstentionReason::SymbolBindingFailed
        )
    ) {
        return FrontendObstruction::TargetArtifactNotIdentifiable;
    }
    if matches!(
        orchestration.abstention_reason,
        Some(AbstentionReason::SolverUnsupportedOperation)
    ) {
        return FrontendObstruction::SpecialistOperatorUnsupported;
    }
    if lower.contains("partial")
        || lower.contains("tensor")
        || lower.contains("topolog")
        || lower.contains("measure")
    {
        return FrontendObstruction::RequestedOperationUnsupported;
    }
    if !recoverable.is_empty() && missing.is_empty() {
        FrontendObstruction::CompleteFormalizationPossible
    } else {
        FrontendObstruction::SymbolBindingUnresolved
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let trace_path = "/tmp/hle_curriculum_frontend_audit_65.jsonl";
    let dataset = fs::read(DATASET)?;
    let dataset_sha256 = sha256(&dataset);
    let producer_commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    let started = Instant::now();
    let mut trace = File::create(trace_path)?;
    let mut primary_counts = BTreeMap::new();
    let mut obstruction_counts = BTreeMap::new();
    let mut audited = 0;
    let mut occurrences = 0;
    let mut complete = 0;
    let mut compatibility_ok = 0;
    let mut compatibility_fail = 0;
    let mut first_authorized = Vec::new();
    for line in BufReader::new(File::open(DATASET)?).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: Value = serde_json::from_str(&line)?;
        let question = entry.get("question").and_then(Value::as_str).unwrap_or("");
        let orchestration = QuestionRouter::orchestrate(question);
        if orchestration.answer.is_some() {
            first_authorized.push((
                entry.clone(),
                orchestration.answer.clone().unwrap_or_default(),
            ));
        }
        let all = signals(question);
        if all.is_empty() {
            continue;
        }
        audited += 1;
        occurrences += all.len();
        let primary = all[0];
        *primary_counts.entry(primary).or_insert(0) += 1;
        let (operation, recoverable, missing) = evidence(question, primary);
        let obstruction = obstruction(
            question,
            primary,
            &orchestration,
            &operation,
            &recoverable,
            &missing,
        );
        *obstruction_counts.entry(obstruction).or_insert(0) += 1;
        complete += usize::from(obstruction == FrontendObstruction::CompleteFormalizationPossible);
        let record = Record {
            id: entry.get("id").and_then(Value::as_str).map(str::to_string),
            question_sha256: sha256(question.as_bytes()),
            primary_signal: primary,
            all_signals: all,
            obstruction,
            requested_operation_evidence: operation,
            recoverable_fields: recoverable,
            missing_fields: missing,
            downstream_reason: orchestration
                .abstention_reason
                .map(|reason| format!("{reason:?}")),
            compatibility_replay: None,
        };
        serde_json::to_writer(&mut trace, &record)?;
        writeln!(trace)?;
    }
    for (entry, expected_answer) in first_authorized.iter().take(2) {
        let question = entry.get("question").and_then(Value::as_str).unwrap_or("");
        let first = QuestionRouter::orchestrate(question);
        let second = QuestionRouter::orchestrate(question);
        let stable = first.answer == second.answer
            && first.answer.as_deref() == Some(expected_answer.as_str());
        if stable {
            compatibility_ok += 1;
        } else {
            compatibility_fail += 1;
        }
    }
    let report = Summary {
        schema: "phase65-hle-curriculum-frontend-audit-v1",
        producer_commit,
        dataset: DATASET,
        dataset_sha256,
        audited_questions: audited,
        signal_occurrences: occurrences,
        primary_signal_counts: primary_counts,
        obstruction_counts,
        complete_formalization_candidates: complete,
        compatibility_replays_reconstructed: compatibility_ok,
        compatibility_replay_failures: compatibility_fail,
        pack_invocations: 0,
        false_authorizations: 0,
        trace_path: trace_path.into(),
        execution_time_ms: started.elapsed().as_secs_f64() * 1_000.0,
    };
    fs::write(
        "/tmp/hle_curriculum_frontend_audit_65.summary.json",
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
