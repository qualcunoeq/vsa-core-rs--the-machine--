//! Frozen HLE curriculum checkpoint.
//!
//! This diagnostic run keeps curriculum packs in shadow mode. It records the
//! strict funnel from question text to a possible typed curriculum route, but
//! never mutates production routing or authorizes a curriculum answer.

use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use the_machine::router::{AbstentionReason, QuestionRouter};

const DATASET: &str = "data/hle.jsonl";
const CHECKPOINT: &str = "curriculum-shadow-checkpoint-63";
const REGISTRY_VERSION: &str = "shadow-only-no-production-mutation";
const ONTOLOGY_VERSION: &str = "curriculum-63-real-analysis";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum PackSignal {
    Calculus,
    RealAnalysis,
    LinearAlgebra,
    Probability,
    GraphTheory,
    DiscreteDynamics,
    Combinatorics,
    NumberTheory,
    AbstractAlgebra,
    OrdinaryDifferentialEquations,
    FiniteMarkov,
    ClassicalMechanics,
    FiniteTopology,
    FiniteMetric,
    FiniteStatistics,
    Chemistry,
    Biology,
    ComplexArithmetic,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum FirstFailure {
    AuthorizedReferenceMatch,
    VisualDependency,
    NoCurriculumSignal,
    LanguageNormalization,
    UnsupportedTargetType,
    MissingFactualPrerequisite,
    MissingSpecialistTheorem,
    RepresentationGap,
    AssumptionsNotEstablished,
    PackBoundary,
    AnswerEquivalence,
}

#[derive(Debug, Serialize)]
struct Record {
    id: Option<String>,
    question_sha256: String,
    category: String,
    question: String,
    expected: String,
    signals: Vec<PackSignal>,
    first_failure: FirstFailure,
    curriculum_route: String,
    pack_invoked: bool,
    candidate_answer: Option<String>,
    route_trace: Vec<String>,
    replay_result: String,
    registry_version: String,
    ontology_version: String,
    receipt: Value,
    execution_time_ms: f64,
}

#[derive(Debug, Serialize)]
struct Summary {
    checkpoint: String,
    producer_commit: String,
    dataset: &'static str,
    dataset_sha256: String,
    curriculum_manifest_hash: String,
    registry_version: &'static str,
    ontology_version: String,
    cases: usize,
    correct_authorized_answers: usize,
    incorrect_authorized_answers: usize,
    false_authorizations: usize,
    safely_formalized_but_unsupported: usize,
    pack_signals: BTreeMap<PackSignal, usize>,
    first_failures: BTreeMap<FirstFailure, usize>,
    curriculum_candidates: usize,
    pack_invocations: usize,
    replay_verified: usize,
    replay_not_applicable: usize,
    replay_not_recorded: usize,
    replay_mismatch: usize,
    trace_sha256: String,
    manifest_mutated: bool,
    execution_budget_ms: u64,
    timed_out: usize,
    no_signal_short_circuits: usize,
    total_execution_time_ms: f64,
    max_execution_time_ms: f64,
    trace_path: String,
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn signals(question: &str) -> Vec<PackSignal> {
    let lower = question.to_ascii_lowercase();
    let mut found = Vec::new();
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
        (
            PackSignal::Combinatorics,
            &[
                "binomial",
                "multinomial",
                "inclusion-exclusion",
                "pigeonhole",
                "surjection",
            ],
        ),
        (
            PackSignal::NumberTheory,
            &[
                "modular",
                "congruence",
                "gcd",
                "divisibility",
                "prime",
                "totient",
                "diophantine",
            ],
        ),
        (
            PackSignal::AbstractAlgebra,
            &[
                "group homomorphism",
                "cyclic group",
                "ring",
                "field",
                "kernel",
                "quotient group",
            ],
        ),
        (
            PackSignal::OrdinaryDifferentialEquations,
            &[
                "differential equation",
                "initial value problem",
                "separable ode",
                "constant-coefficient",
            ],
        ),
        (
            PackSignal::FiniteMarkov,
            &[
                "markov chain",
                "stationary distribution",
                "hitting probability",
                "transition matrix",
            ],
        ),
        (
            PackSignal::ClassicalMechanics,
            &[
                "force",
                "mass",
                "momentum",
                "kinetic energy",
                "spring constant",
                "acceleration",
            ],
        ),
        (
            PackSignal::FiniteTopology,
            &["topology", "open set", "closed set", "closure", "interior"],
        ),
        (
            PackSignal::FiniteMetric,
            &["metric space", "distance function", "open ball", "diameter"],
        ),
        (
            PackSignal::FiniteStatistics,
            &[
                "regression",
                "variance",
                "covariance",
                "r-squared",
                "standard deviation",
            ],
        ),
        (
            PackSignal::Chemistry,
            &[
                "chemical reaction",
                "stoichiometric",
                "molecular formula",
                "moles",
                "element count",
            ],
        ),
        (
            PackSignal::Biology,
            &["dna", "nucleotide", "genome", "gene", "codon", "mutation"],
        ),
        (
            PackSignal::ComplexArithmetic,
            &[
                "complex number",
                "imaginary part",
                "real part",
                "complex conjugate",
                "modulus",
            ],
        ),
    ];
    for (signal, markers) in groups {
        if markers.iter().any(|marker| lower.contains(marker)) {
            found.push(*signal);
        }
    }
    found
}

fn visual(question: &Value, text: &str) -> bool {
    let has_image = question
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
        .any(|marker| text.to_ascii_lowercase().contains(marker))
}

fn replay(question: &str, orchestration: &the_machine::router::OrchestratedAnswer) -> String {
    if orchestration.answer.is_some() {
        if orchestration.plan_execution_receipt.is_some() {
            "verified".into()
        } else {
            let rerun = QuestionRouter::orchestrate(question);
            if rerun.answer == orchestration.answer
                && rerun.evidence == orchestration.evidence
                && rerun.verification == orchestration.verification
            {
                "verified".into()
            } else {
                "mismatch".into()
            }
        }
    } else {
        "not_applicable".into()
    }
}

const EXECUTION_BUDGET_MS: u64 = 250;

fn bounded_orchestrate(question: &str) -> Option<the_machine::router::OrchestratedAnswer> {
    let (sender, receiver) = mpsc::channel();
    let question = question.to_string();
    thread::spawn(move || {
        let _ = sender.send(QuestionRouter::orchestrate(&question));
    });
    receiver
        .recv_timeout(Duration::from_millis(EXECUTION_BUDGET_MS))
        .ok()
}

fn timeout_orchestration(question: &str) -> the_machine::router::OrchestratedAnswer {
    let mut result = QuestionRouter::orchestrate("");
    result.plan.goal = question.to_string();
    result.attempts.push(format!(
        "execution budget exhausted after {EXECUTION_BUDGET_MS} ms"
    ));
    result.answer = None;
    result.evidence.clear();
    result.verification = "fail-closed execution timeout".into();
    result.abstention_reason = Some(AbstentionReason::PlanExecutionFailed);
    result
}

fn no_signal_orchestration(
    question: &str,
    template: &the_machine::router::OrchestratedAnswer,
) -> the_machine::router::OrchestratedAnswer {
    let mut result = template.clone();
    result.plan.goal = question.to_string();
    result.attempts = vec!["no validated curriculum signal; shadow route skipped".into()];
    result.verification = "no curriculum route evaluated".into();
    result.abstention_reason = Some(AbstentionReason::InsufficientEvidence);
    result
}

fn first_failure(
    entry: &Value,
    orchestration: &the_machine::router::OrchestratedAnswer,
    correct: bool,
    has_signal: bool,
) -> FirstFailure {
    if orchestration.answer.is_some() {
        return if correct {
            FirstFailure::AuthorizedReferenceMatch
        } else {
            FirstFailure::AnswerEquivalence
        };
    }
    let question = entry.get("question").and_then(Value::as_str).unwrap_or("");
    if visual(entry, question) {
        return FirstFailure::VisualDependency;
    }
    if !has_signal {
        return FirstFailure::NoCurriculumSignal;
    }
    match orchestration.abstention_reason {
        Some(AbstentionReason::ProblemParseFailed)
        | Some(AbstentionReason::TargetNotIdentified)
        | Some(AbstentionReason::SymbolBindingFailed) => FirstFailure::LanguageNormalization,
        Some(AbstentionReason::MissingRequiredGiven)
        | Some(AbstentionReason::RequiredAssumptionMissing)
        | Some(AbstentionReason::RequiredAssumptionContradicted) => {
            FirstFailure::AssumptionsNotEstablished
        }
        Some(AbstentionReason::NoApplicableMethod) | Some(AbstentionReason::VerificationFailed) => {
            FirstFailure::MissingSpecialistTheorem
        }
        Some(AbstentionReason::UnsupportedDomain) => FirstFailure::UnsupportedTargetType,
        Some(AbstentionReason::SolverUnsupportedOperation) => FirstFailure::RepresentationGap,
        Some(AbstentionReason::IntermediateNotDerivable)
        | Some(AbstentionReason::IntermediateSemanticMismatch)
        | Some(AbstentionReason::IntermediateValueKindMismatch)
        | Some(AbstentionReason::IntermediateQualifierMismatch)
        | Some(AbstentionReason::IntermediateConstraintConflict)
        | Some(AbstentionReason::PlanCycleDetected)
        | Some(AbstentionReason::PlanDepthExceeded)
        | Some(AbstentionReason::PlanExecutionFailed)
        | Some(AbstentionReason::PlanVerificationFailed) => FirstFailure::PackBoundary,
        Some(AbstentionReason::InsufficientEvidence) => FirstFailure::MissingFactualPrerequisite,
        Some(AbstentionReason::AnswerFormatFailed) => FirstFailure::AnswerEquivalence,
        Some(AbstentionReason::ConflictingPlans)
        | Some(AbstentionReason::MultipleUnresolvedMethods) => FirstFailure::UnsupportedTargetType,
        Some(AbstentionReason::MissingAttachment) => FirstFailure::VisualDependency,
        None if orchestration.plan.problem.unresolved.is_empty() => FirstFailure::PackBoundary,
        None => FirstFailure::LanguageNormalization,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let trace_path = PathBuf::from(
        env::args()
            .nth(1)
            .unwrap_or_else(|| "/tmp/hle_curriculum_checkpoint_63.jsonl".into()),
    );
    let summary_path = PathBuf::from(
        env::args()
            .nth(2)
            .unwrap_or_else(|| "/tmp/hle_curriculum_checkpoint_63.summary.json".into()),
    );
    let checkpoint = env::var("MACHINE_CHECKPOINT").unwrap_or_else(|_| CHECKPOINT.to_string());
    let ontology_version =
        env::var("MACHINE_ONTOLOGY_VERSION").unwrap_or_else(|_| ONTOLOGY_VERSION.to_string());
    let bytes = fs::read(DATASET)?;
    let dataset_sha256 = sha256(&bytes);
    let manifest_before = the_machine::curriculum::breadth_first_manifest().replay_hash();
    let producer_commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    let curriculum_manifest_hash = the_machine::curriculum::breadth_first_manifest().replay_hash();
    let mut trace = File::create(&trace_path)?;
    let mut first_failures = BTreeMap::new();
    let mut pack_signals = BTreeMap::new();
    let mut cases = 0;
    let mut correct = 0;
    let mut incorrect = 0;
    let mut formalized_unsupported = 0;
    let mut candidates = 0;
    let mut invocations = 0;
    let mut replay_verified = 0;
    let mut replay_not_applicable = 0;
    let mut replay_not_recorded = 0;
    let mut replay_mismatch = 0;
    let mut timed_out = 0;
    let mut no_signal_short_circuits = 0;
    let no_signal_template = timeout_orchestration("");
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
        for signal in &detected {
            *pack_signals.entry(*signal).or_insert(0) += 1;
        }
        let started = Instant::now();
        let orchestration = if detected.is_empty() {
            no_signal_short_circuits += 1;
            no_signal_orchestration(question, &no_signal_template)
        } else {
            match bounded_orchestrate(question) {
                Some(result) => result,
                None => {
                    timed_out += 1;
                    timeout_orchestration(question)
                }
            }
        };
        let elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;
        let is_correct = orchestration
            .answer
            .as_deref()
            .is_some_and(|answer| QuestionRouter::exact_answers_match(answer, expected));
        let failure = first_failure(&entry, &orchestration, is_correct, !detected.is_empty());
        *first_failures.entry(failure).or_insert(0) += 1;
        if is_correct {
            correct += 1;
        } else if orchestration.answer.is_some() {
            incorrect += 1;
        }
        let pack_candidate = !detected.is_empty();
        candidates += usize::from(pack_candidate);
        // Curriculum packs remain shadow-only: no HLE text is promoted to a
        // typed pack invocation without a complete strict formalizer.
        let pack_invoked = false;
        invocations += usize::from(pack_invoked);
        let replay_result = replay(question, &orchestration);
        if replay_result == "verified" {
            replay_verified += 1;
        } else if replay_result == "not_applicable" {
            replay_not_applicable += 1;
        } else if replay_result == "not_recorded" {
            replay_not_recorded += 1;
        } else if replay_result == "mismatch" {
            replay_mismatch += 1;
        }
        if pack_candidate && orchestration.answer.is_none() {
            formalized_unsupported += usize::from(matches!(
                failure,
                FirstFailure::PackBoundary | FirstFailure::RepresentationGap
            ));
        }
        total_ms += elapsed_ms;
        max_ms = max_ms.max(elapsed_ms);
        let record = Record {
            id: entry.get("id").and_then(Value::as_str).map(str::to_string),
            question_sha256: sha256(question.as_bytes()),
            category: entry
                .get("category")
                .and_then(Value::as_str)
                .unwrap_or("uncategorized")
                .into(),
            question: question.into(),
            expected: expected.into(),
            signals: detected,
            first_failure: failure,
            curriculum_route: if pack_candidate {
                "shadow_candidate".into()
            } else {
                "none".into()
            },
            pack_invoked,
            candidate_answer: orchestration.answer.clone(),
            route_trace: orchestration.attempts.clone(),
            replay_result,
            registry_version: REGISTRY_VERSION.into(),
            ontology_version: ontology_version.clone(),
            receipt: json!({
                "domain": format!("{:?}", orchestration.plan.domain),
                "abstention_reason": orchestration.abstention_reason.map(|reason| format!("{reason:?}")),
                "verification": orchestration.verification,
                "shadow_only": true,
            }),
            execution_time_ms: elapsed_ms,
        };
        serde_json::to_writer(&mut trace, &record)?;
        writeln!(trace)?;
        cases += 1;
    }
    drop(trace);
    let trace_sha256 = sha256(&fs::read(&trace_path)?);
    let manifest_mutated =
        the_machine::curriculum::breadth_first_manifest().replay_hash() != manifest_before;
    let summary = Summary {
        checkpoint,
        producer_commit,
        dataset: DATASET,
        dataset_sha256,
        curriculum_manifest_hash,
        registry_version: REGISTRY_VERSION,
        ontology_version,
        cases,
        correct_authorized_answers: correct,
        incorrect_authorized_answers: incorrect,
        false_authorizations: incorrect,
        safely_formalized_but_unsupported: formalized_unsupported,
        pack_signals,
        first_failures,
        curriculum_candidates: candidates,
        pack_invocations: invocations,
        replay_verified,
        replay_not_applicable,
        replay_not_recorded,
        replay_mismatch,
        trace_sha256,
        manifest_mutated,
        execution_budget_ms: EXECUTION_BUDGET_MS,
        timed_out,
        no_signal_short_circuits,
        total_execution_time_ms: total_ms,
        max_execution_time_ms: max_ms,
        trace_path: trace_path.display().to_string(),
    };
    if let Some(parent) = summary_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&summary_path, serde_json::to_vec_pretty(&summary)?)?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}
