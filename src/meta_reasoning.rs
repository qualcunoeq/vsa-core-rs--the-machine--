// ─── Meta-Reasoning Layer ─────────────────────────────────────────────────
//
// The judgment layer that decides whether The Machine is confident, uncertain,
// or stuck — and what to do in each case.
//
// This is the core of autonomous problem solving.  Given a problem statement
// (error text, system state description), the `assess` function classifies
// it into one of three states:
//
//   Confident — a rule matches, a plan is available, confidence > 0.70.
//                Execute the plan directly.
//
//   Uncertain — no confident plan, but structural analogy or centroid
//               proximity suggests a hypothesis.  Test the best hypothesis
//               by gathering more information.
//
//   Stuck — no rule, no analogy, no centroid match.  Acquire knowledge
//           by fetching documentation or running diagnostic commands.
//
// The `assess` function is the meta-judgment call.  It is called each
// iteration of the autonomous loop to determine what to do next.
//
// Stage 1 of the autonomy build.  No execution — just judgment.
// ────────────────────────────────────────────────────────────────────────────

use crate::abstraction_learner::AbstractionLearner;
use crate::actuator::{ActionRequest, ActionType, JumpBoxActuator};
use crate::diagnostic::{
    absorb_diagnosis_with_learner, classify_structural_with_learner, parse_error_structure,
    parse_error_structure_with_learner, query_diagnostic_category_with_learner, CanonicalSvo,
    ErrorClassifier,
};
use crate::qa::{PlanStep, QaEngine, RuleConfidenceUpdate};
use crate::text_encoder;
use crate::Hypervector;
use crate::VSABrain;

/// Hard safety cap for the autonomous loop.  Callers still choose a smaller
/// budget, but oversized requests are bounded to keep research runs finite.
pub const MAX_AUTONOMOUS_ITERATIONS: usize = 128;

/// Hard cap on how many plan steps can be attempted in one iteration.
/// The planner is depth-limited today, but this keeps the executor bounded
/// if future planners produce wider plans.
pub const MAX_PLAN_STEPS_PER_ITERATION: usize = 16;

/// Stop autonomous solving after this many consecutive cycles produce no new
/// observations.  This prevents uncertain/stuck loops from burning the full
/// iteration budget when probes or documentation acquisition add no evidence.
pub const MAX_NO_PROGRESS_ITERATIONS: usize = 3;

// ─── Types ─────────────────────────────────────────────────────────────────

/// Deterministic explanation for a plan confidence calculation.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanConfidenceReport {
    /// Product of all normalized step confidences.
    pub confidence: f64,
    /// Number of plan steps included in the product.
    pub step_count: usize,
    /// The lowest-confidence step, if the plan is non-empty.
    pub weakest_step: Option<usize>,
    /// Normalized per-step confidence factors in execution order.
    pub factors: Vec<f64>,
}

impl PlanConfidenceReport {
    /// Stable compact string for logs and tests.
    pub fn summary(&self) -> String {
        let factors = self
            .factors
            .iter()
            .map(|factor| format!("{:.3}", factor))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "confidence={:.3}; steps={}; weakest={}; factors=[{}]",
            self.confidence,
            self.step_count,
            self.weakest_step
                .map(|idx| idx.to_string())
                .unwrap_or_else(|| "none".to_string()),
            factors
        )
    }
}

/// The source of a hypothesis — how The Machine arrived at this explanation.
#[derive(Clone, Debug, PartialEq)]
pub enum HypothesisSource {
    /// Direct rule match (Level 1 trigger or Level 2 trigram).
    DirectRule,
    /// Same SVO structure as a known problem (Level 3 structural).
    StructuralAnalogy,
    /// Near a learned centroid in VSA space.
    CentroidProximity,
    /// Weak trigram match below the usual threshold.
    WeakTrigram,
}

/// A candidate explanation for a problem the system hasn't seen before.
#[derive(Clone, Debug)]
pub struct Hypothesis {
    /// The hypothesized diagnostic category (e.g., "port_conflict").
    pub category: String,
    /// How this hypothesis was formed.
    pub source: HypothesisSource,
    /// Confidence in this hypothesis (0.0 = none, 1.0 = certain).
    pub confidence: f64,
    /// The structural SVO triples that led to this hypothesis (if any).
    pub structural_triples: Vec<CanonicalSvo>,
    /// Description of what to test to confirm or refute this hypothesis.
    pub test_description: String,
}

/// The system's judgment about a problem at this moment.
#[derive(Clone, Debug)]
pub enum ReasoningState {
    /// Confident: a plan is available with high confidence. Execute it.
    Confident {
        plan: Vec<PlanStep>,
        confidence: f64,
        category: String,
    },
    /// Uncertain: hypotheses exist but none is confident. Test the best one.
    Uncertain {
        hypotheses: Vec<Hypothesis>,
        best_confidence: f64,
        problem: String,
    },
    /// Stuck: no rule, no analogy, no centroid. Acquire knowledge.
    Stuck { problem: String, tried: Vec<String> },
}

impl ReasoningState {
    /// A short name for logging.
    pub fn name(&self) -> &'static str {
        match self {
            ReasoningState::Confident { .. } => "confident",
            ReasoningState::Uncertain { .. } => "uncertain",
            ReasoningState::Stuck { .. } => "stuck",
        }
    }
}

impl std::fmt::Display for ReasoningState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReasoningState::Confident {
                plan,
                confidence,
                category,
            } => {
                write!(
                    f,
                    "Confident(category={}, confidence={:.2}, plan={} steps)",
                    category,
                    confidence,
                    plan.len()
                )
            }
            ReasoningState::Uncertain {
                hypotheses,
                best_confidence,
                problem,
            } => {
                write!(
                    f,
                    "Uncertain(problem=\"{}\", best_confidence={:.2}, hypotheses={})",
                    problem,
                    best_confidence,
                    hypotheses.len()
                )
            }
            ReasoningState::Stuck { problem, tried } => {
                write!(f, "Stuck(problem=\"{}\", tried={})", problem, tried.len())
            }
        }
    }
}

// ─── The Assessment Function ───────────────────────────────────────────────

/// Assess a problem and return the system's current reasoning state.
///
/// This is the core meta-judgment call.  It tries four levels of analysis
/// in order:
///
///   1-2. Classifier (trigger + trigram): fast, confident when available
///   3.    Structural parser: maps surface form to abstract SVO triples
///   4.    Centroid proximity: checks VSA brain for nearby known problems
///
/// Each level feeds into the next.  If a confident plan is found, return it.
/// If partial evidence exists, return Uncertain with hypotheses.
/// If nothing matches, return Stuck.
/// Thread-safe reference to an AbstractionLearner for assessment.
/// Pass `None` if no learner is available (learner not yet created or
/// assessment-only path without learning).
pub fn assess(
    problem: &str,
    brain: &VSABrain,
    qa: &QaEngine,
    classifier: &ErrorClassifier,
) -> ReasoningState {
    assess_with_learner(problem, brain, qa, classifier, None)
}

/// Like `assess`, but uses the learner's promoted keyword mappings for
/// structural parsing.  Call this from `solve_autonomously` so that
/// learned mappings (e.g., "broker" → network_service) affect future
/// assessments.
pub fn assess_with_learner(
    problem: &str,
    brain: &VSABrain,
    qa: &QaEngine,
    classifier: &ErrorClassifier,
    learner: Option<&AbstractionLearner>,
) -> ReasoningState {
    // ── Level 1-2: Classifier (trigger + trigram Jaccard) ───────────────
    if let (Some(canonical), _level) = classifier.classify_deep(problem) {
        let (_subj, _verb, obj) = canonical.clone();
        let plan = qa.plan_for_goal("service", "is", "running", 5);

        if !plan.is_empty() {
            let confidence = plan_confidence(&plan);
            if confidence >= 0.70 {
                return ReasoningState::Confident {
                    plan,
                    confidence,
                    category: obj,
                };
            }
        }
    }

    // ── Level 3: Structural parser (with learner extensions if available) ─
    let struct_triples = classify_structural_with_learner(problem, learner);
    if let Some(ref triples) = struct_triples {
        // Check if any structural triple has a matching abstract rule
        if let Some(category) = find_best_structural_category(triples, qa) {
            let plan = qa.plan_for_goal("service", "is", "running", 5);
            let base_confidence = if plan.is_empty() { 0.50 } else { 0.65 };

            let hypothesis = Hypothesis {
                category: category.clone(),
                source: HypothesisSource::StructuralAnalogy,
                confidence: base_confidence,
                structural_triples: triples.clone(),
                test_description: format!(
                    "Check resource availability based on structural parse of '{}'",
                    problem
                ),
            };

            return ReasoningState::Uncertain {
                hypotheses: vec![hypothesis],
                best_confidence: base_confidence,
                problem: problem.to_string(),
            };
        }
    }

    // ── Level 4: Structural SVO centroid query (learner-aware) ──────────
    // Uses the same structural SVO encoding as query_diagnostic_category
    // with the learner's promoted keyword mappings.  This is the primary
    // generalization path for learned abstractions.
    if let Some((category, conf)) = query_diagnostic_category_with_learner(brain, problem, learner)
    {
        if conf >= 0.55 {
            let hypothesis = Hypothesis {
                category: category.clone(),
                source: HypothesisSource::CentroidProximity,
                confidence: conf,
                structural_triples: struct_triples.unwrap_or_default(),
                test_description: format!(
                    "Structural SVO centroid: {} (conf={:.2})",
                    category, conf
                ),
            };
            return ReasoningState::Uncertain {
                hypotheses: vec![hypothesis],
                best_confidence: conf,
                problem: problem.to_string(),
            };
        }
    }

    // ── Genuinely stuck ────────────────────────────────────────────────
    ReasoningState::Stuck {
        problem: problem.to_string(),
        tried: vec![
            "classifier".to_string(),
            "structural".to_string(),
            "centroid".to_string(),
        ],
    }
}

/// Compute the overall confidence of a plan (product of step confidences).
fn plan_confidence(plan: &[PlanStep]) -> f64 {
    explain_plan_confidence(plan).confidence
}

/// Explain the overall confidence of a plan with deterministic factors.
pub fn explain_plan_confidence(plan: &[PlanStep]) -> PlanConfidenceReport {
    let factors: Vec<f64> = plan
        .iter()
        .map(|step| normalized_confidence(step.confidence))
        .collect();
    let confidence = factors.iter().fold(1.0, |acc, factor| acc * factor);
    let weakest_step = factors
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.total_cmp(b.1).then_with(|| a.0.cmp(&b.0)))
        .map(|(idx, _)| idx);

    PlanConfidenceReport {
        confidence,
        step_count: plan.len(),
        weakest_step,
        factors,
    }
}

/// Given structural triples, find the best-matching diagnostic category
/// by checking which abstract rules would fire.
fn find_best_structural_category(triples: &[CanonicalSvo], _qa: &QaEngine) -> Option<String> {
    // Categories we know about, mapped from abstract state triples
    let category_patterns: &[(&[(&str, &str, &str)], &str)] = &[
        // port_conflict: process accessing network_service, and it's unavailable
        (
            &[
                ("process", "accesses", "network_service"),
                ("network_service", "has_state", "unavailable"),
            ],
            "port_conflict",
        ),
        // network_service unavailable alone (no action parsed) → connection_refused
        (
            &[("network_service", "has_state", "unavailable")],
            "connection_refused",
        ),
        // missing_file: process accessing file_system, missing
        (
            &[
                ("process", "accesses", "file_system"),
                ("file_system", "has_state", "unavailable"),
            ],
            "missing_file",
        ),
        // missing_file: file_system has_state not_found
        (&[("file_system", "has_state", "not_found")], "missing_file"),
        (
            &[("file_system", "has_state", "resource_missing")],
            "missing_file",
        ),
        // permission_denied: process accessing something, permission blocked
        (
            &[("file_system", "has_state", "permission_blocked")],
            "permission_denied",
        ),
        (
            &[("network_service", "has_state", "permission_blocked")],
            "permission_denied",
        ),
        // disk_full: storage capacity exhausted
        (
            &[("storage", "has_state", "capacity_exhausted")],
            "disk_full",
        ),
        (&[("storage", "has_state", "unavailable")], "disk_full"),
        // credential_invalid: credential has_state invalid (e.g., expired cert, bad token)
        (
            &[("credential", "has_state", "credential_invalid")],
            "credential_invalid",
        ),
        // Generic resource access failure
        (&[("process", "accesses", "storage")], "disk_full"),
        (&[("process", "accesses", "cache_resource")], "disk_full"),
        (&[("process", "accesses", "store_resource")], "disk_full"),
    ];

    for (patterns, category) in category_patterns {
        let all_match = patterns
            .iter()
            .all(|(s, v, o)| triples.contains(&(s.to_string(), v.to_string(), o.to_string())));
        if all_match {
            return Some(category.to_string());
        }
    }

    // Fallback: just use the first abstract triple's object if it exists
    for (s, v, o) in triples {
        if s == "process" && v == "accesses" {
            return Some(format!("resource_access_{}", o));
        }
    }

    None
}

/// Generate a test action description for a hypothesis.
/// Uses the learner's keyword extensions if available.
pub fn generate_test_action(problem: &str) -> String {
    generate_test_action_with_learner(problem, None)
}

fn generate_test_action_with_learner(
    problem: &str,
    learner: Option<&AbstractionLearner>,
) -> String {
    let structure = match learner {
        Some(l) => parse_error_structure_with_learner(problem, l),
        None => parse_error_structure(problem),
    };
    if let Some(ref res) = structure.resource_concrete {
        format!("Check if resource '{}' is available", res)
    } else {
        format!("Run diagnostic commands for: {}", problem)
    }
}

/// Generate hypotheses by structural analogy when the system is stuck.
/// Uses the learner's keyword extensions if available.
pub fn generate_hypotheses(brain: &VSABrain, problem: &str) -> Vec<Hypothesis> {
    generate_hypotheses_with_learner(brain, problem, None)
}

fn generate_hypotheses_with_learner(
    brain: &VSABrain,
    problem: &str,
    learner: Option<&AbstractionLearner>,
) -> Vec<Hypothesis> {
    let mut hypotheses = Vec::new();

    // Try structural parsing (using learner extensions if available)
    if let Some(triples) = classify_structural_with_learner(problem, learner) {
        if let Some(category) = find_best_structural_category(&triples, &QaEngine::new()) {
            hypotheses.push(Hypothesis {
                category,
                source: HypothesisSource::StructuralAnalogy,
                confidence: 0.40,
                structural_triples: triples,
                test_description: generate_test_action_with_learner(problem, learner),
            });
        }
    }

    // Try centroid proximity
    let error_hv = Hypervector::encode_text_ngram(problem, 3);
    if let Some((_idx, sim)) = brain.nearest_centroid_idx(&error_hv) {
        if sim >= 0.50 {
            hypotheses.push(Hypothesis {
                category: format!("centroid_match_{:.2}", sim),
                source: HypothesisSource::CentroidProximity,
                confidence: sim * 0.8,
                structural_triples: vec![],
                test_description: format!("Nearest centroid similarity: {:.2}", sim),
            });
        }
    }

    rank_hypotheses(hypotheses)
}

/// Return hypotheses in deterministic decision order.
///
/// Ranking is intentionally explicit: raw confidence is the primary signal,
/// but equal scores must not leave the machine's next action dependent on
/// vector insertion order or hash iteration.  Structural evidence wins over
/// centroid-only evidence at equal confidence because it carries inspectable
/// SVO support; lexical tie-breakers make the order fully reproducible.
pub fn rank_hypotheses(mut hypotheses: Vec<Hypothesis>) -> Vec<Hypothesis> {
    hypotheses.sort_by(compare_hypotheses);
    hypotheses
}

fn compare_hypotheses(a: &Hypothesis, b: &Hypothesis) -> std::cmp::Ordering {
    normalized_confidence(b.confidence)
        .total_cmp(&normalized_confidence(a.confidence))
        .then_with(|| {
            hypothesis_source_priority(&a.source).cmp(&hypothesis_source_priority(&b.source))
        })
        .then_with(|| a.category.cmp(&b.category))
        .then_with(|| {
            structural_key(&a.structural_triples).cmp(&structural_key(&b.structural_triples))
        })
        .then_with(|| a.test_description.cmp(&b.test_description))
}

fn normalized_confidence(confidence: f64) -> f64 {
    if confidence.is_finite() {
        confidence.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn hypothesis_source_priority(source: &HypothesisSource) -> u8 {
    match source {
        HypothesisSource::DirectRule => 0,
        HypothesisSource::StructuralAnalogy => 1,
        HypothesisSource::CentroidProximity => 2,
        HypothesisSource::WeakTrigram => 3,
    }
}

fn structural_key(triples: &[CanonicalSvo]) -> String {
    let mut parts: Vec<String> = triples
        .iter()
        .map(|(s, v, o)| format!("{}:{}:{}", s, v, o))
        .collect();
    parts.sort();
    parts.join("|")
}

fn bounded_iteration_budget(requested: usize) -> usize {
    requested.min(MAX_AUTONOMOUS_ITERATIONS)
}

fn bounded_plan_step_count(plan_len: usize) -> usize {
    plan_len.min(MAX_PLAN_STEPS_PER_ITERATION)
}

fn next_no_progress_streak(current: usize, observations_added: usize) -> usize {
    if observations_added == 0 {
        current + 1
    } else {
        0
    }
}

fn should_stop_for_no_progress(streak: usize) -> bool {
    streak >= MAX_NO_PROGRESS_ITERATIONS
}

fn confidence_feedback_log(iteration: usize, updates: &[RuleConfidenceUpdate]) -> Vec<String> {
    updates
        .iter()
        .map(|update| {
            format!(
                "[iter {}] Confidence feedback: {}",
                iteration,
                update.summary()
            )
        })
        .collect()
}

// ─── Stage 2: Hypothesis Testing and Knowledge Acquisition ────────────────

/// Extract key technical terms from a problem description for documentation lookup.
pub fn extract_key_terms(problem: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let lower = problem.to_lowercase();

    // Extract single words that look like technical terms (≥4 chars, no common words)
    let stop_words = [
        "the", "this", "that", "from", "with", "was", "were", "has", "have", "been", "will",
        "would", "could", "should", "after", "before", "error", "failed", "failure", "unknown",
        "invalid",
    ];

    for word in lower.split_whitespace() {
        let clean: String = word
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        if clean.len() >= 4 && !stop_words.contains(&clean.as_str()) {
            terms.push(clean);
        }
    }

    // Also extract compound terms (e.g., "SSL", "KMS", "EADDRINUSE")
    for word in lower.split_whitespace() {
        let clean: String = word
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        if clean.len() >= 2
            && clean
                .chars()
                .all(|c| c.is_uppercase() || c.is_ascii_digit())
        {
            // Already added as technical term; add if not present
            if !terms.contains(&clean) && clean.len() > 2 {
                terms.push(clean);
            }
        }
    }

    terms.dedup();
    terms.truncate(5); // max 5 lookups
    terms
}

/// When Uncertain: test the best hypothesis by gathering more information.
///
/// Selects the highest-confidence hypothesis and executes a test action
/// to gather observations.  Returns any SVO observations from the test.
pub async fn resolve_uncertain(
    hypotheses: Vec<Hypothesis>,
    actuator: &JumpBoxActuator,
    brain: &mut VSABrain,
    target_ip: &str,
) -> Vec<(String, String, String)> {
    let mut ranked = rank_hypotheses(hypotheses);
    let best = ranked.remove(0);

    // Generate a test action based on the hypothesis category
    let test_request = match best.category.as_str() {
        "port_conflict" | "network_timeout" => {
            // Check what's on port 80 (common conflict point)
            ActionRequest::new(ActionType::ExecuteCommand, target_ip)
                .with_param(
                    "command",
                    "ss -tlnp 2>/dev/null || netstat -tlnp 2>/dev/null",
                )
                .with_timeout(10)
        }
        "missing_file" => ActionRequest::new(ActionType::ExecuteCommand, target_ip)
            .with_param(
                "command",
                "ls -la /etc/nginx/ 2>/dev/null; ls -la /var/log/ 2>/dev/null",
            )
            .with_timeout(10),
        "permission_denied" => ActionRequest::new(ActionType::ExecuteCommand, target_ip)
            .with_param(
                "command",
                "id; ls -la /var/run/ 2>/dev/null; cat /proc/self/status 2>/dev/null | grep Cap",
            )
            .with_timeout(10),
        "disk_full" => ActionRequest::new(ActionType::ExecuteCommand, target_ip)
            .with_param(
                "command",
                "df -h 2>/dev/null; du -sh /var/log/ 2>/dev/null | head -5",
            )
            .with_timeout(10),
        _ => {
            // Generic: read system status
            ActionRequest::new(ActionType::ExecuteCommand, target_ip)
                .with_param(
                    "command",
                    "uptime; free -h; ss -tlnp 2>/dev/null | head -20",
                )
                .with_timeout(10)
        }
    };

    let result = actuator.send_request(&test_request).await;

    if result.success && !result.raw_output.trim().is_empty() {
        // Ingest the test results into the brain
        text_encoder::ingest_text(brain, &result.raw_output, "hypothesis_test");
    }

    result.observations
}

/// When Stuck: acquire knowledge by fetching documentation.
///
/// Extracts key terms from the problem, looks up documentation for each,
/// and ingests the results into the brain.  Returns any observations.
pub async fn resolve_stuck(
    problem: &str,
    actuator: &JumpBoxActuator,
    brain: &mut VSABrain,
) -> Vec<(String, String, String)> {
    let terms = extract_key_terms(problem);
    let mut all_observations = Vec::new();

    for term in &terms {
        let docs_request = ActionRequest::fetch_docs(term);
        let result = actuator.send_request(&docs_request).await;

        if result.success && !result.raw_output.trim().is_empty() {
            // Ingest the documentation into the brain
            text_encoder::ingest_text(
                brain,
                &result.raw_output,
                &format!("acquired_knowledge_{}", term),
            );

            // Extract any SVO observations
            all_observations.extend(result.observations);
        }
    }

    all_observations
}

// ─── Stage 3: The Autonomous Loop ─────────────────────────────────────────

/// The result of an autonomous problem-solving session.
#[derive(Clone, Debug)]
pub enum SolutionResult {
    /// The problem was solved successfully.
    Solved {
        iterations: usize,
        plan: Vec<PlanStep>,
        confidence: f64,
        log: Vec<String>,
        category: String,
    },
    /// The system failed to solve the problem within the iteration budget.
    Failed {
        iterations: usize,
        last_state: String,
        log: Vec<String>,
    },
}

/// Attempt to solve a problem autonomously.
///
/// The loop:
///   1. Assess the problem (Confident / Uncertain / Stuck)
///   2. Act on the assessment
///   3. Repeat until solved or max_iterations
///
/// Each iteration produces a log entry showing what the system was thinking
/// and what it did about it.  The log is the publishable artifact.
pub async fn solve_autonomously(
    brain: &mut VSABrain,
    qa: &mut QaEngine,
    classifier: &mut ErrorClassifier,
    actuator: &JumpBoxActuator,
    problem: &str,
    goal: (&str, &str, &str),
    target_ip: &str,
    max_iterations: usize,
) -> SolutionResult {
    solve_autonomously_with_learner(
        brain,
        qa,
        classifier,
        actuator,
        problem,
        goal,
        target_ip,
        max_iterations,
        None,
    )
    .await
}

/// Like `solve_autonomously`, but accepts an external `AbstractionLearner`
/// that persists across solve sessions.  Use this when calling solve
/// repeatedly so learned keyword mappings accumulate.
///
/// Pass `None` to create an internal learner (dropped after each solve).
pub async fn solve_autonomously_with_learner(
    brain: &mut VSABrain,
    qa: &mut QaEngine,
    classifier: &mut ErrorClassifier,
    actuator: &JumpBoxActuator,
    problem: &str,
    goal: (&str, &str, &str),
    target_ip: &str,
    max_iterations: usize,
    learner: Option<&mut AbstractionLearner>,
) -> SolutionResult {
    let mut iteration_log: Vec<String> = Vec::new();
    let effective_max_iterations = bounded_iteration_budget(max_iterations);
    if effective_max_iterations < max_iterations {
        iteration_log.push(format!(
            "[setup] iteration budget capped: requested={}, effective={}",
            max_iterations, effective_max_iterations
        ));
    }

    // Learner for self-extending keyword maps.  If the caller provided one,
    // use it (persistent across calls).  Otherwise create a fresh internal one.
    let mut internal_learner = AbstractionLearner::new();
    let learner_ref: &mut AbstractionLearner = learner.unwrap_or(&mut internal_learner);
    let mut no_progress_streak = 0usize;

    // Store the problem as a fact
    qa.store_fact("system", "has_problem", problem, "autonomous_loop");

    for iteration in 0..effective_max_iterations {
        // ── 1. Check if goal is already achieved ────────────────────────────
        let (goal_verified, _) = qa.verify_fact(goal.0, goal.1, goal.2);
        if goal_verified {
            let solved_msg = format!(
                "[iter {}] Goal achieved! system {} {}",
                iteration, goal.1, goal.2
            );
            iteration_log.push(solved_msg);
            return SolutionResult::Solved {
                iterations: iteration,
                plan: vec![],
                confidence: 1.0,
                log: iteration_log,
                category: "goal_achieved".to_string(),
            };
        }

        // ── 2. Assess the current state (with learner for promoted keywords) ─
        let state = assess_with_learner(problem, brain, qa, classifier, Some(learner_ref));
        let state_name = state.name().to_string();
        let log_entry = format!("[iter {}] state={} | {}", iteration, state_name, state);
        iteration_log.push(log_entry);

        match state {
            ReasoningState::Confident {
                plan,
                confidence,
                category,
            } => {
                let confidence_report = explain_plan_confidence(&plan);
                iteration_log.push(format!(
                    "[iter {}] Plan confidence: {}",
                    iteration,
                    confidence_report.summary()
                ));
                // Execute each step of the plan
                let mut all_succeeded = true;
                let executable_steps = bounded_plan_step_count(plan.len());
                if executable_steps < plan.len() {
                    iteration_log.push(format!(
                        "[iter {}] Plan execution capped: requested_steps={}, executed_steps={}",
                        iteration,
                        plan.len(),
                        executable_steps
                    ));
                    all_succeeded = false;
                }

                for (step_idx, step) in plan.iter().take(executable_steps).enumerate() {
                    let action_req = crate::actuator::plan_step_to_request(step, target_ip);
                    let result = actuator.send_request(&action_req).await;

                    let step_log = format!(
                        "[iter {}] Executing step {}: ({}, {}, {}) → success={}",
                        iteration,
                        step_idx,
                        step.action.0,
                        step.action.1,
                        step.action.2,
                        result.success
                    );
                    iteration_log.push(step_log);

                    if !result.success {
                        all_succeeded = false;
                        // Record the failure for the planner
                        let updates = qa.evaluate_plan_outcome_report(0.0, &[step.clone()]);
                        iteration_log.extend(confidence_feedback_log(iteration, &updates));
                        break;
                    }

                    // Ingest observations from the action result
                    let obs =
                        crate::actuator::parse_result_observations(&action_req, &result, target_ip);
                    crate::actuator::ingest_observations(brain, &obs);
                }

                if all_succeeded {
                    // Check if goal was achieved
                    let (goal_ok, _) = qa.verify_fact(goal.0, goal.1, goal.2);
                    if goal_ok {
                        let updates = qa.evaluate_plan_outcome_report(1.0, &plan);
                        iteration_log.extend(confidence_feedback_log(iteration, &updates));
                        // Absorb the diagnosis — must cover all categories the
                        // diagnostic pipeline can return.
                        let categories = [
                            "port_conflict",
                            "connection_refused",
                            "missing_file",
                            "permission_denied",
                            "disk_full",
                            "credential_invalid",
                            "startup_failure",
                        ];
                        for cat in &categories {
                            if category.contains(cat) {
                                absorb_diagnosis_with_learner(
                                    brain,
                                    qa,
                                    classifier,
                                    problem,
                                    cat,
                                    1.0,
                                    Some(&mut *learner_ref),
                                );
                                break;
                            }
                        }
                        // Log learner state after each solved episode
                        let mut learner_report = learner_ref.report();
                        learner_report.truncate(400);
                        iteration_log.push(format!(
                            "[iter {}] Learner: {} promoted, {} tracked tokens",
                            iteration,
                            learner_ref.promoted_count(),
                            learner_ref.tracked_token_count(),
                        ));
                        eprintln!("  📊 Learner report:\n{}", learner_ref.report());
                        return SolutionResult::Solved {
                            iterations: iteration + 1,
                            plan,
                            confidence,
                            log: iteration_log,
                            category,
                        };
                    }
                }
                // If we get here, the plan didn't achieve the goal.
                // Next iteration will reassess with new information.
            }

            ReasoningState::Uncertain { hypotheses, .. } => {
                let new_obs = resolve_uncertain(hypotheses, actuator, brain, target_ip).await;
                let obs_count = crate::actuator::ingest_observations(brain, &new_obs);
                no_progress_streak = next_no_progress_streak(no_progress_streak, obs_count);
                iteration_log.push(format!(
                    "[iter {}] Uncertainty → tested hypothesis, ingested {} observations, no_progress_streak={}",
                    iteration, obs_count, no_progress_streak
                ));
                if should_stop_for_no_progress(no_progress_streak) {
                    iteration_log.push(format!(
                        "[iter {}] Stopping: no new observations for {} consecutive cycles",
                        iteration, no_progress_streak
                    ));
                    return SolutionResult::Failed {
                        iterations: iteration + 1,
                        last_state: "no_progress".to_string(),
                        log: iteration_log,
                    };
                }
                // Forward chain with the new information
                qa.forward_chain(0.75);
            }

            ReasoningState::Stuck { problem: p, .. } => {
                let new_obs = resolve_stuck(&p, actuator, brain).await;
                let obs_count = crate::actuator::ingest_observations(brain, &new_obs);
                no_progress_streak = next_no_progress_streak(no_progress_streak, obs_count);
                iteration_log.push(format!(
                    "[iter {}] Stuck → acquired knowledge, ingested {} observations, no_progress_streak={}",
                    iteration, obs_count, no_progress_streak
                ));
                if should_stop_for_no_progress(no_progress_streak) {
                    iteration_log.push(format!(
                        "[iter {}] Stopping: no new observations for {} consecutive cycles",
                        iteration, no_progress_streak
                    ));
                    return SolutionResult::Failed {
                        iterations: iteration + 1,
                        last_state: "no_progress".to_string(),
                        log: iteration_log,
                    };
                }
                // Forward chain with the new knowledge
                qa.forward_chain(0.75);
            }
        }
    }

    // Exhausted iteration budget
    let last_state = assess_with_learner(problem, brain, qa, classifier, Some(learner_ref));
    SolutionResult::Failed {
        iterations: effective_max_iterations,
        last_state: last_state.name().to_string(),
        log: iteration_log,
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::{seed_diagnostic_knowledge, seed_error_classifier};
    use crate::qa::QaEngine;

    fn setup() -> (VSABrain, QaEngine, ErrorClassifier) {
        let mut brain = VSABrain::new(0.12);
        let mut qa = QaEngine::new();
        let classifier = seed_error_classifier();
        seed_diagnostic_knowledge(&mut qa, &mut brain);
        (brain, qa, classifier)
    }

    fn test_hypothesis(category: &str, source: HypothesisSource, confidence: f64) -> Hypothesis {
        Hypothesis {
            category: category.to_string(),
            source,
            confidence,
            structural_triples: vec![],
            test_description: format!("test {}", category),
        }
    }

    #[test]
    fn test_confident_direct_match() {
        let (brain, qa, classifier) = setup();
        // "Address already in use" has a direct trigger match → should be Confident
        let state = assess("Address already in use", &brain, &qa, &classifier);
        eprintln!("  Confident test: {:?}", state);
        assert!(
            matches!(state, ReasoningState::Confident { .. }),
            "Should be Confident for a direct trigger match"
        );
    }

    #[test]
    fn test_confident_trigram_match() {
        let (brain, qa, classifier) = setup();
        // This shares trigrams with "Connection refused" pattern
        let state = assess("connect refused by host", &brain, &qa, &classifier);
        eprintln!("  Trigram test: {:?}", state);
        // May be Confident or Uncertain depending on trigram strength
        let name = state.name();
        assert!(
            name == "confident" || name == "uncertain",
            "Should at least be Uncertain for a trigram match (got {})",
            name
        );
    }

    #[test]
    fn test_uncertain_structural_only() {
        let (brain, qa, classifier) = setup();
        // "KV store compaction stalled" has no trigger/trigram match
        // but the structural parser should find it
        let state = assess(
            "KV store compaction stalled unexpectedly; index rebuild queued",
            &brain,
            &qa,
            &classifier,
        );
        eprintln!("  Structural test: {:?}", state);
        assert!(
            matches!(state, ReasoningState::Uncertain { .. }),
            "Should be Uncertain for structural-only match (got {})",
            state.name()
        );
    }

    #[test]
    fn test_stuck_no_match() {
        let (brain, qa, classifier) = setup();
        // Completely nonsensical text — no trigger, no trigram, no structure
        let state = assess(
            "The quick brown fox jumps over the lazy dog",
            &brain,
            &qa,
            &classifier,
        );
        eprintln!("  Stuck test: {:?}", state);
        assert!(
            matches!(state, ReasoningState::Stuck { .. }),
            "Should be Stuck for completely unknown text (got {})",
            state.name()
        );
    }

    #[test]
    fn test_hypothesis_generation() {
        let mut brain = VSABrain::new(0.12);
        // Add some knowledge to the brain so centroid proximity has something
        let hv = Hypervector::encode_text_ngram("SSL certificate expired", 3);
        brain.absorb_epistemic_update(&hv, "port_conflict", true);

        let hypotheses = generate_hypotheses(&brain, "SSL certificate expired");
        eprintln!("  Hypotheses: {} generated", hypotheses.len());
        // Should generate at least centroid-based hypothesis
        assert!(
            !hypotheses.is_empty(),
            "Should generate at least one hypothesis"
        );
    }

    #[test]
    fn test_hypothesis_ranking_is_order_independent() {
        let structural = test_hypothesis(
            "connection_refused",
            HypothesisSource::StructuralAnalogy,
            0.60,
        );
        let centroid = test_hypothesis("disk_full", HypothesisSource::CentroidProximity, 0.60);

        let ranked_a = rank_hypotheses(vec![structural.clone(), centroid.clone()]);
        let ranked_b = rank_hypotheses(vec![centroid, structural]);

        assert_eq!(ranked_a[0].category, "connection_refused");
        assert_eq!(ranked_b[0].category, "connection_refused");
        assert_eq!(
            ranked_a
                .iter()
                .map(|h| h.category.as_str())
                .collect::<Vec<_>>(),
            ranked_b
                .iter()
                .map(|h| h.category.as_str())
                .collect::<Vec<_>>(),
            "Ranking should not depend on input order"
        );
    }

    #[test]
    fn test_hypothesis_ranking_clamps_invalid_confidence() {
        let invalid = test_hypothesis("invalid_nan", HypothesisSource::DirectRule, f64::NAN);
        let valid = test_hypothesis("valid_low", HypothesisSource::WeakTrigram, 0.20);

        let ranked = rank_hypotheses(vec![invalid, valid]);
        assert_eq!(ranked[0].category, "valid_low");
    }

    #[test]
    fn test_state_display() {
        let state = ReasoningState::Stuck {
            problem: "test".to_string(),
            tried: vec!["classifier".to_string()],
        };
        let display = format!("{}", state);
        assert!(
            display.contains("Stuck"),
            "Display should contain state name"
        );
        assert!(display.contains("test"), "Display should contain problem");
    }

    #[test]
    fn test_extract_key_terms() {
        let terms = extract_key_terms("bind() to 0.0.0.0:80 failed (98: Unknown error)");
        eprintln!("  Extracted terms: {:?}", terms);
        assert!(!terms.is_empty(), "Should extract technical terms");
        // Should extract meaningful terms like "bind", "failed", "Unknown"
        let has_bind = terms.iter().any(|t| t.contains("bind"));
        let has_failed = terms.iter().any(|t| t.contains("failed"));
        assert!(
            has_bind || has_failed,
            "Should extract action-related terms"
        );
    }

    #[test]
    fn test_extract_key_terms_empty_for_short_words() {
        let terms = extract_key_terms("a b c");
        assert!(terms.is_empty(), "Short words should not be extracted");
    }

    #[test]
    fn test_extract_key_terms_max_5() {
        let terms = extract_key_terms("alpha beta gamma delta epsilon zeta eta theta");
        assert!(terms.len() <= 5, "Should extract at most 5 terms");
    }

    #[test]
    fn test_iteration_budget_is_bounded() {
        assert_eq!(bounded_iteration_budget(0), 0);
        assert_eq!(bounded_iteration_budget(3), 3);
        assert_eq!(
            bounded_iteration_budget(MAX_AUTONOMOUS_ITERATIONS + 1),
            MAX_AUTONOMOUS_ITERATIONS
        );
    }

    #[test]
    fn test_plan_step_budget_is_bounded() {
        assert_eq!(bounded_plan_step_count(0), 0);
        assert_eq!(bounded_plan_step_count(4), 4);
        assert_eq!(
            bounded_plan_step_count(MAX_PLAN_STEPS_PER_ITERATION + 10),
            MAX_PLAN_STEPS_PER_ITERATION
        );
    }

    #[test]
    fn test_no_progress_streak_updates_deterministically() {
        assert_eq!(next_no_progress_streak(0, 0), 1);
        assert_eq!(next_no_progress_streak(1, 0), 2);
        assert_eq!(next_no_progress_streak(2, 4), 0);
    }

    #[test]
    fn test_no_progress_stop_threshold() {
        assert!(!should_stop_for_no_progress(0));
        assert!(!should_stop_for_no_progress(MAX_NO_PROGRESS_ITERATIONS - 1));
        assert!(should_stop_for_no_progress(MAX_NO_PROGRESS_ITERATIONS));
    }

    #[test]
    fn test_plan_confidence_product() {
        let plan = vec![
            PlanStep {
                action: ("a".to_string(), "b".to_string(), "c".to_string()),
                achieves: ("d".to_string(), "e".to_string(), "f".to_string()),
                confidence: 0.9,
                depth: 0,
                rule_chain: vec![],
            },
            PlanStep {
                action: ("g".to_string(), "h".to_string(), "i".to_string()),
                achieves: ("j".to_string(), "k".to_string(), "l".to_string()),
                confidence: 0.8,
                depth: 1,
                rule_chain: vec![],
            },
        ];
        assert!(
            (plan_confidence(&plan) - 0.72).abs() < 0.01,
            "Plan confidence should be product of step confidences"
        );
    }

    #[test]
    fn test_plan_confidence_report_is_deterministic() {
        let plan = vec![
            PlanStep {
                action: ("a".to_string(), "b".to_string(), "c".to_string()),
                achieves: ("d".to_string(), "e".to_string(), "f".to_string()),
                confidence: 0.9,
                depth: 0,
                rule_chain: vec![],
            },
            PlanStep {
                action: ("g".to_string(), "h".to_string(), "i".to_string()),
                achieves: ("j".to_string(), "k".to_string(), "l".to_string()),
                confidence: 0.8,
                depth: 1,
                rule_chain: vec![],
            },
        ];

        let report = explain_plan_confidence(&plan);

        assert!((report.confidence - 0.72).abs() < 0.01);
        assert_eq!(report.step_count, 2);
        assert_eq!(report.weakest_step, Some(1));
        assert_eq!(
            report.summary(),
            "confidence=0.720; steps=2; weakest=1; factors=[0.900,0.800]"
        );
    }

    #[test]
    fn test_plan_confidence_report_clamps_invalid_factors() {
        let plan = vec![
            PlanStep {
                action: ("a".to_string(), "b".to_string(), "c".to_string()),
                achieves: ("d".to_string(), "e".to_string(), "f".to_string()),
                confidence: f64::NAN,
                depth: 0,
                rule_chain: vec![],
            },
            PlanStep {
                action: ("g".to_string(), "h".to_string(), "i".to_string()),
                achieves: ("j".to_string(), "k".to_string(), "l".to_string()),
                confidence: 1.4,
                depth: 1,
                rule_chain: vec![],
            },
        ];

        let report = explain_plan_confidence(&plan);

        assert_eq!(report.factors, vec![0.0, 1.0]);
        assert_eq!(report.confidence, 0.0);
        assert_eq!(report.weakest_step, Some(0));
    }

    #[test]
    fn test_confidence_feedback_log_is_stable() {
        let updates = vec![RuleConfidenceUpdate {
            rule_idx: 7,
            before: 0.4,
            after: 0.46,
            outcome: 1.0,
            error: 0.0,
            rule_label: "a b c -> d e f".to_string(),
        }];

        let lines = confidence_feedback_log(3, &updates);

        assert_eq!(
            lines,
            vec![
                "[iter 3] Confidence feedback: rule=7; before=0.400; after=0.460; delta=0.060; outcome=1.000; error=0.000; a b c -> d e f"
                    .to_string()
            ]
        );
    }
}
