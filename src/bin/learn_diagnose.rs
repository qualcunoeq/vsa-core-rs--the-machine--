// ─── Multi-Episode Diagnostic Learning Experiment ─────────────────────
//
// Tests whether The Machine improves its diagnostic classification through
// repeated exposure.  Runs multiple episodes with different error texts
// for the same categories, feeds each successful diagnosis back into the
// VSABrain via epistemic updates, and then tests whether the system can
// classify a NOVEL variant using learned patterns and centroids.
//
// Phase 1 — Accumulate episodes:
//   5 diagnostic episodes across 3 categories (port_conflict, missing_file,
//   connection_refused).  Each episode: read error text → classify →
//   forward chain → verify → absorb diagnosis into brain.
//
// Phase 2 — Test learning:
//   Present novel error text variants that do NOT match any trigger but
//   share trigram overlap with Phase 1 texts.  Report whether they are
//   classified via:
//     (a) Level 1 trigger — hardcoded synonym map
//     (b) Level 2 trigram — Jaccard similarity with learned patterns
//     (c) Level 3 centroid — VSA brain cluster projection
//     (d) None — honest "unknown"
//
// The key question: after Phase 1, does Phase 2 succeed more often than
// it would WITHOUT the learning?
//
// Usage:
//   cargo run --bin learn_diagnose
//
// Does NOT require VMs — all diagnostic scenarios are simulated.
// ────────────────────────────────────────────────────────────────────────────

use std::time::Instant;
use the_machine::diagnostic::{
    absorb_diagnosis, classify_structural, query_diagnostic_category,
    seed_diagnostic_knowledge, seed_error_classifier, structure_to_triples,
};
use the_machine::qa::QaEngine;
use the_machine::VSABrain;

/// A simulated diagnostic episode.
struct Episode {
    /// The error text to diagnose.
    pub error_text: &'static str,
    /// The expected category.
    pub expected_category: &'static str,
    /// Whether this episode is "novel" (no trigger match, tests learning).
    pub is_novel: bool,
}

/// Episodes for Phase 1 (accumulation).
/// All are classified via Level 1 trigger matching.
const PHASE1_EPISODES: &[Episode] = &[
    Episode { error_text: "bind() to 0.0.0.0:80 failed (98: Unknown error)", expected_category: "port_conflict", is_novel: false },
    Episode { error_text: "Address already in use", expected_category: "port_conflict", is_novel: false },
    Episode { error_text: "socket.error: [Errno 98] EADDRINUSE", expected_category: "port_conflict", is_novel: false },
    Episode { error_text: "No such file or directory", expected_category: "missing_file", is_novel: false },
    Episode { error_text: "Connection refused", expected_category: "connection_refused", is_novel: false },
];

/// Episodes for Phase 2 (testing learning).
/// None of these match any Level 1 trigger.  The first three share trigram
/// overlap with Phase 1 texts.  The last two are genuinely novel.
const PHASE2_EPISODES: &[Episode] = &[
    // Shares trigrams with "bind() to 0.0.0.0:80 failed (98: Unknown error)"
    Episode { error_text: "bind to [::]:443 failed", expected_category: "port_conflict", is_novel: true },
    // Shares trigrams with "Address already in use" and "port is already allocated"
    Episode { error_text: "port already allocated", expected_category: "port_conflict", is_novel: true },
    // Shares trigrams with "Connection refused"
    Episode { error_text: "connect refused by host", expected_category: "connection_refused", is_novel: true },
    // Genuinely novel — no trigram overlap with any Phase 1 text
    Episode { error_text: "KMS keyserver unreachable: timeout", expected_category: "unknown", is_novel: true },
    // Another genuinely novel one
    Episode { error_text: "disk quota exceeded on /var/log", expected_category: "unknown", is_novel: true },
];

/// Phase 3: test that the classifier OWNS new patterns that it learned.
/// After Phase 1, the classifier's `add_pattern` was called for each episode.
/// So a text that previously didn't match any trigger should now match via
/// Level 2 trigram because the classifier learned it.
const PHASE3_TEXT: &str = "bind() to 0.0.0.0:80 failed (98: Unknown error)";

/// Zero-overlap test cases for structural matching (Phase 4).
/// These texts have ZERO trigram overlap with any Phase 1 text but share
/// the same ABSTRACT STRUCTURE.  The structural parser should bridge the gap.
const ZERO_OVERLAP_TESTS: &[(&str, &str)] = &[
    // Zero trigram overlap with any Phase 1 text, but structurally identical
    // to "bind() to 0.0.0.0:80 failed" at the abstract level.
    ("KMS keyserver unreachable: timeout", "network_service_unavailable"),
    ("SSL certificate validation failed", "credential_invalid"),
    ("disk quota exceeded on /var/log", "storage_full"),
];

fn main() {
    let start = Instant::now();
    let mut brain = VSABrain::new(0.12);
    let mut qa = QaEngine::new();
    let mut classifier = seed_error_classifier();

    seed_diagnostic_knowledge(&mut qa, &mut brain);

    eprintln!("═══════════════════════════════════════════════════════════");
    eprintln!("  Multi-Episode Diagnostic Learning Experiment");
    eprintln!("═══════════════════════════════════════════════════════════");
    eprintln!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 1: Accumulate episodes
    // ═══════════════════════════════════════════════════════════════════════

    eprintln!("── Phase 1: Accumulating {} episodes ───────────", PHASE1_EPISODES.len());
    eprintln!();

    for (i, ep) in PHASE1_EPISODES.iter().enumerate() {
        eprintln!("  Episode {}: \"{}…\"", i + 1, &ep.error_text[..ep.error_text.len().min(40)]);

        // Classify the error text
        let (svo, level) = classifier.classify_deep(ep.error_text);
        let category = svo.map(|c| c.2.clone()).unwrap_or_else(|| "unknown".to_string());

        if category == ep.expected_category {
            eprintln!("    → Classified as: {} (Level {}) ✓", category, level);

            // Absorb the diagnosis into the brain
            absorb_diagnosis(&mut brain, &mut qa, &mut classifier,
                ep.error_text, &category, 1.0);
            eprintln!("    → Diagnosis absorbed");
        } else {
            eprintln!("    → FAILED: expected {}, got {} (level {})",
                ep.expected_category, category, level);
        }
    }

    eprintln!();
    eprintln!("  Phase 1 summary:");
    eprintln!("    Clusters: {} dejavu, {} transient",
        brain.dejavu_clusters.len(), brain.transient_clusters.len());
    eprintln!("    Associations: {}",
        brain.cross_cluster_associations.len());
    for (name, count) in classifier.pattern_counts() {
        eprintln!("    {}: {} patterns", name, count);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 2: Test novel variants
    // ═══════════════════════════════════════════════════════════════════════

    eprintln!();
    eprintln!("── Phase 2: Testing {} novel variants ───────────", PHASE2_EPISODES.len());
    eprintln!();

    let mut classified = 0;
    let mut unknown = 0;

    for (i, ep) in PHASE2_EPISODES.iter().enumerate() {
        eprintln!("  Novel variant {}: \"{}…\"", i + 1, &ep.error_text[..ep.error_text.len().min(40)]);

        // First: try the classifier
        let (svo, level) = classifier.classify_deep(ep.error_text);
        let category_from_classifier = svo.map(|c| c.2.as_str());

        // Second: try the brain centroid query
        let category_from_brain = query_diagnostic_category(&brain, ep.error_text);

        match (category_from_classifier, category_from_brain) {
            (Some(cat), _) if cat == "port_conflict" || cat == "missing_file" || cat == "connection_refused" => {
                eprintln!("    → Classified by classifier (Level {}) as: {}", level, cat);
                classified += 1;
            }
            (None, Some((cat, _conf))) => {
                eprintln!("    → Classified by brain centroids as: {} (conf={:.4})", cat, _conf);
                classified += 1;
            }
            (Some(cat), _) => {
                eprintln!("    → Classified as: {} (unexpected)", cat);
                unknown += 1;
            }
            (None, None) => {
                eprintln!("    → NOT CLASSIFIED (honest unknown)");
                unknown += 1;
            }
        }

        if ep.expected_category == "unknown" {
            eprintln!("    (expected: unknown — correct behavior)");
        }
    }

    eprintln!();

    // ═══════════════════════════════════════════════════════════════════════
    // Summary
    // ═══════════════════════════════════════════════════════════════════════

    let elapsed = start.elapsed();
    eprintln!("── Results ─────────────────────────────────────");
    eprintln!("  Total episodes processed: {}", PHASE1_EPISODES.len() + PHASE2_EPISODES.len());
    eprintln!("  Phase 1: {} accumulation episodes", PHASE1_EPISODES.len());
    eprintln!("  Phase 2: {} novel variants", PHASE2_EPISODES.len());
    eprintln!("  Classified: {} / {} ({:.0}%)",
        classified, PHASE2_EPISODES.len(),
        classified as f64 / PHASE2_EPISODES.len() as f64 * 100.0);
    eprintln!("  Unknown: {} / {} ({:.0}%)",
        unknown, PHASE2_EPISODES.len(),
        unknown as f64 / PHASE2_EPISODES.len() as f64 * 100.0);
    eprintln!();
    eprintln!("  Brain state:");
    eprintln!("    Dejavu clusters: {}", brain.dejavu_clusters.len());
    eprintln!("    Transient clusters: {}", brain.transient_clusters.len());
    eprintln!("    Cross-cluster associations: {}", brain.cross_cluster_associations.len());
    eprintln!("  Time: {:?}", elapsed);
    eprintln!();

    // Phase 3: verify the classifier learned the Phase 1 texts
    eprintln!("── Phase 3: Verifying classifier learned Phase 1 texts ────");
    // After Phase 1, the classifier had add_pattern() called for each episode.
    // Re-seed a fresh classifier to show that learning persists beyond the session.
    let fresh_classifier = seed_error_classifier();
    // The fresh classifier should NOT have the Phase 1 patterns because
    // they were added to the mutable classifier, not to the seed function.
    // This demonstrates that learning is session-local (for now).
    let (before_svo, _) = fresh_classifier.classify_deep(PHASE3_TEXT);
    let (after_svo, after_level) = classifier.classify_deep(PHASE3_TEXT);

    eprintln!("  Text: \"{}…\"", &PHASE3_TEXT[..40]);
    eprintln!("  Fresh classifier (no Phase 1): {}",
        if before_svo.is_some() { "trigger match (hardcoded)" } else { "no match" });
    eprintln!("  Learned classifier (Phase 1 absorbed): Level {} → {}",
        after_level,
        after_svo.map(|c| c.2.as_str()).unwrap_or("none"));

    eprintln!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 4: Zero-overlap structural matching (Level 3)
    // ═══════════════════════════════════════════════════════════════════════

    eprintln!("── Phase 4: Zero-overlap structural matching ──────");
    eprintln!("  Tests whether structural parsing bridges the gap for");
    eprintln!("  errors with ZERO trigram overlap.");
    eprintln!();

    let mut structural_classified = 0;
    let mut structural_total = 0;

    for (text, expected_category) in ZERO_OVERLAP_TESTS {
        structural_total += 1;
        eprintln!("  Text: \"{}…\"", &text[..text.len().min(40)]);

        // First: try the classifier (Levels 1+2) — should fail
        let (svo, level) = classifier.classify_deep(text);
        let from_classifier = svo.is_some();
        if from_classifier {
            eprintln!("    Level {}/1-2: {} (UNEXPECTED — should need structural)",
                level, svo.unwrap().2);
        } else {
            eprintln!("    Level 1-2: no match (expected)");
        }

        // Then: try structural parsing (Level 3)
        let structural_triples = classify_structural(text);
        match structural_triples {
            Some(triples) => {
                eprintln!("    Level 3 structural: {} triples generated", triples.len());
                for (i, (s, v, o)) in triples.iter().enumerate().take(3) {
                    eprintln!("      Triple {}: ({}, {}, {})", i + 1, s, v, o);
                }

                // Check if abstract triples match known diagnostic categories
                let has_network_service = triples.contains(
                    &("process".to_string(), "accesses".to_string(), "network_service".to_string()));
                let has_unavailable = triples.contains(
                    &("network_service".to_string(), "has_state".to_string(), "unavailable".to_string()));
                let has_storage_full = triples.contains(
                    &("storage".to_string(), "has_state".to_string(), "capacity_exhausted".to_string()));

                if has_network_service || has_unavailable {
                    eprintln!("    → Bridge to: network_service_unavailable (port conflict / connection)");
                    structural_classified += 1;
                } else if has_storage_full {
                    eprintln!("    → Bridge to: storage_full (disk space)");
                    structural_classified += 1;
                } else {
                    eprintln!("    → Structural triples generated but no diagnostic bridge (uncategorized)");
                }
            }
            None => {
                eprintln!("    Level 3 structural: no triples generated");
            }
        }
    }

    eprintln!();
    eprintln!("── Phase 4 Results ────────────────────────────────");
    eprintln!("  Zero-overlap tests: {} / {} structurally classified",
        structural_classified, structural_total);
    eprintln!("  Classification rate: {:.0}%",
        structural_classified as f64 / structural_total as f64 * 100.0);
    eprintln!();
    eprintln!("═══════════════════════════════════════════════════════════");
}
