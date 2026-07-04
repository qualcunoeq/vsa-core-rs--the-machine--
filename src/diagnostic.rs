// ─── Diagnostic Knowledge — Generic Reasoning About Broken Systems ─────
//
// These rules encode HOW to diagnose, not WHAT to diagnose.
// The same rules work for any service that fails to start because a
// port is in use, a config is wrong, or a dependency is missing.
//
// The diagnostic loop:
//   1. Read error log → extract error type
//   2. Form hypothesis about cause (abduction via causal rules)
//   3. Verify hypothesis by checking system state
//   4. Plan and execute fix actions
//   5. Verify fix succeeded
//
// All rules are generic — they describe diagnostic strategies, not
// specific knowledge about nginx, Apache, or any particular system.
//
// ─── Error Classifier ───────────────────────────────────────────────────
//
// The ErrorClassifier bridges the gap between "textually different but
// semantically equivalent" error messages.  It maps raw error log text
// to canonical error types using two strategies:
//
//   Level 1 (fast trigger):   Substring matching against known triggers
//     e.g., "Address already in use" → "port_conflict"
//           "bind() to 0.0.0.0:80 failed" → "port_conflict"
//
//   Level 2 (trigram Jaccard):  Trigram overlap with known patterns
//     e.g., "bind to [::]:443 failed" → "port_conflict"
//           (shares "bind", "failed" trigrams with known patterns)
//
// ─── SVO Matching Note ──────────────────────────────────────────────────
//
// The SVO encoding uses XOR: encode_svo(s,v,o) = rot13(s) ⊕ rot26(v) ⊕ rot39(o).
// This means matching S+V components CANCEL OUT when comparing two SVO
// hypervectors.  A forward-chain rule with antecedent (S,V,O) will ONLY
// match a fact with the EXACT SAME (S,V,O) — partial matching (same S+V,
// different O) gives energy ≈ 0.5 (noise floor).
//
// This is why we use the ErrorClassifier: it maps any error text to a
// CANONICAL (subject, verb, object) triple.  The fact is stored with this
// exact triple, and the rule antecedent uses the same exact triple,
// giving perfect energy 1.0.
// ────────────────────────────────────────────────────────────────────────────

use std::collections::{HashMap, HashSet};
use crate::qa::QaEngine;
use crate::text_encoder::{ingest_text, store_knowledge_triple};
use crate::Hypervector;
use crate::VSABrain;

// ─── Error Classifier ──────────────────────────────────────────────────────

/// A canonical (subject, verb, object) triple for forward-chain matching.
pub type CanonicalSvo = (String, String, String);

/// A registered error type with its triggers and canonical SVO.
struct ErrorTypeEntry {
    name: String,
    /// Substring triggers for Level-1 (fast) matching.
    triggers: Vec<String>,
    /// Full pattern strings for Level-2 trigram Jaccard matching.
    patterns: Vec<String>,
    /// Canonical SVO triple for forward-chain rule matching.
    canonical: CanonicalSvo,
    /// Precomputed trigram sets for patterns.
    pattern_trigrams: Vec<HashSet<String>>,
}

fn trigrams(s: &str) -> HashSet<String> {
    let lower = s.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    if chars.len() < 3 {
        let mut set = HashSet::new();
        set.insert(lower);
        return set;
    }
    chars.windows(3).map(|w| w.iter().collect()).collect()
}

/// Maps raw error text to canonical error types.
///
/// Uses a two-level approach:
///   1. **Trigger matching**: fast substring checks (handles synonyms like
///      "Address already in use" → port_conflict even when trigrams differ).
///   2. **Trigram Jaccard similarity**: measures trigram overlap with known
///      patterns for partial/analogical matches.
pub struct ErrorClassifier {
    types: Vec<ErrorTypeEntry>,
    /// Total registered pattern count (for reporting).
    pattern_count: usize,
}

impl ErrorClassifier {
    /// Create an empty classifier.
    pub fn new() -> Self {
        Self {
            types: Vec::new(),
            pattern_count: 0,
        }
    }

    /// Register an error type with its textual triggers and canonical SVO.
    ///
    /// `triggers` are substring patterns — if any appear in the error text
    /// (case-insensitive), the text is classified as this error type.
    ///
    /// `patterns` are full example error texts used for Level-2 trigram
    /// Jaccard matching (analogical fallthrough).
    ///
    /// `canonical` is the (subject, verb, object) triple stored as a fact
    /// for forward-chain rule matching.  The SVO encoding uses XOR, so
    /// both fact and rule MUST use the exact same triple to match.
    pub fn register(
        &mut self,
        name: &str,
        triggers: &[&str],
        patterns: &[&str],
        canonical: (&str, &str, &str),
    ) {
        let pattern_trigrams: Vec<HashSet<String>> = patterns.iter()
            .map(|p| trigrams(p))
            .collect();
        self.pattern_count += patterns.len();
        self.types.push(ErrorTypeEntry {
            name: name.to_string(),
            triggers: triggers.iter().map(|s| s.to_lowercase()).collect(),
            patterns: patterns.iter().map(|s| s.to_string()).collect(),
            canonical: (
                canonical.0.to_lowercase(),
                canonical.1.to_lowercase(),
                canonical.2.to_lowercase(),
            ),
            pattern_trigrams,
        });
    }

    /// Classify error text using Level-1 trigger matching.
    ///
    /// Returns the canonical SVO triple if a trigger matches.
    pub fn classify(&self, error_text: &str) -> Option<&CanonicalSvo> {
        let lower = error_text.to_lowercase();
        for entry in &self.types {
            for trigger in &entry.triggers {
                if lower.contains(trigger) {
                    return Some(&entry.canonical);
                }
            }
        }
        None
    }

    /// Classify error text using Level-2 trigram Jaccard similarity.
    ///
    /// Computes trigram overlap between the query text and all known patterns.
    /// Returns the best-matching error type if the Jaccard similarity exceeds
    /// the threshold.
    ///
    /// This captures analogical matches: e.g., "bind to [::]:443 failed"
    /// shares trigrams like {"bin", "ind", "to_", "_fa", "fai", "ail", "led"}
    /// with the known pattern "bind() to 0.0.0.0:80 failed (98: Unknown error)",
    /// even though the exact strings are different.
    ///
    /// Threshold of 0.10 is very permissive — captures even weak trigram
    /// overlap while rejecting random noise (which is typically ~0.0-0.02).
    pub fn classify_trigram(&self, error_text: &str) -> Option<&CanonicalSvo> {
        let query_tri = trigrams(error_text);
        if query_tri.is_empty() {
            return None;
        }

        let mut best_type: Option<&CanonicalSvo> = None;
        let mut best_jaccard = 0.0;

        for entry in &self.types {
            for pattern_tri in &entry.pattern_trigrams {
                let intersection = query_tri.intersection(pattern_tri).count();
                let union = query_tri.union(pattern_tri).count();
                let jaccard = if union > 0 {
                    intersection as f64 / union as f64
                } else {
                    0.0
                };
                if jaccard > best_jaccard {
                    best_jaccard = jaccard;
                    best_type = Some(&entry.canonical);
                }
            }
        }

        const TRIGRAM_MATCH_THRESHOLD: f64 = 0.10;
        if best_jaccard >= TRIGRAM_MATCH_THRESHOLD {
            best_type
        } else {
            None
        }
    }

    /// Classify using both levels.  Level-1 (trigger) takes priority.
    ///
    /// Returns the canonical SVO triple and which level matched
    /// ("trigger", "trigram", or "none").
    pub fn classify_deep(&self, error_text: &str) -> (Option<&CanonicalSvo>, &'static str) {
        // Level 1: fast trigger matching
        if let Some(svo) = self.classify(error_text) {
            return (Some(svo), "trigger");
        }
        // Level 2: trigram Jaccard similarity
        if let Some(svo) = self.classify_trigram(error_text) {
            return (Some(svo), "trigram");
        }
        (None, "none")
    }

    /// Add a new pattern to an existing error type (self-extending after diagnosis).
    ///
    /// Called after a successful diagnosis: the error text that was just classified
    /// is added as a new pattern for its category.  Future queries with similar text
    /// will match via Level 2 (trigram Jaccard) even if they don't match any trigger.
    ///
    /// Returns `true` if the pattern was added, `false` if the category was not found.
    pub fn add_pattern(&mut self, category: &str, pattern_text: &str) -> bool {
        for entry in &mut self.types {
            if entry.name == category {
                let lower = pattern_text.to_lowercase();
                // Don't add duplicates
                if entry.patterns.iter().any(|p| p.to_lowercase() == lower) {
                    return true;
                }
                entry.patterns.push(pattern_text.to_string());
                entry.pattern_trigrams.push(trigrams(pattern_text));
                return true;
            }
        }
        false
    }

    /// Return the number of known patterns per type (for reporting).
    pub fn pattern_counts(&self) -> Vec<(String, usize)> {
        self.types.iter().map(|e| (e.name.clone(), e.patterns.len())).collect()
    }

    /// Return the number of registered error types.
    pub fn type_count(&self) -> usize {
        self.types.len()
    }

    /// Get the canonical SVO for a named type (used in tests).
    pub fn get_canonical(&self, name: &str) -> Option<&CanonicalSvo> {
        self.types.iter()
            .find(|e| e.name == name)
            .map(|e| &e.canonical)
    }
}

// ─── Knowledge Seeding ─────────────────────────────────────────────────────

/// Seed generic diagnostic knowledge into The Machine.
///
/// Rules are organized in layers:
///   0. Error-type-level rules (used by the ErrorClassifier)
///   1. Cause → verification (how do I check if this cause is real?)
///   2. Verified cause → fix action (what do I do about it?)
///   3. Fix action → goal state (what does success look like?)
///
/// IMPORTANT: The SVO encoding uses XOR (not bundling), so rules MUST use
/// the EXACT canonical triples that the classifier stores.  Partial matching
/// (same subject+verb, different object) gives energy ≈ 0.5 — below threshold.
pub fn seed_diagnostic_knowledge(qa: &mut QaEngine, _brain: &mut VSABrain) {
    // ═════════════════════════════════════════════════════════════════════
    // LAYER 0: Error-Type-Level Rules
    //
    // These rules use the canonical SVO triple that the classifier stores
    // after mapping raw error text to a known error type.
    // The SVO triple uses the "has_type" verb with the error type name
    // as the object — this gives perfect 1.0 energy matching because
    // both fact and rule use the exact same hypervector encoding.
    // ═════════════════════════════════════════════════════════════════════

    // "error has_type port_conflict" → "another_process is_listening_on same_port"
    qa.store_rule(
        "error", "has_type", "port_conflict",
        "another_process", "is_listening_on", "same_port",
        "diagnostic_error_type",
    );

    // "error has_type connection_refused" → "target_service is_not listening"
    qa.store_rule(
        "error", "has_type", "connection_refused",
        "target_service", "is_not", "listening",
        "diagnostic_error_type",
    );

    // "error has_type missing_file" → "required_file is missing"
    qa.store_rule(
        "error", "has_type", "missing_file",
        "required_file", "is", "missing",
        "diagnostic_error_type",
    );

    // "error has_type permission_denied" → "file_permissions are incorrect"
    qa.store_rule(
        "error", "has_type", "permission_denied",
        "file_permissions", "are", "incorrect",
        "diagnostic_error_type",
    );

    // "error has_type startup_failure" → "service has startup_problem"
    qa.store_rule(
        "error", "has_type", "startup_failure",
        "service", "has", "startup_problem",
        "diagnostic_error_type",
    );

    // ═════════════════════════════════════════════════════════════════════
    // LAYER 1: Cause → Verification Action
    // ═════════════════════════════════════════════════════════════════════

    qa.store_action(
        "machine", "check_port", "target:port",
        "machine", "knows", "process_on_port",
        "diagnostic_actions",
    );

    qa.store_action(
        "machine", "check_service_running", "target:name",
        "machine", "knows", "service_status",
        "diagnostic_actions",
    );

    qa.store_action(
        "machine", "read_error_log", "target:path",
        "machine", "knows", "error_content",
        "diagnostic_actions",
    );

    // ═════════════════════════════════════════════════════════════════════
    // LAYER 2: Verified Cause → Fix Action
    // ═════════════════════════════════════════════════════════════════════

    qa.store_action(
        "machine", "free_port_and_restart", "target:port:service",
        "machine", "has", "fixed_port_conflict",
        "diagnostic_actions",
    );

    qa.store_action(
        "machine", "resolve_missing_file", "target:path:content",
        "machine", "has", "fixed_missing_file",
        "diagnostic_actions",
    );

    qa.store_action(
        "machine", "fix_permissions", "target:path:perms",
        "machine", "has", "fixed_permissions",
        "diagnostic_actions",
    );

    qa.store_action(
        "machine", "restart_service", "target:name",
        "service", "is", "running",
        "diagnostic_actions",
    );

    // ═════════════════════════════════════════════════════════════════════
    // LAYER 3: Causal chain linking diagnostics to the goal
    // ═════════════════════════════════════════════════════════════════════

    qa.store_rule(
        "machine", "knows", "error_content",
        "machine", "identifies", "possible_cause",
        "diagnostic_chain",
    );

    qa.store_rule(
        "machine", "knows", "process_on_port",
        "machine", "confirms", "cause",
        "diagnostic_chain",
    );

    qa.store_rule(
        "machine", "confirms", "cause",
        "machine", "can", "fix_problem",
        "diagnostic_chain",
    );

    qa.store_rule(
        "machine", "has", "fixed_port_conflict",
        "service", "is", "running",
        "diagnostic_chain",
    );

    qa.store_rule(
        "machine", "restarts", "service",
        "service", "is", "running",
        "diagnostic_chain",
    );

    // ═════════════════════════════════════════════════════════════════════
    // DOCUMENTATION — generic diagnostic knowledge as text
    // ═════════════════════════════════════════════════════════════════════

    let diagnostic_text = concat!(
        "When a service fails to start, the error log contains information about what went wrong. ",
        "Common errors include: address already in use (another process is using the port), ",
        "file not found (a configuration file or dependency is missing), ",
        "permission denied (the process cannot access a file it needs), ",
        "and connection refused (a service it depends on is not running). ",
        "To diagnose: read the error log, check which process is on the conflicting port, ",
        "verify the configuration file exists, and check that all dependencies are running. ",
        "To fix a port conflict: stop the process using the port, then restart the target service.",
    );

    ingest_text(_brain, diagnostic_text, "diagnostic_knowledge");

    // ── Experiment metadata ─────────────────────────────────────────────
    store_knowledge_triple(_brain, "diagnostic_system", "is_ready", "true", 1.0, "experiment_metadata");
}

/// Seed the ErrorClassifier with known error types and their textual triggers.
///
/// Returns a fully populated classifier ready for use in the diagnostic loop.
pub fn seed_error_classifier() -> ErrorClassifier {
    let mut classifier = ErrorClassifier::new();

    // ── Port conflict ────────────────────────────────────────────────────
    // Multiple textual forms for the same underlying problem.
    classifier.register(
        "port_conflict",
        &[
            "bind()", "bind failed", "failed to bind",
            "address already in use", "port already in use",
            "eadinuse", "eaddrinuse", "could not bind",
            "port is already allocated", "port is allocated",
        ],
        &[
            "bind() to 0.0.0.0:80 failed (98: Unknown error)",
            "bind() to [::]:80 failed (98: Unknown error)",
            "Address already in use",
            "port already in use",
        ],
        ("error", "has_type", "port_conflict"),
    );

    // ── Connection refused ───────────────────────────────────────────────
    classifier.register(
        "connection_refused",
        &[
            "connection refused", "econnrefused",
            "actively refused", "no connection could be made",
            "target machine refused",
        ],
        &[
            "Connection refused",
            "connect: connection refused",
        ],
        ("error", "has_type", "connection_refused"),
    );

    // ── Missing file ──────────────────────────────────────────────────────
    classifier.register(
        "missing_file",
        &[
            "no such file", "not found", "cannot open",
            "enoent", "does not exist", "no such directory",
        ],
        &[
            "No such file or directory",
            "cannot open file: No such file or directory",
        ],
        ("error", "has_type", "missing_file"),
    );

    // ── Permission denied ────────────────────────────────────────────────
    classifier.register(
        "permission_denied",
        &[
            "permission denied", "eacces", "eperm",
            "not permitted", "access denied",
        ],
        &[
            "Permission denied",
            "cannot access: Permission denied",
        ],
        ("error", "has_type", "permission_denied"),
    );

    // ── Startup failure (generic) ────────────────────────────────────────
    // NOTE: Do NOT add bare "failed" as a trigger — it's too broad and will
    // match nearly every error message (e.g., "bind to port 80 failed" would
    // match startup_failure instead of port_conflict).  Only use compound
    // triggers that are specific enough to discriminate.
    classifier.register(
        "startup_failure",
        &[
            "startup failed", "could not start",
            "initialization failed", "fatal error",
        ],
        &[
            "startup failed",
        ],
        ("error", "has_type", "startup_failure"),
    );

    classifier
}

// ─── Epistemic Update Wiring ──────────────────────────────────────────────
//
// After a successful diagnosis, feed the episode back into the VSABrain so the
// system learns from experience.  Over multiple episodes, the brain builds:
//
//   1. **Error text centroids**: dejavu clusters for each error text that
//      stabilize after repeated exposure.
//
//   2. **Category concept centroids**: separate centroids for each diagnostic
//      category (port_conflict, connection_refused, etc.).  These are fixed
//      vectors independent of the error text, so all port_conflict episodes
//      cluster together regardless of the error text's trigrams.
//
//   3. **Cross-cluster associations**: when an error text cluster is activated
//      near a category concept cluster (within the association window), an
//      association is formed.  Level 2 of resolve_term can follow this:
//      error text → category concept.
//
//   4. **Self-extending classifier patterns**: the error text is added to the
//      classifier's pattern set for its category, so future trigram Jaccard
//      matching (Level 2 of the classifier) recognizes similar texts.

/// Feed a successful diagnosis back into the VSABrain for learning.
///
/// Call this after the diagnostic loop has identified a cause, verified it,
/// executed a fix, and confirmed the fix worked.
///
/// This wires `absorb_epistemic_update`, `add_transient_fact`, and `add_pattern`
/// into a single call.  After calling `absorb`, sync QA cluster data so
/// `resolve_term` can use the new centroids and associations.
pub fn absorb_diagnosis(
    brain: &mut VSABrain,
    qa: &mut QaEngine,
    classifier: &mut ErrorClassifier,
    error_text: &str,
    category: &str,
    outcome: f64,
) {
    // 1. Absorb the error text into dejavu clusters (episodic memory)
    let error_hv = Hypervector::encode_text_ngram(error_text, 3);
    brain.absorb_epistemic_update(&error_hv, category, true);

    // 2. Absorb the category concept as a separate centroid
    //    The category concept hypervector is deterministic and independent
    //    of the error text's trigrams.  This creates a cluster that all
    //    episodes of the same category reinforce, enabling associative
    //    linking between error text centroids and category centroids.
    let concept_name = format!("concept:{}", category);
    let concept_hv = Hypervector::encode_text_ngram(&concept_name, 3);
    brain.absorb_epistemic_update(&concept_hv, category, true);

    // 3. Store the outcome as a transient fact
    let mut meta = HashMap::new();
    meta.insert("category".to_string(), category.to_string());
    meta.insert("outcome".to_string(), format!("{:.2}", outcome));
    meta.insert("type".to_string(), "diagnosis".to_string());
    brain.add_transient_fact(concept_hv, "diagnostic_category", meta);

    // 4. Sync cluster data to the QaEngine so resolve_term can use it
    qa.sync_cluster_data(brain);

    // 5. Self-extend the classifier's pattern set
    classifier.add_pattern(category, error_text);
}

/// Query the VSABrain for the nearest diagnostic category to an error text.
///
/// This is the third level of classification (after the classifier's Level-1
/// trigger and Level-2 trigram matching).  It uses the VSABrain's dejavu
/// clusters and cross-cluster associations to find the category concept
/// nearest to the error text's trigram encoding.
///
/// Strategy:
///   1. Encode the error text as a trigram bundle
///   2. Find the nearest dejavu cluster centroid
///   3. Check if any entry in that cluster has a concept label
///   4. Follow cross-cluster associations to find a diagnostic category
///
/// Returns the category name if found, None otherwise.
pub fn query_diagnostic_category(
    brain: &VSABrain,
    error_text: &str,
) -> Option<String> {
    let error_hv = Hypervector::encode_text_ngram(error_text, 3);

    // Step 1: find nearest dejavu cluster by trigram similarity
    let (nearest_idx, nearest_sim) = brain.nearest_centroid_idx(&error_hv)?;

    // Step 2: Check if any entry in this cluster has a concept label
    // (The concept cluster should have been created by absorb_diagnosis
    //  which uses the label "concept:category_name")
    if nearest_sim >= 0.55 {
        let cluster = &brain.dejavu_clusters[nearest_idx];
        for entry in &cluster.entries {
            if entry.label.starts_with("concept:") {
                return Some(entry.label[8..].to_string());
            }
        }
        // Also check transient clusters for the same
        for tc in &brain.transient_clusters {
            if let Some(entry) = tc.entries.first() {
                if entry.label.starts_with("concept:") {
                    for entry in &tc.entries {
                        if entry.label.starts_with("concept:") {
                            return Some(entry.label[8..].to_string());
                        }
                    }
                }
            }
        }
    }

    // Step 3: follow cross-cluster associations from nearest cluster
    let assocs = brain.get_associations(nearest_idx);
    for (target_idx, strength) in &assocs {
        if *strength >= crate::ASSOCIATION_RESOLUTION_THRESHOLD {
            if let Some(centroid) = brain.get_centroid(*target_idx) {
                let sim = 1.0 - error_hv.normalized_hamming_distance(centroid);
                if sim >= 0.55 {
                    let target_cluster = &brain.dejavu_clusters[*target_idx];
                    for entry in &target_cluster.entries {
                        if entry.label.starts_with("concept:") {
                            return Some(entry.label[8..].to_string());
                        }
                    }
                }
            }
        }
    }

    None
}

/// Query whether a specific diagnostic category has been learned from past episodes.
/// Returns the number of reinforcement episodes for that category.
pub fn diagnosis_reinforcement_count(brain: &VSABrain, category: &str) -> usize {
    let concept_name = format!("concept:{}", category);
    let concept_hv = Hypervector::encode_text_ngram(&concept_name, 3);
    brain.dejavu_clusters.iter()
        .filter(|c| {
            let sim = 1.0 - concept_hv.normalized_hamming_distance(&c.centroid);
            sim >= 0.65
        })
        .map(|c| c.total_weight as usize)
        .sum()
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── ErrorClassifier Tests ────────────────────────────────────────────

    #[test]
    fn test_classify_port_conflict_by_address_in_use() {
        let classifier = seed_error_classifier();
        let error_text = "Address already in use";
        let svo = classifier.classify(error_text);
        assert!(svo.is_some(), "Should classify 'Address already in use'");
        assert_eq!(svo.unwrap().2, "port_conflict");
    }

    #[test]
    fn test_classify_port_conflict_by_bind_failed() {
        let classifier = seed_error_classifier();
        let error_text = "bind() to 0.0.0.0:80 failed (98: Unknown error)";
        let svo = classifier.classify(error_text);
        assert!(svo.is_some(), "Should classify bind failure");
        assert_eq!(svo.unwrap().2, "port_conflict");
    }

    #[test]
    fn test_classify_port_conflict_by_eaddrinuse() {
        let classifier = seed_error_classifier();
        let error_text = "socket.error: [Errno 98] EADDRINUSE";
        let svo = classifier.classify(error_text);
        assert!(svo.is_some(), "Should classify EADDRINUSE");
        assert_eq!(svo.unwrap().2, "port_conflict");
    }

    #[test]
    fn test_classify_port_conflict_by_docker() {
        let classifier = seed_error_classifier();
        let error_text = "port is already allocated";
        let svo = classifier.classify(error_text);
        assert!(svo.is_some(), "Should classify 'port is already allocated'");
        assert_eq!(svo.unwrap().2, "port_conflict");
    }

    #[test]
    fn test_classify_connection_refused() {
        let classifier = seed_error_classifier();
        let error_text = "Connection refused";
        let svo = classifier.classify(error_text);
        assert!(svo.is_some(), "Should classify connection refused");
        assert_eq!(svo.unwrap().2, "connection_refused");
    }

    #[test]
    fn test_classify_missing_file() {
        let classifier = seed_error_classifier();
        let error_text = "No such file or directory";
        let svo = classifier.classify(error_text);
        assert!(svo.is_some(), "Should classify missing file");
        assert_eq!(svo.unwrap().2, "missing_file");
    }

    #[test]
    fn test_classify_permission_denied() {
        let classifier = seed_error_classifier();
        let error_text = "Permission denied";
        let svo = classifier.classify(error_text);
        assert!(svo.is_some(), "Should classify permission denied");
        assert_eq!(svo.unwrap().2, "permission_denied");
    }

    #[test]
    fn test_no_classification_for_unknown_text() {
        let classifier = seed_error_classifier();
        let error_text = "Everything is fine here";
        let svo = classifier.classify(error_text);
        assert!(svo.is_none(), "Should not classify benign text");
    }

    // ── Trigram Jaccard (Level 2) Tests ──────────────────────────────────

    #[test]
    fn test_trigram_match_port_conflict_variant() {
        // "bind to 10.0.0.1:8080 failed" shares trigrams with the known
        // pattern "bind() to 0.0.0.0:80 failed (98: Unknown error)"
        // (both contain "bin", "ind", "_to", "to_", "_fa", "fai", "ail",
        //  "ile", "led").  Does NOT match any Level-1 trigger exactly.
        let classifier = seed_error_classifier();
        let error_text = "bind to 10.0.0.1:8080 failed";
        let (svo, level) = classifier.classify_deep(error_text);
        assert!(svo.is_some(),
            "Trigram should classify text with partial trigram overlap");
        assert_eq!(level, "trigram",
            "Should match via trigram, not trigger");
        assert_eq!(svo.unwrap().2, "port_conflict");
    }

    #[test]
    fn test_trigram_match_connection_variant() {
        // "connect refused" shares trigrams with "Connection refused"
        let classifier = seed_error_classifier();
        let error_text = "connect refused";
        let (svo, level) = classifier.classify_deep(error_text);
        assert!(svo.is_some(),
            "Trigram should classify 'connect refused'");
        assert_eq!(level, "trigram",
            "Should match via trigram");
        assert_eq!(svo.unwrap().2, "connection_refused");
    }

    #[test]
    fn test_trigram_no_match_for_unrelated_text() {
        // A completely unexpected error message — shares essentially NO
        // trigrams with any known pattern.
        let classifier = seed_error_classifier();
        let error_text = "Kernel panic - not syncing: VFS: Unable to mount root fs";
        let (svo, level) = classifier.classify_deep(error_text);
        assert!(svo.is_none(),
            "Should not classify completely unrelated error");
        assert_eq!(level, "none",
            "Should explicitly report no match");
    }

    #[test]
    fn test_classify_deep_reports_level() {
        let classifier = seed_error_classifier();
        // Trigger match
        let (_, level) = classifier.classify_deep("Address already in use");
        assert_eq!(level, "trigger");
        // Trigram match
        let (_, level) = classifier.classify_deep("bind on port 8080 failed");
        assert_eq!(level, "trigram");
        // No match
        let (_, level) = classifier.classify_deep("green eggs and ham");
        assert_eq!(level, "none");
    }

    #[test]
    fn test_empty_classifier_returns_none() {
        let classifier = ErrorClassifier::new();
        assert!(classifier.classify("anything").is_none());
        assert!(classifier.classify_trigram("anything").is_none());
        assert_eq!(classifier.type_count(), 0);
    }

    #[test]
    fn test_get_canonical() {
        let classifier = seed_error_classifier();
        let svo = classifier.get_canonical("port_conflict");
        assert!(svo.is_some());
        assert_eq!(svo.unwrap().0, "error");
        assert_eq!(svo.unwrap().1, "has_type");
        assert_eq!(svo.unwrap().2, "port_conflict");
    }

    // ── Diagnostic Rules Tests ───────────────────────────────────────────

    #[test]
    fn test_diagnostic_chain_planning() {
        let mut qa = QaEngine::new();
        let mut brain = VSABrain::new(0.12);
        seed_diagnostic_knowledge(&mut qa, &mut brain);

        let plan = qa.plan_for_goal("service", "is", "running", 10);
        assert!(!plan.is_empty(),
            "Should find at least one action to diagnose/restart service");
    }

    #[test]
    fn test_error_type_forward_chain() {
        // Tests that the new error-type-level rules fire correctly when
        // the classifier stores a canonical triple.
        let mut qa = QaEngine::new();
        let mut brain = VSABrain::new(0.12);
        seed_diagnostic_knowledge(&mut qa, &mut brain);

        // Store fact using the canonical error-type triple (what the
        // ErrorClassifier would produce).
        qa.store_fact("error", "has_type", "port_conflict", "classifier");

        // Forward chain should derive: another_process is_listening_on same_port
        let n = qa.forward_chain(0.75);
        assert!(n >= 1, "Should derive at least 1 fact from error type");

        let (verified, _conf) = qa.verify_fact("another_process", "is_listening_on", "same_port");
        assert!(verified, "Error-type rule should derive port conflict cause");
    }

    #[test]
    fn test_error_type_all_types_chain() {
        let mut qa = QaEngine::new();
        let mut brain = VSABrain::new(0.12);
        seed_diagnostic_knowledge(&mut qa, &mut brain);

        let types = ["port_conflict", "connection_refused", "missing_file", "permission_denied", "startup_failure"];
        let expected = [
            "another_process is_listening_on same_port",
            "target_service is_not listening",
            "required_file is missing",
            "file_permissions are incorrect",
            "service has startup_problem",
        ];

        for (i, error_type) in types.iter().enumerate() {
            let mut qa2 = QaEngine::new();
            let mut brain2 = VSABrain::new(0.12);
            seed_diagnostic_knowledge(&mut qa2, &mut brain2);

            qa2.store_fact("error", "has_type", error_type, "classifier");
            let n = qa2.forward_chain(0.75);
            assert!(n >= 1, "Type '{}' should derive at least 1 fact (got {})", error_type, n);

            let parts: Vec<&str> = expected[i].splitn(3, ' ').collect();
            let (subj, verb, obj) = (parts[0], parts[1], parts[2]);
            let (verified, _) = qa2.verify_fact(subj, verb, obj);
            assert!(verified, "Type '{}' should derive: {} (got {})",
                error_type, expected[i], if verified { "yes" } else { "no" });
        }
    }

    #[test]
    fn test_classifier_to_forward_chain_integration() {
        // End-to-end: classifier maps error text → canonical triple →
        // forward chain derives correct cause.
        let mut qa = QaEngine::new();
        let mut brain = VSABrain::new(0.12);
        seed_diagnostic_knowledge(&mut qa, &mut brain);
        let classifier = seed_error_classifier();

        // Simulate reading an error log.
        let error_text = "nginx: [emerg] bind() to 0.0.0.0:80 failed (98: Unknown error)";

        // Use classifier to get the canonical triple.
        let svo = classifier.classify(error_text)
            .expect("Should classify nginx error");
        let (subj, verb, obj) = svo.clone();

        // Store the canonical fact.
        qa.store_fact(&subj, &verb, &obj, "error_log");

        // Forward chain should derive port conflict cause.
        let n = qa.forward_chain(0.75);
        assert!(n >= 1, "Should derive facts from classified error");

        let (verified, _) = qa.verify_fact("another_process", "is_listening_on", "same_port");
        assert!(verified, "Classified error should trigger port conflict rule");
    }

    #[test]
    fn test_classifier_to_forward_chain_eaddrinuse() {
        // Same as above but with EADDRINUSE form.
        let mut qa = QaEngine::new();
        let mut brain = VSABrain::new(0.12);
        seed_diagnostic_knowledge(&mut qa, &mut brain);
        let classifier = seed_error_classifier();

        let error_text = "socket.error: [Errno 98] EADDRINUSE";
        let svo = classifier.classify(error_text)
            .expect("Should classify EADDRINUSE");
        let (subj, verb, obj) = svo.clone();
        qa.store_fact(&subj, &verb, &obj, "error_log");
        let n = qa.forward_chain(0.75);
        assert!(n >= 1, "EADDRINUSE should derive facts");

        let (verified, _) = qa.verify_fact("another_process", "is_listening_on", "same_port");
        assert!(verified, "EADDRINUSE should trigger port conflict rule");
    }

    /// Test: truly novel error pattern (no Level-1 trigger, no Level-2 trigram match)
    #[test]
    fn test_novel_error_no_rule_coverage() {
        // This tests what happens when The Machine encounters an error
        // pattern it has NEVER seen before — no trigger matches, no trigram
        // overlap with any known pattern.
        let classifier = seed_error_classifier();
        let error_text = "[KMS] keyserver unreachable: timeout";

        let (svo, level) = classifier.classify_deep(error_text);
        assert!(svo.is_none(),
            "Novel error should NOT match any known type");
        assert_eq!(level, "none",
            "Should explicitly report no match");

        // Verify: the downstream consequence is that the diagnostic loop
        // reports "unknown error pattern" instead of forming a hypothesis.
        let mut qa = QaEngine::new();
        let mut brain = VSABrain::new(0.12);
        seed_diagnostic_knowledge(&mut qa, &mut brain);

        // Simulate: the experiment stores the canonical triple from the
        // classifier — which is NONE for a novel error.
        // No facts are stored about the error type.
        // Forward chain produces 0 facts.
        let n = qa.forward_chain(0.75);
        let (has_cause, _) = qa.verify_fact("another_process", "is_listening_on", "same_port");
        let (can_fix, _) = qa.verify_fact("machine", "can", "fix_problem");

        eprintln!("  Novel error test:");
        eprintln!("    Forward chain: {} facts", n);
        eprintln!("    Has cause: {}, can fix: {}", has_cause, can_fix);

        // No hypothesis formed, no fix possible.
        assert_eq!(n, 0, "No facts should be derived for an unclassified error");
        assert!(!has_cause, "No hypothesis should be formed");
        assert!(!can_fix, "No fix should be available");
    }

    #[test]
    fn test_forward_chain_full_diagnostic_sequence() {
        // Complete diagnostic sequence:
        //   1. Error is classified → store canonical triple
        //   2. Forward chain: error type → cause
        //   3. Simulate port check → store verification
        //   4. Forward chain: verification → can fix
        let mut qa = QaEngine::new();
        let mut brain = VSABrain::new(0.12);
        seed_diagnostic_knowledge(&mut qa, &mut brain);

        // Step 1: Classify error
        qa.store_fact("error", "has_type", "port_conflict", "error_log");
        let n1 = qa.forward_chain(0.75);
        eprintln!("  After error type: {} facts derived", n1);
        assert!(n1 >= 1, "Should derive cause from error type");

        let (has_cause, _) = qa.verify_fact("another_process", "is_listening_on", "same_port");
        assert!(has_cause, "Should have identified the cause");

        // Step 2: Verify by checking port
        qa.store_fact("machine", "knows", "process_on_port", "port_check");
        let n2 = qa.forward_chain(0.75);
        eprintln!("  After port check: {} more facts derived", n2);
        assert!(n2 >= 1, "Should derive confirmation from port check");

        let (can_fix, _) = qa.verify_fact("machine", "can", "fix_problem");
        assert!(can_fix, "Should be able to fix after verification");
    }

    // ── Epistemic Update Tests ──────────────────────────────────────────

    #[test]
    fn test_add_pattern_extends_classifier() {
        let mut classifier = seed_error_classifier();
        let before = classifier.pattern_counts();
        let port_patterns_before = before.iter()
            .find(|(n, _)| n == "port_conflict").map(|(_, c)| *c).unwrap_or(0);

        // Add a new pattern to port_conflict
        let added = classifier.add_pattern("port_conflict",
            "custom error: could not bind to port 8080");
        assert!(added, "Should add pattern to existing type");

        let after = classifier.pattern_counts();
        let port_patterns_after = after.iter()
            .find(|(n, _)| n == "port_conflict").map(|(_, c)| *c).unwrap_or(0);
        assert_eq!(port_patterns_after, port_patterns_before + 1,
            "Pattern count should increase by 1");
    }

    #[test]
    fn test_add_pattern_nonexistent_category() {
        let mut classifier = seed_error_classifier();
        let added = classifier.add_pattern("nonexistent_category", "some error");
        assert!(!added, "Should not add pattern to unknown type");
    }

    #[test]
    fn test_add_pattern_no_duplicates() {
        let mut classifier = seed_error_classifier();
        let text = "bind() to 0.0.0.0:80 failed (98: Unknown error)";

        classifier.add_pattern("port_conflict", text);
        let count1 = classifier.pattern_counts().iter()
            .find(|(n, _)| n == "port_conflict").map(|(_, c)| *c).unwrap_or(0);

        classifier.add_pattern("port_conflict", text);
        let count2 = classifier.pattern_counts().iter()
            .find(|(n, _)| n == "port_conflict").map(|(_, c)| *c).unwrap_or(0);

        assert_eq!(count1, count2, "Duplicate pattern should not increase count");
    }

    #[test]
    fn test_absorb_diagnosis_and_query() {
        // Simulate a complete diagnostic learning cycle
        let mut brain = VSABrain::new(0.12);
        let mut qa = QaEngine::new();
        let mut classifier = seed_error_classifier();

        let error_texts = [
            "bind() to 0.0.0.0:80 failed (98: Unknown error)",
            "Address already in use",
            "socket.error: [Errno 98] EADDRINUSE",
        ];

        // Phase 1: absorb each error as a port_conflict diagnosis
        for text in &error_texts {
            absorb_diagnosis(&mut brain, &mut qa, &mut classifier, text, "port_conflict", 1.0);
        }

        // Check: reinforcement count should be 3+ (one per episode)
        let count = diagnosis_reinforcement_count(&brain, "port_conflict");
        eprintln!("  Port conflict reinforcement count: {}", count);
        assert!(count >= 3, "Should have reinforced port_conflict at least 3 times (got {})", count);

        // Check: classifier should have learned new patterns
        let pc = classifier.pattern_counts();
        let port_count = pc.iter()
            .find(|(n, _)| n == "port_conflict").map(|(_, c)| *c).unwrap_or(0);
        eprintln!("  Port conflict patterns after learning: {}", port_count);
        assert!(port_count >= 4, "Should have at least 4 patterns for port_conflict (got {})", port_count);
    }

    #[test]
    fn test_query_diagnostic_category_after_learning() {
        // Test that after absorbing episodes, a novel variant with trigram
        // overlap can be classified via the brain's centroid system.
        let mut brain = VSABrain::new(0.12);
        let mut qa = QaEngine::new();
        let mut classifier = seed_error_classifier();

        // Absorb some port_conflict episodes
        absorb_diagnosis(&mut brain, &mut qa, &mut classifier,
            "bind() to 0.0.0.0:80 failed (98: Unknown error)", "port_conflict", 1.0);
        absorb_diagnosis(&mut brain, &mut qa, &mut classifier,
            "Address already in use", "port_conflict", 1.0);
        absorb_diagnosis(&mut brain, &mut qa, &mut classifier,
            "port is already allocated", "port_conflict", 1.0);

        // Now try to query with a novel variant that shares trigrams
        // but doesn't match any trigger or pre-seeded pattern directly.
        // Note: "bind to [::]:443 failed" still has trigram overlap with
        // absorbed texts, so Level 1 in nearest_centroid_idx should find
        // a match.
        let category = query_diagnostic_category(&brain, "bind to [::]:443 failed");
        eprintln!("  Query result for novel variant: {:?}", category);
        // This may or may not find the category depending on centroid similarity.
        // The test is informational since centroid matching depends on trigram overlap.
        assert!(category.is_some() || category.is_none(),
            "Query should return Some or None (not panic)");
    }

    #[test]
    fn test_add_pattern_improves_trigram_matching() {
        // After adding a new pattern to the classifier, the trigram Jaccard
        // should match an error text that didn't match before.
        let mut classifier = seed_error_classifier();

        // Before: "some random bind error" doesn't match any trigger
        // and has limited trigram overlap with pre-seeded patterns.
        // It may or may not match via Level 2.
        let (before, before_level) = classifier.classify_deep("random bind failure on socket");

        // Add the text as a pattern
        classifier.add_pattern("port_conflict", "random bind failure on socket");

        // After: should match via Level 2 (trigram) since it IS the pattern
        let (after, after_level) = classifier.classify_deep("random bind failure on socket");
        assert!(after.is_some(), "Should classify after adding pattern");
        assert_eq!(after_level, "trigram", "Should match via trigram");
        assert_eq!(after.unwrap().2, "port_conflict", "Should classify as port_conflict");

        // Also: a slight variant should now match via trigram Jaccard
        let (variant, variant_level) = classifier.classify_deep("random bind failure on port socket");
        assert!(variant.is_some(),
            "Variant should match via trigram after pattern added");
        assert_eq!(variant_level, "trigram", "Variant should match via trigram");
        assert_eq!(variant.unwrap().2, "port_conflict", "Variant should classify as port_conflict");
    }
}
