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

use crate::diagnostic::{
    classify_structural, parse_error_structure,
    query_diagnostic_category, CanonicalSvo, ErrorClassifier,
};
use crate::qa::{PlanStep, QaEngine};
use crate::Hypervector;
use crate::VSABrain;

// ─── Types ─────────────────────────────────────────────────────────────────

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
    Stuck {
        problem: String,
        tried: Vec<String>,
    },
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
            ReasoningState::Confident { plan, confidence, category } => {
                write!(f, "Confident(category={}, confidence={:.2}, plan={} steps)",
                    category, confidence, plan.len())
            }
            ReasoningState::Uncertain { hypotheses, best_confidence, problem } => {
                write!(f, "Uncertain(problem=\"{}\", best_confidence={:.2}, hypotheses={})",
                    problem, best_confidence, hypotheses.len())
            }
            ReasoningState::Stuck { problem, tried } => {
                write!(f, "Stuck(problem=\"{}\", tried={})",
                    problem, tried.len())
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
pub fn assess(
    problem: &str,
    brain: &VSABrain,
    qa: &QaEngine,
    classifier: &ErrorClassifier,
) -> ReasoningState {
    // ── Level 1-2: Classifier (trigger + trigram Jaccard) ───────────────
    if let Some(canonical) = classifier.classify(problem) {
        let (subj, verb, obj) = canonical.clone();
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

    // ── Level 3: Structural parser ──────────────────────────────────────
    let struct_triples = classify_structural(problem);
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
                test_description: format!("Check resource availability based on structural parse of '{}'", problem),
            };

            return ReasoningState::Uncertain {
                hypotheses: vec![hypothesis],
                best_confidence: base_confidence,
                problem: problem.to_string(),
            };
        }
    }

    // ── Level 4: Centroid proximity ─────────────────────────────────────
    let error_hv = Hypervector::encode_text_ngram(problem, 3);
    if let Some((idx, sim)) = brain.nearest_centroid_idx(&error_hv) {
        if sim >= 0.55 {
            // Find the centroid's most common label
            let mut best_label = String::new();
            let mut best_count = 0u32;
            if let Some(cluster) = brain.dejavu_clusters.get(idx) {
                let mut freq = std::collections::HashMap::new();
                for entry in &cluster.entries {
                    *freq.entry(&entry.label).or_insert(0) += entry.weight.max(1);
                }
                for (label, count) in &freq {
                    if *count > best_count {
                        best_count = *count;
                        best_label = (*label).clone();
                    }
                }
            }

            if !best_label.is_empty() && !best_label.starts_with("concept:") {
                let hypothesis = Hypothesis {
                    category: best_label.clone(),
                    source: HypothesisSource::CentroidProximity,
                    confidence: sim,
                    structural_triples: struct_triples.unwrap_or_default(),
                    test_description: format!("Closest centroid: {} (sim={:.2})", best_label, sim),
                };
                return ReasoningState::Uncertain {
                    hypotheses: vec![hypothesis],
                    best_confidence: sim,
                    problem: problem.to_string(),
                };
            }
        }
    }

    // ── Genuinely stuck ────────────────────────────────────────────────
    ReasoningState::Stuck {
        problem: problem.to_string(),
        tried: vec!["classifier".to_string(), "structural".to_string(), "centroid".to_string()],
    }
}

/// Compute the overall confidence of a plan (product of step confidences).
fn plan_confidence(plan: &[PlanStep]) -> f64 {
    plan.iter()
        .map(|s| s.confidence)
        .fold(1.0, |acc, c| acc * c)
}

/// Given structural triples, find the best-matching diagnostic category
/// by checking which abstract rules would fire.
fn find_best_structural_category(
    triples: &[CanonicalSvo],
    qa: &QaEngine,
) -> Option<String> {
    // Categories we know about, mapped from abstract state triples
    let category_patterns: &[(&[(&str, &str, &str)], &str)] = &[
        // port_conflict: process accessing network_service, and it's unavailable
        (&[("process", "accesses", "network_service"),
           ("network_service", "has_state", "unavailable")], "port_conflict"),
        // network_timeout: process accessing network_service, unavailable
        (&[("process", "accesses", "network_service"),
           ("network_service", "has_state", "unavailable")], "network_timeout"),
        // missing_file: process accessing file_system, missing
        (&[("process", "accesses", "file_system"),
           ("file_system", "has_state", "unavailable")], "missing_file"),
        // missing_file: file_system has_state not_found
        (&[("file_system", "has_state", "not_found")], "missing_file"),
        (&[("file_system", "has_state", "resource_missing")], "missing_file"),
        // permission_denied: process accessing something, permission blocked
        (&[("file_system", "has_state", "permission_blocked")], "permission_denied"),
        (&[("network_service", "has_state", "permission_blocked")], "permission_denied"),
        // disk_full: storage capacity exhausted
        (&[("storage", "has_state", "capacity_exhausted")], "disk_full"),
        (&[("storage", "has_state", "unavailable")], "disk_full"),
        // credential_invalid
        (&[("credential", "has_state", "credential_invalid")], "permission_denied"),
        // Generic resource access failure
        (&[("process", "accesses", "storage")], "disk_full"),
        (&[("process", "accesses", "cache_resource")], "disk_full"),
        (&[("process", "accesses", "store_resource")], "disk_full"),
    ];

    for (patterns, category) in category_patterns {
        let all_match = patterns.iter().all(|(s, v, o)| {
            triples.contains(&(s.to_string(), v.to_string(), o.to_string()))
        });
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
pub fn generate_test_action(problem: &str) -> String {
    let structure = parse_error_structure(problem);
    if let Some(res) = structure.resource_concrete {
        format!("Check if resource '{}' is available", res)
    } else {
        format!("Run diagnostic commands for: {}", problem)
    }
}

/// Generate hypotheses by structural analogy when the system is stuck.
pub fn generate_hypotheses(brain: &VSABrain, problem: &str) -> Vec<Hypothesis> {
    let mut hypotheses = Vec::new();

    // Try structural parsing
    if let Some(triples) = classify_structural(problem) {
        if let Some(category) = find_best_structural_category(&triples, &QaEngine::new()) {
            hypotheses.push(Hypothesis {
                category,
                source: HypothesisSource::StructuralAnalogy,
                confidence: 0.40,
                structural_triples: triples,
                test_description: generate_test_action(problem),
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

    hypotheses
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

    #[test]
    fn test_confident_direct_match() {
        let (brain, qa, classifier) = setup();
        // "Address already in use" has a direct trigger match → should be Confident
        let state = assess("Address already in use", &brain, &qa, &classifier);
        eprintln!("  Confident test: {:?}", state);
        assert!(matches!(state, ReasoningState::Confident { .. }),
            "Should be Confident for a direct trigger match");
    }

    #[test]
    fn test_confident_trigram_match() {
        let (brain, qa, classifier) = setup();
        // This shares trigrams with "Connection refused" pattern
        let state = assess("connect refused by host", &brain, &qa, &classifier);
        eprintln!("  Trigram test: {:?}", state);
        // May be Confident or Uncertain depending on trigram strength
        let name = state.name();
        assert!(name == "confident" || name == "uncertain",
            "Should at least be Uncertain for a trigram match (got {})", name);
    }

    #[test]
    fn test_uncertain_structural_only() {
        let (brain, qa, classifier) = setup();
        // "KV store compaction stalled" has no trigger/trigram match
        // but the structural parser should find it
        let state = assess("KV store compaction stalled unexpectedly; index rebuild queued",
                           &brain, &qa, &classifier);
        eprintln!("  Structural test: {:?}", state);
        assert!(matches!(state, ReasoningState::Uncertain { .. }),
            "Should be Uncertain for structural-only match (got {})", state.name());
    }

    #[test]
    fn test_stuck_no_match() {
        let (brain, qa, classifier) = setup();
        // Completely nonsensical text — no trigger, no trigram, no structure
        let state = assess("The quick brown fox jumps over the lazy dog",
                           &brain, &qa, &classifier);
        eprintln!("  Stuck test: {:?}", state);
        assert!(matches!(state, ReasoningState::Stuck { .. }),
            "Should be Stuck for completely unknown text (got {})", state.name());
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
        assert!(!hypotheses.is_empty(),
            "Should generate at least one hypothesis");
    }

    #[test]
    fn test_state_display() {
        let state = ReasoningState::Stuck {
            problem: "test".to_string(),
            tried: vec!["classifier".to_string()],
        };
        let display = format!("{}", state);
        assert!(display.contains("Stuck"), "Display should contain state name");
        assert!(display.contains("test"), "Display should contain problem");
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
        assert!((plan_confidence(&plan) - 0.72).abs() < 0.01,
            "Plan confidence should be product of step confidences");
    }
}
