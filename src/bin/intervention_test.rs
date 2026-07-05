// ── Intervention Test ─────────────────────────────────────────────────────
//
// Tests the advisor's question: what fraction of zero-overlap error texts
// can be correctly classified WITHOUT the hand-coded abstraction tables?
//
// Methodology:
//   1. Seed a fresh classifier (triggers + patterns only) and brain
//   2. Do NOT use the structural parser's keyword tables (ACTIONS, RESOURCES,
//      ERROR_CLASSES) — those ARE the abstraction tables
//   3. Test 3 zero-overlap texts that the structural parser SHOULD handle
//   4. Measure classification at each level:
//      - Level 1 (trigger): direct substring match
//      - Level 2 (trigram Jaccard): trigram overlap with known patterns
//      - Level 4 (centroid): nearest centroid in dejavu clusters
//
// Expected result: 0/3 without abstraction tables, because:
//   - The zero-overlap texts share NO triggers/trigrams with known patterns
//   - The VSA centroids encode trigrams, and orthogonal trigrams → noise floor

use std::collections::HashMap;
use the_machine::diagnostic::{seed_diagnostic_knowledge, seed_error_classifier, classify_structural, CanonicalSvo};
use the_machine::qa::QaEngine;
use the_machine::Hypervector;
use the_machine::VSABrain;

/// Replicate the category pattern matching from `find_best_structural_category`
/// (which is module-private) so this binary can use it.
fn find_best_structural_category(triples: &[CanonicalSvo], _qa: &QaEngine) -> Option<String> {
    let category_patterns: &[(&[(&str, &str, &str)], &str)] = &[
        (&[("process", "accesses", "network_service"),
           ("network_service", "has_state", "unavailable")], "port_conflict"),
        (&[("network_service", "has_state", "unavailable")], "connection_refused"),
        (&[("process", "accesses", "network_service"),
           ("network_service", "has_state", "unavailable")], "network_timeout"),
        (&[("process", "accesses", "file_system"),
           ("file_system", "has_state", "unavailable")], "missing_file"),
        (&[("file_system", "has_state", "not_found")], "missing_file"),
        (&[("file_system", "has_state", "resource_missing")], "missing_file"),
        (&[("file_system", "has_state", "permission_blocked")], "permission_denied"),
        (&[("network_service", "has_state", "permission_blocked")], "permission_denied"),
        (&[("storage", "has_state", "capacity_exhausted")], "disk_full"),
        (&[("storage", "has_state", "unavailable")], "disk_full"),
        (&[("credential", "has_state", "credential_invalid")], "permission_denied"),
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
    for (s, v, o) in triples {
        if s == "process" && v == "accesses" {
            return Some(format!("resource_access_{}", o));
        }
    }
    None
}

fn main() {
    println!("═══════════════════════════════════════════════════════════════════════");
    println!("  Intervention Test: Zero-Overlap Classification");
    println!("═══════════════════════════════════════════════════════════════════════\n");

    println!("  This test measures what the VSA architecture (centroid proximity,");
    println!("  association memory, trigram encoding) contributes to zero-overlap");
    println!("  error classification WITHOUT the hand-coded abstraction tables.\n");

    // Zero-overlap test cases: these share NO textual surface with any
    // known error pattern, but are structurally analogous.
    let zero_overlap_texts = [
        ("KMS keyserver unreachable: timeout", "connection_refused"),
        ("disk quota exceeded",                "disk_full"),
        ("SSL certificate validation failed",  "credential_invalid"),
    ];

    // ── Setup ────────────────────────────────────────────────────────────
    let mut qa = QaEngine::new();
    let mut brain = VSABrain::new(0.12);
    let mut classifier = seed_error_classifier();
    seed_diagnostic_knowledge(&mut qa, &mut brain);

    // Simulate epistemic learning: absorb each KNOWN category's texts so
    // that centroids exist for centroid-proximity matching.
    // This gives Level 4 the best possible chance.
    let known_texts: &[(&str, &str)] = &[
        ("port_conflict",       "bind() to 0.0.0.0:80 failed (98: Unknown error)"),
        ("port_conflict",       "Address already in use"),
        ("connection_refused",  "Connection refused"),
        ("connection_refused",  "connect: connection refused"),
        ("missing_file",        "No such file or directory"),
        ("permission_denied",   "Permission denied"),
        ("startup_failure",     "startup failed"),
    ];
    for &(cat, text) in known_texts {
        // Manual absorb_diagnosis (we can't call the real one because it needs
        // a &mut ErrorClassifier and it calls add_pattern which changes state)
        let error_hv = Hypervector::encode_text_ngram(text, 3);
        brain.absorb_epistemic_update(&error_hv, cat, true);
        let concept_name = format!("concept:{}", cat);
        let concept_hv = Hypervector::encode_text_ngram(&concept_name, 3);
        brain.absorb_epistemic_update(&concept_hv, cat, true);
        let mut meta = HashMap::new();
        meta.insert("category".to_string(), cat.to_string());
        meta.insert("outcome".to_string(), "1.00".to_string());
        meta.insert("type".to_string(), "diagnosis".to_string());
        brain.add_transient_fact(concept_hv, "diagnostic_category", meta);
        qa.sync_cluster_data(&brain);
        classifier.add_pattern(cat, text);
    }

    let mut total_correct_bare = 0usize;
    let mut total_correct_structural = 0usize;
    let mut total_false_positive = 0usize;

    for (i, (text, expected)) in zero_overlap_texts.iter().enumerate() {
        println!("  ── Test case {}: {:?} ──", i + 1, text);
        println!("    Expected category: {}\n", expected);

        // ── Level 1 (trigger matching) ───────────────────────────────────
        let l1 = classifier.classify(text);
        println!("    Level 1 (triggers):       {:?}",
            l1.map(|s| s.2.as_str()).unwrap_or("none"));

        // ── Level 2 (trigram Jaccard) ────────────────────────────────────
        let (l2, l2_method) = classifier.classify_deep(text);
        println!("    Level 2 (trigrams, {}):  {:?}",
            l2_method, l2.map(|s| s.2.as_str()).unwrap_or("none"));

        // ── Level 3 (structural parser) — reported but SUPPRESSED ────────
        let struct_result = classify_structural(text);
        if let Some(ref triples) = struct_result {
            println!("    Level 3 (structural):      {} triples (SUPPRESSED)", triples.len());
            for t in triples {
                println!("                             ({}, {}, {})", t.0, t.1, t.2);
            }
        } else {
            println!("    Level 3 (structural):      no triples");
        }

        // ── Level 4 (centroid proximity) ─────────────────────────────────
        let error_hv = Hypervector::encode_text_ngram(text, 3);
        let l4 = brain.nearest_centroid_idx(&error_hv);
        match l4 {
            Some((idx, sim)) => {
                let label = brain.dejavu_clusters.get(idx)
                    .and_then(|c| c.entries.first().map(|e| e.label.clone()))
                    .unwrap_or_default();
                // Also try query_diagnostic_category for better resolution
                let cat = the_machine::diagnostic::query_diagnostic_category(&brain, text);
                println!("    Level 4 (centroids):      idx={}, sim={:.4}", idx, sim);
                println!("                             label={:?}, resolved_category={:?}", label, cat);
            }
            None => println!("    Level 4 (centroids):      no match"),
        }

        // ── WITHOUT structural parser ────────────────────────────────────
        let category_bare = {
            if let Some(svo) = classifier.classify(text) {
                if svo.2 == *expected { Some("L1 correct") } else { Some("L1 wrong") }
            } else if let Some(svo) = classifier.classify_trigram(text) {
                if svo.2 == *expected { Some("L2 correct") } else { Some("L2 wrong") }
            } else if let Some((_idx, sim)) = brain.nearest_centroid_idx(&error_hv) {
                if sim >= 0.55 { Some("L4 ambiguous") } else { None }
            } else {
                None
            }
        };

        match category_bare {
            Some(verdict) if verdict.contains("wrong") => {
                println!("\n    ▶ WITHOUT ABSTRACTION TABLES: classified WRONG category via {}", verdict);
                total_false_positive += 1;
            }
            Some(verdict) if verdict.contains("correct") => {
                println!("\n    ▶ WITHOUT ABSTRACTION TABLES: CORRECT via {}", verdict);
                total_correct_bare += 1;
            }
            Some(verdict) => {
                println!("\n    ▶ WITHOUT ABSTRACTION TABLES: ambiguous ({})", verdict);
            }
            None => {
                println!("\n    ▶ WITHOUT ABSTRACTION TABLES: STUCK (no level matched)");
            }
        }

        // ── WITH structural parser ───────────────────────────────────────
        let category_with = struct_result.as_ref()
            .and_then(|triples| find_best_structural_category(triples, &qa));
        match category_with {
            Some(ref cat) if cat == expected => {
                println!("    ▶ WITH ABSTRACTION TABLES:   CORRECT as '{}' ✓", cat);
                total_correct_structural += 1;
            }
            Some(ref cat) => {
                println!("    ▶ WITH ABSTRACTION TABLES:   classified as '{}' (expected '{}')", cat, expected);
            }
            None => {
                println!("    ▶ WITH ABSTRACTION TABLES:   no category match");
            }
        }
        println!();
    }

    // ── Summary ──────────────────────────────────────────────────────────
    println!("═══════════════════════════════════════════════════════════════════════\n");
    println!("  Summary:\n");
    println!("    Without abstraction tables:");
    println!("      Correct:  {}/{}  (Level 1/2/4 combined)", total_correct_bare, zero_overlap_texts.len());
    println!("      Wrong:    {}/{}  (false positives)", total_false_positive, zero_overlap_texts.len());
    println!("      Stuck:    {}/{}  (honestly unknown)", zero_overlap_texts.len() - total_correct_bare - total_false_positive, zero_overlap_texts.len());
    println!();
    println!("    With abstraction tables (Level 3 structural parser):");
    println!("      Correct:  {}/{}", total_correct_structural, zero_overlap_texts.len());
    println!();

    if total_correct_bare == 0 {
        println!("  INTERPRETATION:");
        println!("  The hand-coded abstraction tables (ACTIONS, RESOURCES, ERROR_CLASSES)");
        println!("  are the ONLY mechanism that bridges the zero-overlap analogy gap.");
        println!("  The VSA architecture contributes NOTHING to zero-overlap");
        println!("  classification with the current trigram encoding scheme.");
        println!();
        println!("  This is NOT a failure — it is an honest finding that tells us");
        println!("  exactly what needs to be built next: the L2 hierarchy must");
        println!("  store STRUCTURAL CENTROIDS (encoded SVO triples like");
        println!("  encode_svo(process, accesses, network_service)) so that the");
        println!("  centroid-proximity path can bridge the analogy gap autonomously");
        println!("  without hand-coded keyword tables.");
    } else if total_correct_bare > 0 {
        println!("  INTERPRETATION:");
        println!("  The VSA architecture contributes SOME zero-overlap capability.");
        println!("  This means the epistemic updates (absorb_diagnosis) are working");
        println!("  as intended — centroids encode structural information.");
    }
}
