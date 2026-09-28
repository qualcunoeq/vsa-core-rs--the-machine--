//! Phase 9 — conversation evaluation.
//!
//! Turns representative conversation traces into versioned regression cases and
//! measures the *complete* application, including its failures. It owns:
//!
//! * the evaluation corpus format (regression and an untouched evaluation set);
//! * trace capture: a run of the corpus is snapshotted to a versioned trace
//!   file, and a later run is diffed against it for drift;
//! * the metrics the goal names — answer correctness, coverage, unsupported
//!   assertions, clarification success, context accuracy, correction
//!   propagation, latency, and memory;
//! * a strict separation between **oracle scoring** (whether a delivered
//!   answer is right, judged against gold labels) and **system self-rejection**
//!   (the system's own abstentions and clarifications counted on their own);
//! * paired ablations: the same corpus run with a mechanism enabled and
//!   disabled, reporting both sides and the delta.
//!
//! The module computes everything; `src/bin/machine_eval.rs` drives it and
//! writes the committed report artifacts.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::conversation::{ConversationService, EvalConfig};
use crate::conversation::{TurnResult, VerificationStatus};

/// Schema tag for trace and report artifacts.
pub const EVAL_SCHEMA: &str = "phase9-conversation-eval-v1";

// ─── Corpus ────────────────────────────────────────────────────────────────

/// Which side of the evaluation a case belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Split {
    /// Frozen regression cases. These gate the release.
    Regression,
    /// The untouched evaluation set. Reported, never used to tune, not gating.
    Holdout,
}

impl Split {
    pub fn label(self) -> &'static str {
        match self {
            Split::Regression => "regression",
            Split::Holdout => "holdout",
        }
    }
}

/// A full evaluation corpus file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Corpus {
    pub schema: String,
    pub split: Split,
    pub cases: Vec<EvalCase>,
}

/// One conversation: a sequence of turns, each with a gold expectation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvalCase {
    pub name: String,
    /// Topic tags, e.g. `correction`, `follow_up`, `unsupported`.
    #[serde(default)]
    pub tags: Vec<String>,
    pub steps: Vec<EvalStep>,
}

/// One turn and its gold expectation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvalStep {
    pub input: String,
    #[serde(default = "default_session")]
    pub session: String,
    /// Reopen the database after this turn, exercising durable reload.
    #[serde(default)]
    pub restart_after: bool,
    #[serde(default)]
    pub gold: Gold,
}

fn default_session() -> String {
    "main".to_string()
}

/// The gold expectation for one turn. Every field is optional; the metrics
/// that can be computed are exactly the ones the corpus declares.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Gold {
    /// The turn is expected to be answered.
    #[serde(default)]
    pub answered: Option<bool>,
    /// Substrings a correct answer must contain.
    #[serde(default)]
    pub answer_contains: Vec<String>,
    /// Substrings a correct answer must not contain.
    #[serde(default)]
    pub answer_not_contains: Vec<String>,
    /// The turn is expected to resolve ambiguity (clarification offered).
    #[serde(default)]
    pub clarification: Option<bool>,
    /// The turn is expected to depend on conversation context correctly; when
    /// true, a `follow_up` interpretation is required.
    #[serde(default)]
    pub context_dependent: bool,
    /// The turn corrects earlier knowledge; `stale_at_least` turns must be
    /// invalidated.
    #[serde(default)]
    pub stale_at_least: Option<usize>,
    /// The evidence must cite at least one item with this provenance substring.
    #[serde(default)]
    pub evidence_provenance_contains: Option<String>,
    /// Expected capability id, when the case pins one.
    #[serde(default)]
    pub capability: Option<String>,
}

impl Gold {
    pub fn is_empty(&self) -> bool {
        self.answered.is_none()
            && self.answer_contains.is_empty()
            && self.answer_not_contains.is_empty()
            && self.clarification.is_none()
            && !self.context_dependent
            && self.stale_at_least.is_none()
            && self.evidence_provenance_contains.is_none()
            && self.capability.is_none()
    }
}

impl Corpus {
    /// Parse a corpus from JSON text.
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|error| format!("corpus parse error: {error}"))
    }

    /// Parse a corpus from a file path.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, String> {
        let text = fs::read_to_string(path.as_ref())
            .map_err(|error| format!("could not read {}: {error}", path.as_ref().display()))?;
        Self::from_json(&text)
    }

    pub fn case_count(&self) -> usize {
        self.cases.len()
    }

    pub fn step_count(&self) -> usize {
        self.cases.iter().map(|case| case.steps.len()).sum()
    }
}

// ─── Trace ─────────────────────────────────────────────────────────────────

/// A snapshot of one turn, sufficient to detect behaviour drift without
/// reconstructing internal state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TurnTrace {
    pub case: String,
    pub step: usize,
    pub session: String,
    pub input: String,
    pub outcome: String,
    pub answer_text: String,
    /// The interpretation's request kind: `question`, `teaching`, `unsupported`.
    pub request_kind: String,
    #[serde(default)]
    pub answer_contains: Vec<String>,
    pub capability: String,
    pub evidence_count: usize,
    pub verification: String,
    pub follow_up_kind: Option<String>,
    pub follow_up_source: Option<String>,
    pub elapsed_ms: f64,
    pub fact_count: usize,
    pub rule_count: usize,
    #[serde(default)]
    pub stale_after: Option<usize>,
}

impl TurnTrace {
    fn from_result(
        case: &str,
        step: usize,
        session: &str,
        input: &str,
        result: &TurnResult,
        stale_after: Option<usize>,
    ) -> Self {
        let verification = match &result.verification {
            VerificationStatus::Verified { method, .. } => format!("verified:{method}"),
            VerificationStatus::Unverified { reason } => format!("unverified:{reason}"),
            VerificationStatus::NotAttempted => "not_attempted".to_string(),
        };
        let follow_up = result.interpretation.follow_up.as_ref();
        TurnTrace {
            case: case.to_string(),
            step,
            session: session.to_string(),
            input: input.to_string(),
            outcome: result.outcome.label().to_string(),
            answer_text: result.answer_text.clone(),
            request_kind: match result.interpretation.kind {
                crate::conversation::RequestKind::Question => "question",
                crate::conversation::RequestKind::Teaching => "teaching",
                crate::conversation::RequestKind::Unsupported => "unsupported",
            }
            .to_string(),
            answer_contains: Vec::new(),
            capability: result.capability.id(),
            evidence_count: result.evidence.len(),
            verification,
            follow_up_kind: follow_up.map(|info| info.kind.clone()),
            follow_up_source: follow_up.map(|info| info.source.clone()),
            elapsed_ms: result.timing.elapsed_ms,
            fact_count: result.diagnostics.fact_count_after,
            rule_count: result.diagnostics.rule_count_after,
            stale_after,
        }
    }
}

/// A captured run of a corpus.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Trace {
    pub schema: String,
    pub split: Split,
    pub turns: Vec<TurnTrace>,
}

impl Trace {
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|error| format!("trace parse error: {error}"))
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, String> {
        let text = fs::read_to_string(path.as_ref())
            .map_err(|error| format!("could not read {}: {error}", path.as_ref().display()))?;
        Self::from_json(&text)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("trace serializes")
    }

    /// Hash of the trace contents, normalized for volatile values (latency and
    /// freshly generated provenance ids), so the digest is stable across
    /// otherwise-identical runs.
    pub fn digest(&self) -> String {
        let stable: Vec<TurnTrace> = self
            .turns
            .iter()
            .map(|turn| {
                let mut turn = turn.clone();
                turn.elapsed_ms = 0.0;
                turn.answer_text = normalize_answer_text(&turn.answer_text);
                turn
            })
            .collect();
        format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&stable).expect("turns serialize"))
        )
    }
}

/// A single drift detected between two runs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Drift {
    pub case: String,
    pub step: usize,
    pub field: String,
    pub expected: String,
    pub actual: String,
}

/// Normalize volatile identifiers in answer text so drift detection reacts to
/// behavioural change, not to freshly generated provenance ids.
///
/// Source ids of the form `source-<32 hex>` (and bare 32-hex tokens) are
/// replaced with a stable placeholder.
pub fn normalize_answer_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if text[i..].starts_with("source-") {
            let start = i + "source-".len();
            let mut end = start;
            while end < bytes.len() && bytes[end].is_ascii_hexdigit() {
                end += 1;
            }
            if end - start >= 8 {
                out.push_str("source-<id>");
                i = end;
                continue;
            }
        }
        // A bare long hex token (assertion ids).
        if bytes[i].is_ascii_hexdigit() {
            let start = i;
            let mut end = i;
            while end < bytes.len() && bytes[end].is_ascii_hexdigit() {
                end += 1;
            }
            if end - start >= 16 {
                out.push_str("<id>");
                i = end;
                continue;
            }
        }
        let ch = text[i..].chars().next().expect("valid boundary");
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Compare two traces and report any drift. Latency is compared separately by
/// the caller and never treated as drift on its own (it is not deterministic).
pub fn detect_drift(expected: &Trace, actual: &Trace) -> Vec<Drift> {
    let mut drift = Vec::new();
    if expected.turns.len() != actual.turns.len() {
        drift.push(Drift {
            case: "<corpus>".to_string(),
            step: 0,
            field: "turn_count".to_string(),
            expected: expected.turns.len().to_string(),
            actual: actual.turns.len().to_string(),
        });
        return drift;
    }
    for (left, right) in expected.turns.iter().zip(actual.turns.iter()) {
        let mut push = |field: &str, expected: String, actual: String| {
            if expected != actual {
                drift.push(Drift {
                    case: left.case.clone(),
                    step: left.step,
                    field: field.to_string(),
                    expected,
                    actual,
                });
            }
        };
        push("input", left.input.clone(), right.input.clone());
        push("outcome", left.outcome.clone(), right.outcome.clone());
        push(
            "answer_text",
            normalize_answer_text(&left.answer_text),
            normalize_answer_text(&right.answer_text),
        );
        push("capability", left.capability.clone(), right.capability.clone());
        push(
            "evidence_count",
            left.evidence_count.to_string(),
            right.evidence_count.to_string(),
        );
        push("verification", left.verification.clone(), right.verification.clone());
        push(
            "follow_up_kind",
            format!("{:?}", left.follow_up_kind),
            format!("{:?}", right.follow_up_kind),
        );
        push(
            "follow_up_source",
            format!("{:?}", left.follow_up_source),
            format!("{:?}", right.follow_up_source),
        );
        push(
            "fact_count",
            left.fact_count.to_string(),
            right.fact_count.to_string(),
        );
        push(
            "rule_count",
            left.rule_count.to_string(),
            right.rule_count.to_string(),
        );
        push(
            "stale_after",
            format!("{:?}", left.stale_after),
            format!("{:?}", right.stale_after),
        );
    }
    drift
}

// ─── Metrics ───────────────────────────────────────────────────────────────

/// One evaluated turn: the trace plus the gold it was judged against.
#[derive(Clone, Debug)]
pub struct EvaluatedTurn {
    pub trace: TurnTrace,
    pub gold: Gold,
    /// Oracle judgement: true when a delivered answer is right, false when the
    /// delivered answer is wrong, `None` when the turn is not a deliverable
    /// answer (abstention/clarification) or the gold declares no answer.
    pub oracle_correct: Option<bool>,
    /// Whether this turn depended on conversation context and resolved it.
    pub context_correct: Option<bool>,
    /// Whether a correction invalidated the expected number of prior turns.
    pub correction_propagated: Option<bool>,
    /// Whether a clarification was offered and accepted as a follow-up.
    pub clarification_ok: Option<bool>,
}

/// Aggregate metrics over an evaluated run.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ConversationMetrics {
    pub turns: usize,
    pub cases: usize,

    // Answer coverage and delivery
    pub answered: usize,
    pub clarification_needed: usize,
    pub unsupported: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub coverage_rate: f64,

    // Oracle scoring (answers judged against gold)
    pub oracle_scored: usize,
    pub oracle_correct: usize,
    pub oracle_wrong: usize,
    pub oracle_wrong_rate: f64,

    // System self-rejection (the system's own abstentions, on their own)
    pub system_abstentions: usize,
    pub system_abstention_rate: f64,
    pub system_clarifications: usize,
    pub system_rejections_that_were_oracle_expected: usize,

    // Unsupported assertions: delivered answers with no supporting evidence
    pub unsupported_assertions: usize,
    pub unsupported_assertion_rate: f64,
    /// The hard ceiling: the evaluation fails if this is exceeded.
    pub unsupported_assertion_limit: usize,

    // Clarification success
    pub clarification_expected: usize,
    pub clarification_succeeded: usize,
    pub clarification_success_rate: f64,

    // Context accuracy
    pub context_expected: usize,
    pub context_accurate: usize,
    pub context_accuracy: f64,

    // Correction propagation
    pub corrections_expected: usize,
    pub corrections_propagated: usize,
    pub correction_propagation_rate: f64,

    // Latency (ms)
    pub latency_total_ms: f64,
    pub latency_mean_ms: f64,
    pub latency_p50_ms: f64,
    pub latency_p95_ms: f64,
    pub latency_max_ms: f64,

    // Memory (bytes; conversation-state estimate via reliability accounting)
    pub memory_total_bytes: usize,
    pub memory_budget_bytes: usize,
    pub memory_within_budget: bool,
}

fn percentile(sorted: &[f64], pct: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = ((pct / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

/// The declared ceiling on delivered answers with no supporting evidence.
pub const UNSUPPORTED_ASSERTION_LIMIT: usize = 0;

/// Compute metrics from evaluated turns.
pub fn compute_metrics(
    cases: usize,
    turns: &[EvaluatedTurn],
    memory_total_bytes: usize,
    memory_budget_bytes: usize,
) -> ConversationMetrics {
    let mut metrics = ConversationMetrics {
        turns: turns.len(),
        cases,
        unsupported_assertion_limit: UNSUPPORTED_ASSERTION_LIMIT,
        memory_total_bytes,
        memory_budget_bytes,
        memory_within_budget: memory_total_bytes <= memory_budget_bytes,
        ..Default::default()
    };

    let mut latencies: Vec<f64> = Vec::with_capacity(turns.len());
    for turn in turns {
        latencies.push(turn.trace.elapsed_ms);
        metrics.latency_total_ms += turn.trace.elapsed_ms;

        match turn.trace.outcome.as_str() {
            "answered" => metrics.answered += 1,
            "clarification_needed" => {
                metrics.clarification_needed += 1;
                metrics.system_abstentions += 1;
                metrics.system_clarifications += 1;
            }
            "unsupported" => {
                metrics.unsupported += 1;
                metrics.system_abstentions += 1;
            }
            "failed" => metrics.failed += 1,
            "cancelled" => metrics.cancelled += 1,
            _ => {}
        }

        // Oracle scoring: only deliverable answers with a declared gold verdict.
        if let Some(correct) = turn.oracle_correct {
            metrics.oracle_scored += 1;
            if correct {
                metrics.oracle_correct += 1;
            } else {
                metrics.oracle_wrong += 1;
            }
        }

        // A delivered answer with no supporting evidence is an unsupported
        // assertion. Teaching turns acknowledge storage rather than assert a
        // fact from evidence, and a clarification is not an assertion, so both
        // are excluded.
        if turn.trace.outcome == "answered"
            && turn.trace.request_kind == "question"
            && turn.trace.evidence_count == 0
        {
            metrics.unsupported_assertions += 1;
        }

        if let Some(ok) = turn.context_correct {
            metrics.context_expected += 1;
            if ok {
                metrics.context_accurate += 1;
            }
        }
        if let Some(ok) = turn.correction_propagated {
            metrics.corrections_expected += 1;
            if ok {
                metrics.corrections_propagated += 1;
            }
        }
        if let Some(ok) = turn.clarification_ok {
            metrics.clarification_expected += 1;
            if ok {
                metrics.clarification_succeeded += 1;
            }
        }

        // A system abstention where the gold expected an answer is a
        // self-rejection the corpus marks as undesirable. Counted separately
        // from oracle wrongness.
        if turn.trace.outcome != "answered" && turn.gold.answered == Some(true) {
            metrics.system_rejections_that_were_oracle_expected += 1;
        }
    }

    let n = turns.len().max(1) as f64;
    metrics.coverage_rate = metrics.answered as f64 / n;
    metrics.system_abstention_rate = metrics.system_abstentions as f64 / n;
    metrics.oracle_wrong_rate = if metrics.oracle_scored > 0 {
        metrics.oracle_wrong as f64 / metrics.oracle_scored as f64
    } else {
        0.0
    };
    metrics.unsupported_assertion_rate = metrics.unsupported_assertions as f64 / n;
    metrics.clarification_success_rate = if metrics.clarification_expected > 0 {
        metrics.clarification_succeeded as f64 / metrics.clarification_expected as f64
    } else {
        1.0
    };
    metrics.context_accuracy = if metrics.context_expected > 0 {
        metrics.context_accurate as f64 / metrics.context_expected as f64
    } else {
        1.0
    };
    metrics.correction_propagation_rate = if metrics.corrections_expected > 0 {
        metrics.corrections_propagated as f64 / metrics.corrections_expected as f64
    } else {
        1.0
    };

    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    metrics.latency_mean_ms = metrics.latency_total_ms / n;
    metrics.latency_p50_ms = percentile(&latencies, 50.0);
    metrics.latency_p95_ms = percentile(&latencies, 95.0);
    metrics.latency_max_ms = latencies.last().copied().unwrap_or(0.0);

    metrics
}

// ─── Running the corpus ────────────────────────────────────────────────────

/// Run a corpus against a fresh service per case and return evaluated turns
/// paired with their gold. `db_root` is a directory for per-case databases.
pub fn run_corpus(
    corpus: &Corpus,
    db_root: impl AsRef<Path>,
) -> Result<Vec<EvaluatedTurn>, String> {
    run_corpus_with(corpus, db_root, EvalConfig::default())
}

/// Run a corpus under an explicit [`EvalConfig`] (used by the ablations).
pub fn run_corpus_with(
    corpus: &Corpus,
    db_root: impl AsRef<Path>,
    eval: EvalConfig,
) -> Result<Vec<EvaluatedTurn>, String> {
    let db_root = db_root.as_ref();
    fs::create_dir_all(db_root).map_err(|error| format!("could not create db root: {error}"))?;

    let mut evaluated = Vec::new();
    for case in &corpus.cases {
        let db_path = db_root.join(format!("case_{}.db", sanitize(&case.name)));
        let _ = fs::remove_file(&db_path);
        let mut service = ConversationService::with_database(&db_path, None, None)?
            .with_eval_config(eval);

        // Track knowledge state per session so correction propagation can be
        // measured against the gold.
        let mut turns: Vec<EvaluatedTurn> = Vec::new();
        for (step_index, step) in case.steps.iter().enumerate() {
            let result = service.handle_turn(&step.session, &step.input);

            let stale_after = service
                .store()
                .session(&step.session)
                .map(|session| session.stale_count());

            let trace = TurnTrace::from_result(
                &case.name,
                step_index + 1,
                &step.session,
                &step.input,
                &result,
                stale_after,
            );

            let oracle_correct = judge_oracle(&trace, &step.gold);
            let context_correct = if step.gold.context_dependent {
                Some(
                    trace.follow_up_kind.is_some()
                        || trace.capability.starts_with("structured_solver"),
                )
            } else {
                None
            };
            let correction_propagated = step.gold.stale_at_least.map(|minimum| {
                stale_after.unwrap_or(0) >= minimum
            });
            let clarification_ok = step.gold.clarification.map(|expected| {
                (trace.outcome == "clarification_needed") == expected
            });

            turns.push(EvaluatedTurn {
                trace,
                gold: step.gold.clone(),
                oracle_correct,
                context_correct,
                correction_propagated,
                clarification_ok,
            });

            if step.restart_after {
                service = ConversationService::with_database(&db_path, None, None)?
                    .with_eval_config(eval);
            }
        }
        let _ = fs::remove_file(&db_path);
        evaluated.extend(turns);
    }
    Ok(evaluated)
}

/// Oracle judgement for a delivered answer. Returns `None` when the turn did
/// not deliver an answer or the gold declares no answer expectation.
fn judge_oracle(trace: &TurnTrace, gold: &Gold) -> Option<bool> {
    let expects_answer = gold.answered == Some(true)
        || !gold.answer_contains.is_empty()
        || !gold.answer_not_contains.is_empty()
        || gold.capability.is_some();
    if !expects_answer {
        return None;
    }
    // An abstention where an answer was expected is judged by the harness as a
    // self-rejection in `compute_metrics`, not as an oracle correctness value.
    if trace.outcome != "answered" {
        return None;
    }
    let mut correct = true;
    for needle in &gold.answer_contains {
        if !trace.answer_text.contains(needle) {
            correct = false;
        }
    }
    for needle in &gold.answer_not_contains {
        if trace.answer_text.contains(needle) {
            correct = false;
        }
    }
    if let Some(capability) = &gold.capability {
        if &trace.capability != capability {
            correct = false;
        }
    }
    if let Some(needle) = &gold.evidence_provenance_contains {
        // Evidence provenance is not carried in the trace by default; when a
        // case pins it the harness records a miss unless the answer itself
        // names the provenance.
        if !trace.answer_text.contains(needle) {
            correct = false;
        }
    }
    Some(correct)
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

// ─── Ablations ─────────────────────────────────────────────────────────────

/// The paired result of one ablation: the same corpus under the production
/// config and with one mechanism disabled.
#[derive(Clone, Debug, Serialize)]
pub struct AblationComparison {
    /// The mechanism that was disabled (e.g. `vsa_retrieval`).
    pub mechanism: String,
    pub enabled: ConversationMetrics,
    pub disabled: ConversationMetrics,
    /// disabled − enabled for each metric that has a signed meaning.
    pub deltas: BTreeMap<String, f64>,
    /// Whether the disabled run preserved the safety contract (no unsupported
    /// assertions beyond the limit).
    pub safety_preserved: bool,
}

/// Build a comparison from two metric sets.
pub fn compare_ablation(
    mechanism: &str,
    enabled: ConversationMetrics,
    disabled: ConversationMetrics,
) -> AblationComparison {
    let mut deltas = BTreeMap::new();
    let diff = |a: f64, b: f64| b - a;
    deltas.insert("coverage_rate".to_string(), diff(enabled.coverage_rate, disabled.coverage_rate));
    deltas.insert(
        "oracle_wrong_rate".to_string(),
        diff(enabled.oracle_wrong_rate, disabled.oracle_wrong_rate),
    );
    deltas.insert(
        "unsupported_assertions".to_string(),
        diff(
            enabled.unsupported_assertions as f64,
            disabled.unsupported_assertions as f64,
        ),
    );
    deltas.insert(
        "system_abstention_rate".to_string(),
        diff(enabled.system_abstention_rate, disabled.system_abstention_rate),
    );
    deltas.insert(
        "clarification_success_rate".to_string(),
        diff(enabled.clarification_success_rate, disabled.clarification_success_rate),
    );
    deltas.insert("context_accuracy".to_string(), diff(enabled.context_accuracy, disabled.context_accuracy));
    deltas.insert(
        "correction_propagation_rate".to_string(),
        diff(enabled.correction_propagation_rate, disabled.correction_propagation_rate),
    );
    deltas.insert("latency_mean_ms".to_string(), diff(enabled.latency_mean_ms, disabled.latency_mean_ms));

    let safety_preserved =
        disabled.unsupported_assertions <= disabled.unsupported_assertion_limit;

    AblationComparison {
        mechanism: mechanism.to_string(),
        enabled,
        disabled,
        deltas,
        safety_preserved,
    }
}

/// The named mechanisms the harness ablates, in report order.
pub const ABLATION_MECHANISMS: [&str; 6] = [
    "vsa_retrieval",
    "context_retrieval",
    "typed_capabilities",
    "reuse",
    "semantic_worker",
    "consolidation",
];

/// Run every ablation paired on the same corpus.
pub fn run_ablations(
    corpus: &Corpus,
    db_root: impl AsRef<Path>,
    memory_budget_bytes: usize,
) -> Result<Vec<AblationComparison>, String> {
    let production = EvalConfig::production();
    let enabled_turns = run_corpus_with(corpus, db_root.as_ref(), production)?;
    let enabled_memory = estimate_memory(&enabled_turns, memory_budget_bytes);
    let enabled = compute_metrics(corpus.case_count(), &enabled_turns, enabled_memory.0, memory_budget_bytes);

    let mut comparisons = Vec::new();
    for mechanism in ABLATION_MECHANISMS {
        let config = EvalConfig::disabled(mechanism)
            .ok_or_else(|| format!("unknown ablation mechanism {mechanism}"))?;
        let turns = run_corpus_with(corpus, db_root.as_ref(), config)?;
        let memory = estimate_memory(&turns, memory_budget_bytes);
        let metrics = compute_metrics(corpus.case_count(), &turns, memory.0, memory_budget_bytes);
        comparisons.push(compare_ablation(mechanism, enabled.clone(), metrics));
    }
    Ok(comparisons)
}

/// Estimate the conversation-state memory footprint of a run using the Phase 7
/// accounting. Returns `(total_bytes, conversation_bytes)`.
pub fn estimate_memory(turns: &[EvaluatedTurn], budget_bytes: usize) -> (usize, usize) {
    use std::collections::BTreeSet;
    let sessions: BTreeSet<(&str, &str)> = turns
        .iter()
        .map(|turn| (turn.trace.case.as_str(), turn.trace.session.as_str()))
        .collect();
    let mut report = crate::reliability::MemoryReport {
        budget_bytes,
        ..Default::default()
    };
    crate::reliability::account_conversation(
        &mut report,
        sessions.len(),
        turns.len(),
        0,
        turns.len(),
    );
    (report.total_bytes, report.conversation_bytes)
}

// ─── Report ────────────────────────────────────────────────────────────────

/// The full Phase 9 report.
#[derive(Clone, Debug, Serialize)]
pub struct ConversationEvalReport {
    pub schema: String,
    pub regression: ConversationMetrics,
    pub holdout: ConversationMetrics,
    pub regression_drift: Vec<Drift>,
    pub ablations: Vec<AblationComparison>,
    pub trace_sha256: String,
    pub holdout_trace_sha256: String,
}

impl ConversationEvalReport {
    /// Whether the report's held contracts pass.
    ///
    /// * the regression run must have no drift against its frozen trace;
    /// * no run may exceed the unsupported-assertion limit;
    /// * every ablation must preserve safety.
    pub fn contracts_hold(&self) -> Result<(), String> {
        if !self.regression_drift.is_empty() {
            return Err(format!(
                "regression trace drifted in {} place(s)",
                self.regression_drift.len()
            ));
        }
        if self.regression.unsupported_assertions > self.regression.unsupported_assertion_limit {
            return Err(format!(
                "regression run delivered {} unsupported assertion(s), limit {}",
                self.regression.unsupported_assertions, self.regression.unsupported_assertion_limit
            ));
        }
        if self.holdout.unsupported_assertions > self.holdout.unsupported_assertion_limit {
            return Err(format!(
                "holdout run delivered {} unsupported assertion(s), limit {}",
                self.holdout.unsupported_assertions, self.holdout.unsupported_assertion_limit
            ));
        }
        for ablation in &self.ablations {
            if !ablation.safety_preserved {
                return Err(format!(
                    "ablation {} did not preserve safety (unsupported assertions {})",
                    ablation.mechanism, ablation.disabled.unsupported_assertions
                ));
            }
        }
        Ok(())
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("report serializes")
    }

    /// Render the human-readable markdown report.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Phase 9 — conversation evaluation report\n\n");
        out.push_str("Generated by `cargo run --bin machine_eval`. Measures the \
complete application on representative conversation traces, including its \
failures. Oracle scoring (answers judged against gold) is reported separately \
from system self-rejection (the system's own abstentions).\n\n");

        out.push_str("## Runs\n\n");
        out.push_str("| metric | regression | holdout |\n|---|---|---|\n");
        let row = |name: &str, a: String, b: String| format!("| {name} | {a} | {b} |\n");
        let r = &self.regression;
        let h = &self.holdout;
        out.push_str(&row("turns", r.turns.to_string(), h.turns.to_string()));
        out.push_str(&row("cases", r.cases.to_string(), h.cases.to_string()));
        out.push_str(&row("answered", r.answered.to_string(), h.answered.to_string()));
        out.push_str(&row(
            "coverage rate",
            format!("{:.3}", r.coverage_rate),
            format!("{:.3}", h.coverage_rate),
        ));
        out.push_str(&row(
            "oracle scored / correct / wrong",
            format!("{} / {} / {}", r.oracle_scored, r.oracle_correct, r.oracle_wrong),
            format!("{} / {} / {}", h.oracle_scored, h.oracle_correct, h.oracle_wrong),
        ));
        out.push_str(&row(
            "oracle wrong rate",
            format!("{:.3}", r.oracle_wrong_rate),
            format!("{:.3}", h.oracle_wrong_rate),
        ));
        out.push_str(&row(
            "system abstentions",
            format!("{} ({:.3})", r.system_abstentions, r.system_abstention_rate),
            format!("{} ({:.3})", h.system_abstentions, h.system_abstention_rate),
        ));
        out.push_str(&row(
            "self-rejections when an answer was expected",
            r.system_rejections_that_were_oracle_expected.to_string(),
            h.system_rejections_that_were_oracle_expected.to_string(),
        ));
        out.push_str(&row(
            "unsupported assertions",
            format!("{} (limit {})", r.unsupported_assertions, r.unsupported_assertion_limit),
            format!("{} (limit {})", h.unsupported_assertions, h.unsupported_assertion_limit),
        ));
        out.push_str(&row(
            "clarification success",
            format!("{}/{} ({:.3})", r.clarification_succeeded, r.clarification_expected, r.clarification_success_rate),
            format!("{}/{} ({:.3})", h.clarification_succeeded, h.clarification_expected, h.clarification_success_rate),
        ));
        out.push_str(&row(
            "context accuracy",
            format!("{}/{} ({:.3})", r.context_accurate, r.context_expected, r.context_accuracy),
            format!("{}/{} ({:.3})", h.context_accurate, h.context_expected, h.context_accuracy),
        ));
        out.push_str(&row(
            "correction propagation",
            format!("{}/{} ({:.3})", r.corrections_propagated, r.corrections_expected, r.correction_propagation_rate),
            format!("{}/{} ({:.3})", h.corrections_propagated, h.corrections_expected, h.correction_propagation_rate),
        ));
        out.push_str(&row(
            "latency mean / p50 / p95 / max (ms)",
            format!("{:.2} / {:.2} / {:.2} / {:.2}", r.latency_mean_ms, r.latency_p50_ms, r.latency_p95_ms, r.latency_max_ms),
            format!("{:.2} / {:.2} / {:.2} / {:.2}", h.latency_mean_ms, h.latency_p50_ms, h.latency_p95_ms, h.latency_max_ms),
        ));
        out.push_str(&row(
            "memory bytes / budget",
            format!("{} / {} (within: {})", r.memory_total_bytes, r.memory_budget_bytes, r.memory_within_budget),
            format!("{} / {} (within: {})", h.memory_total_bytes, h.memory_budget_bytes, h.memory_within_budget),
        ));

        out.push_str("\n## Trace integrity\n\n");
        out.push_str(&format!("* regression trace SHA-256: `{}`\n", self.trace_sha256));
        out.push_str(&format!("* holdout trace SHA-256: `{}`\n", self.holdout_trace_sha256));
        out.push_str(&format!("* regression drift vs frozen trace: {}\n", self.regression_drift.len()));

        out.push_str("\n## Paired ablations\n\n");
        out.push_str("Each mechanism is disabled in turn while the same corpus runs. \
Deltas are `disabled − enabled` over the regression set.\n\n");
        out.push_str("| mechanism | coverage Δ | oracle-wrong Δ | unsupported Δ | abstention Δ | context Δ | correction Δ | latency Δ (ms) | safety |\n");
        out.push_str("|---|---|---|---|---|---|---|---|---|\n");
        for ablation in &self.ablations {
            let d = &ablation.deltas;
            let g = |key: &str| d.get(key).copied().unwrap_or(0.0);
            out.push_str(&format!(
                "| {} | {:+.3} | {:+.3} | {:+.0} | {:+.3} | {:+.3} | {:+.3} | {:+.2} | {} |\n",
                ablation.mechanism,
                g("coverage_rate"),
                g("oracle_wrong_rate"),
                g("unsupported_assertions"),
                g("system_abstention_rate"),
                g("context_accuracy"),
                g("correction_propagation_rate"),
                g("latency_mean_ms"),
                if ablation.safety_preserved { "preserved" } else { "VIOLATED" },
            ));
        }

        out.push_str("\n## Interpretation\n\n");
        out.push_str("* A wrong delivered answer and a system abstention are different \
outcomes: the first is an oracle error, the second is self-rejection. They are \
never summed.\n");
        out.push_str("* Coverage is how often the system answers at all; a high \
coverage with a high oracle-wrong rate is not success.\n");
        out.push_str("* The holdout set is reported but never gated: it exists to be \
untouched.\n");
        out
    }
}

// ─── Artifacts ─────────────────────────────────────────────────────────────

/// Write a trace to disk as JSON.
pub fn write_trace(trace: &Trace, path: impl AsRef<Path>) -> Result<(), String> {
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent).map_err(|error| format!("could not create {parent:?}: {error}"))?;
    }
    fs::write(path.as_ref(), format!("{}\n", trace.to_json()))
        .map_err(|error| format!("could not write {}: {error}", path.as_ref().display()))
}

/// Capture a trace from a corpus under the production config.
pub fn capture_trace(corpus: &Corpus, db_root: impl AsRef<Path>) -> Result<Trace, String> {
    let turns = run_corpus(corpus, db_root)?;
    Ok(Trace {
        schema: EVAL_SCHEMA.to_string(),
        split: corpus.split,
        turns: turns.into_iter().map(|turn| turn.trace).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_corpus() -> Corpus {
        Corpus::from_json(
            r#"{
              "schema": "phase9-conversation-eval-v1",
              "split": "regression",
              "cases": [
                {
                  "name": "observatory",
                  "tags": ["correction", "follow_up"],
                  "steps": [
                    { "input": "Alice manages the observatory.",
                      "gold": { "answered": true, "answer_contains": ["Alice"] } },
                    { "input": "Who manages the observatory?",
                      "gold": { "answered": true, "answer_contains": ["Alice"], "answer_not_contains": ["Bob"] } },
                    { "input": "Actually, Bob manages it now.",
                      "gold": { "answered": true, "stale_at_least": 1 } },
                    { "input": "Who manages the observatory?",
                      "gold": { "answered": true, "answer_contains": ["Bob"] } }
                  ]
                }
              ]
            }"#,
        )
        .expect("sample corpus parses")
    }

    #[test]
    fn corpus_parses_and_counts() {
        let corpus = sample_corpus();
        assert_eq!(corpus.case_count(), 1);
        assert_eq!(corpus.step_count(), 4);
        assert_eq!(corpus.split, Split::Regression);
    }

    #[test]
    fn running_the_corpus_scores_answers_against_gold() {
        let dir = std::env::temp_dir().join("phase9_eval_unit_run");
        let corpus = sample_corpus();
        let turns = run_corpus(&corpus, &dir).expect("corpus runs");
        assert_eq!(turns.len(), 4);
        // Turn 2 must be judged correct against its gold.
        assert_eq!(turns[1].oracle_correct, Some(true));
        // The correction must invalidate at least one prior turn.
        assert_eq!(turns[2].correction_propagated, Some(true));

        let (memory, _) = estimate_memory(&turns, 8 * 1024 * 1024);
        let metrics = compute_metrics(corpus.case_count(), &turns, memory, 8 * 1024 * 1024);
        assert_eq!(metrics.turns, 4);
        assert!(metrics.answered >= 3, "{metrics:?}");
        assert_eq!(metrics.unsupported_assertions, 0);
        // No delivered answer lacked evidence here, so the safety contract holds.
        assert!(metrics.unsupported_assertions <= metrics.unsupported_assertion_limit);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn oracle_scoring_is_separate_from_system_rejection() {
        // A conversation where the system abstains on an expected answer: the
        // turn must not be counted as an oracle error, only as a self-rejection.
        let corpus = Corpus::from_json(
            r#"{
              "schema": "phase9-conversation-eval-v1",
              "split": "regression",
              "cases": [
                { "name": "abstain",
                  "steps": [
                    { "input": "Who manages the observatory?",
                      "gold": { "answered": true, "answer_contains": ["Alice"] } }
                  ] }
              ]
            }"#,
        )
        .expect("corpus parses");
        let dir = std::env::temp_dir().join("phase9_eval_unit_abstain");
        let turns = run_corpus(&corpus, &dir).expect("corpus runs");
        assert_eq!(turns[0].oracle_correct, None, "an abstention is not an oracle verdict");
        let (memory, _) = estimate_memory(&turns, 1024 * 1024);
        let metrics = compute_metrics(corpus.case_count(), &turns, memory, 1024 * 1024);
        assert_eq!(metrics.oracle_scored, 0);
        assert_eq!(metrics.oracle_wrong, 0);
        assert_eq!(metrics.system_rejections_that_were_oracle_expected, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn trace_capture_and_drift_detection() {
        let dir = std::env::temp_dir().join("phase9_eval_unit_trace");
        let corpus = sample_corpus();
        let first = capture_trace(&corpus, &dir).expect("capture");
        let second = capture_trace(&corpus, &dir).expect("recapture");
        let d = detect_drift(&first, &second); assert!(d.is_empty(), "deterministic run drifted: {d:?}");

        let mut mutated = first.clone();
        mutated.turns[0].outcome = "unsupported".to_string();
        let drift = detect_drift(&first, &mutated);
        assert_eq!(drift.len(), 1);
        assert_eq!(drift[0].field, "outcome");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn paired_ablation_reports_both_sides() {
        let dir = std::env::temp_dir().join("phase9_eval_unit_ablation");
        let corpus = sample_corpus();
        let comparisons = run_ablations(&corpus, &dir, 8 * 1024 * 1024).expect("ablations run");
        assert_eq!(comparisons.len(), ABLATION_MECHANISMS.len());
        for comparison in &comparisons {
            assert_eq!(comparison.enabled.turns, comparison.disabled.turns);
            assert!(comparison.deltas.contains_key("coverage_rate"));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn contracts_fail_on_drift() {
        let report = ConversationEvalReport {
            schema: EVAL_SCHEMA.to_string(),
            regression: ConversationMetrics::default(),
            holdout: ConversationMetrics::default(),
            regression_drift: vec![Drift {
                case: "x".to_string(),
                step: 1,
                field: "outcome".to_string(),
                expected: "answered".to_string(),
                actual: "unsupported".to_string(),
            }],
            ablations: Vec::new(),
            trace_sha256: "0".to_string(),
            holdout_trace_sha256: "0".to_string(),
        };
        assert!(report.contracts_hold().is_err());
    }
}

