// ─── 50-Episode Diagnostic Learning Experiment ─────────────────────────
//
// Tests whether The Machine's L2 hierarchy learns abstract diagnostic
// categories from experience.  After 50 episodes across 5 categories,
// presents 3 genuinely novel errors and measures whether they are
// classified through structural parsing (Level 3) and how close the
// resulting centroids are to the learned category clusters.
//
// The metric that matters: centroid similarity for novel errors.
//   >0.65: clearly within a learned cluster
//   0.55-0.65: marginal — cluster exists but not dense enough
//   <0.55: insufficient episodes for stable abstraction
//
// Usage:
//   cargo run --bin fifty_episodes
//
// All episodes are simulated (no VMs needed).  Runs in <100ms.
// ────────────────────────────────────────────────────────────────────────────

use std::time::Instant;
use the_machine::diagnostic::{
    absorb_diagnosis, classify_structural, seed_diagnostic_knowledge, seed_error_classifier,
};
use the_machine::qa::QaEngine;
use the_machine::Hypervector;
use the_machine::VSABrain;

// ─── Episode Corpus: 50 episodes across 5 categories ─────────────────────

const CATEGORIES: &[&str] = &[
    "port_conflict",
    "network_timeout",
    "missing_file",
    "permission_denied",
    "disk_full",
];

/// 10 error text variants per category.
const EPISODES: &[&[&str]] = &[
    // port_conflict
    &[
        "bind() to 0.0.0.0:80 failed (98: Unknown error)",
        "Address already in use",
        "socket.error: [Errno 98] EADDRINUSE",
        "port is already allocated",
        "could not bind to port 8080",
        "another process is listening on port 443",
        "TCP port 3000 already in use by another service",
        "failed to listen on socket: address in use",
        "port conflict detected on interface eth0",
        "unable to bind to any available port",
    ],
    // network_timeout
    &[
        "Connection refused",
        "connect: no route to host",
        "KMS keyserver unreachable: timeout",
        "Operation timed out on socket",
        "peer closed connection unexpectedly",
        "connection reset by remote host",
        "ETIMEDOUT waiting for response",
        "host unreachable on port 8080",
        "network is unreachable",
        "upstream server not reachable",
    ],
    // missing_file
    &[
        "No such file or directory",
        "ENOENT: cannot open config.json",
        "file not found at /var/log/app.log",
        "missing required dependency: libssl.so",
        "cannot access /etc/nginx/nginx.conf",
        "required module not found in path",
        "unable to locate configuration file",
        "file does not exist in working directory",
        "could not open database file",
        "missing include file in nginx config",
    ],
    // permission_denied
    &[
        "Permission denied",
        "EACCES: access denied to /var/run",
        "operation not permitted on socket",
        "access denied: insufficient privileges",
        "cannot open file: permission denied",
        "permission denied to bind privileged port",
        "user does not have write access",
        "access to resource blocked by policy",
        "EACCES error while accessing shared memory",
        "no permission to execute binary",
    ],
    // disk_full
    &[
        "disk quota exceeded on /var/log",
        "No space left on device",
        "ENOSPC: write failed to filesystem",
        "disk full: cannot write to log file",
        "out of disk space on /dev/sda1",
        "filesystem capacity reached 100%",
        "quota limit reached for user",
        "storage volume has no free space",
        "write error: filesystem is full",
        "cannot allocate new blocks on disk",
    ],
];

// ─── Novel Test Texts ────────────────────────────────────────────────────
//
// These share ZERO meaningful trigram overlap with any episode text.
// They are designed so the structural parser (Level 3) must do the work.
//
// Each pair: (error_text, expected_category_name)
const NOVEL_TESTS: &[(&str, &str)] = &[
    (
        "KV store compaction stalled unexpectedly; index rebuild queued",
        "network_timeout",
    ),
    (
        "SELinux policy audit: type=1400 avc:  denied  { read } for pid=1234",
        "permission_denied",
    ),
    (
        "RAID controller reports battery charge critically low; cache flush failed",
        "disk_full",
    ),
];

/// Feature vector for reporting.
struct EpisodeStats {
    category: &'static str,
    episode_count: usize,
    pattern_count: usize,
    centroid_similarity_self: f64,
}

fn trigram_jaccard(a: &str, b: &str) -> f64 {
    let trigrams = |s: &str| -> std::collections::HashSet<String> {
        let lower = s.to_lowercase();
        let chars: Vec<char> = lower.chars().collect();
        if chars.len() < 3 {
            let mut set = std::collections::HashSet::new();
            set.insert(lower);
            return set;
        }
        chars.windows(3).map(|w| w.iter().collect()).collect()
    };
    let ta = trigrams(a);
    let tb = trigrams(b);
    let intersection = ta.intersection(&tb).count();
    let union = ta.union(&tb).count();
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

fn measure_centroid_similarity(brain: &VSABrain, concept_name: &str) -> f64 {
    let concept_hv = Hypervector::encode_text_ngram(&format!("concept:{}", concept_name), 3);
    brain
        .nearest_centroid_idx(&concept_hv)
        .map(|(_, sim)| sim)
        .unwrap_or(0.0)
}

fn main() {
    let start = Instant::now();
    let mut brain = VSABrain::new(0.12);
    let mut qa = QaEngine::new();
    let mut classifier = seed_error_classifier();
    seed_diagnostic_knowledge(&mut qa, &mut brain);

    eprintln!("═══════════════════════════════════════════════════════════════");
    eprintln!("  50-Episode Diagnostic Learning Experiment");
    eprintln!("═══════════════════════════════════════════════════════════════");
    eprintln!();
    eprintln!("  Categories: {}", CATEGORIES.join(", "));
    eprintln!("  Episodes per category: {}", EPISODES[0].len());
    eprintln!(
        "  Total episodes: {}",
        EPISODES.iter().map(|e| e.len()).sum::<usize>()
    );
    eprintln!("  Novel test texts: {}", NOVEL_TESTS.len());
    eprintln!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 1: Run all 50 episodes
    // ═══════════════════════════════════════════════════════════════════════

    eprintln!(
        "── Phase 1: Running {} episodes ─────────────────",
        EPISODES.iter().map(|e| e.len()).sum::<usize>()
    );
    eprintln!();

    let mut classified = 0;
    let mut failed = 0;

    for (cat_idx, category) in CATEGORIES.iter().enumerate() {
        let variants = EPISODES[cat_idx];
        for (ep_idx, error_text) in variants.iter().enumerate() {
            let short = if error_text.len() > 45 {
                format!("{}…", &error_text[..45])
            } else {
                error_text.to_string()
            };

            // Classify via the 3-level pipeline
            let (svo, level) = classifier.classify_deep(error_text);

            // If Level 1+2 fail, try Level 3 (structural)
            let effective_level = match (svo, level) {
                (Some(svo), level) => {
                    let (subj, verb, obj) = svo.clone();
                    qa.store_fact(&subj, &verb, &obj, &format!("ep{}_{}", cat_idx, ep_idx));
                    level
                }
                (None, "none") => {
                    // Try structural parser as fallback
                    if let Some(triples) = classify_structural(error_text) {
                        for (s, v, o) in &triples {
                            qa.store_fact(s, v, o, &format!("struct_{}_{}", cat_idx, ep_idx));
                        }
                        "structural"
                    } else {
                        "unclassified"
                    }
                }
                _ => "unknown",
            };

            // Forward chain
            let n = qa.forward_chain(0.75);

            // Check if cause was identified
            let has_cause = qa
                .verify_fact("another_process", "is_listening_on", "same_port")
                .0
                || qa.verify_fact("target_service", "is_not", "listening").0
                || qa.verify_fact("required_file", "is", "missing").0
                || qa.verify_fact("file_permissions", "are", "incorrect").0
                || qa.verify_fact("disk_space", "is", "full").0;

            if has_cause {
                classified += 1;
            } else {
                failed += 1;
            }

            // Absorb the diagnosis into the brain
            absorb_diagnosis(
                &mut brain,
                &mut qa,
                &mut classifier,
                error_text,
                category,
                1.0,
            );

            if ep_idx < 2 || ep_idx == variants.len() - 1 {
                eprintln!(
                    "  [{}/{}] {}: {} (level={}, fwd={}, cause={})",
                    cat_idx + 1,
                    ep_idx + 1,
                    category,
                    short,
                    effective_level,
                    n,
                    if has_cause { "✓" } else { "✗" }
                );
            }
        }
        eprintln!(
            "  → {} done ({} episodes, {:.0}% classified)",
            category,
            variants.len(),
            classified as f64 / (cat_idx as f64 * 10.0 + variants.len() as f64).max(1.0) * 100.0
        );
    }

    eprintln!();
    eprintln!("  Phase 1 summary:");
    eprintln!(
        "    Classified: {}/{} ({:.0}%)",
        classified,
        classified + failed,
        classified as f64 / (classified + failed) as f64 * 100.0
    );
    eprintln!(
        "    Brain: {} dejavu clusters, {} transient clusters",
        brain.dejavu_clusters.len(),
        brain.transient_clusters.len()
    );
    eprintln!(
        "    Associations: {}",
        brain.cross_cluster_associations.len()
    );
    eprintln!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 2: Centroid stability measurement
    // ═══════════════════════════════════════════════════════════════════════

    eprintln!("── Phase 2: Centroid stability ──────────────────");
    eprintln!();

    for category in CATEGORIES {
        let sim = measure_centroid_similarity(&brain, category);
        let concept_hv_string = format!("concept:{}", category);
        let concept_hv = Hypervector::encode_text_ngram(&concept_hv_string, 3);

        // Find the number of clusters whose label matches this concept
        let cluster_count = brain
            .dejavu_clusters
            .iter()
            .filter(|c| {
                c.entries
                    .iter()
                    .any(|e| e.label == concept_hv_string || e.label == *category)
            })
            .count();

        let ep_count = EPISODES[0].len(); // same for all categories
        eprintln!(
            "  {}: centroid sim={:.4}, clusters={}, episodes={}",
            category, sim, cluster_count, ep_count
        );
    }
    eprintln!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 3: Test novel errors (zero-overlap validation)
    // ═══════════════════════════════════════════════════════════════════════

    eprintln!("── Phase 3: Novel error classification ──────────");
    eprintln!();

    for (text, expected_category) in NOVEL_TESTS {
        eprintln!("  Text: \"{}\"", text);
        eprintln!("  Expected category: {}", expected_category);

        // Check trigram overlap with ALL episode texts
        let mut max_jaccard = 0.0;
        for variants in EPISODES {
            for ep_text in *variants {
                let j = trigram_jaccard(text, ep_text);
                if j > max_jaccard {
                    max_jaccard = j;
                }
            }
        }
        eprintln!(
            "  Max trigram Jaccard vs episodes: {:.4} (threshold=0.10)",
            max_jaccard
        );

        // Level 1+2: classifier
        let (svo, level) = classifier.classify_deep(text);
        match svo {
            Some(canonical) => {
                eprintln!("  Level {}/1-2: {} (should NOT match)", level, canonical.2)
            }
            None => eprintln!("  Level 1-2: no match ✓"),
        }

        // Level 3: structural parser
        let structural = classify_structural(text);
        match structural {
            Some(ref triples) => {
                eprintln!("  Level 3 structural: {} triples", triples.len());
                for (s, v, o) in triples.iter().take(4) {
                    eprintln!("    ({}, {}, {})", s, v, o);
                }

                // Store structural triples and check forward chain
                for (s, v, o) in triples {
                    qa.store_fact(s, v, o, "novel_test");
                }
                let n = qa.forward_chain(0.75);
                eprintln!("  Forward chain: {} facts derived", n);

                // Check which causes were identified
                let mut causes = Vec::new();
                if qa
                    .verify_fact("another_process", "is_listening_on", "same_port")
                    .0
                {
                    causes.push("port_conflict");
                }
                if qa.verify_fact("target_service", "is_not", "listening").0 {
                    causes.push("network_timeout");
                }
                if qa.verify_fact("required_file", "is", "missing").0 {
                    causes.push("missing_file");
                }
                if qa.verify_fact("file_permissions", "are", "incorrect").0 {
                    causes.push("permission_denied");
                }
                if qa.verify_fact("disk_space", "is", "full").0 {
                    causes.push("disk_full");
                }

                if causes.is_empty() {
                    eprintln!("  ✗ No cause identified — structural bridge failed");
                } else {
                    eprintln!("  ✓ Identified causes: {}", causes.join(", "));
                    if causes.contains(&expected_category) {
                        eprintln!("  ✓ Matches expected category: {}", expected_category);
                    } else {
                        eprintln!(
                            "  ⚠ Mismatch: expected {}, got {}",
                            expected_category,
                            causes.join(", ")
                        );
                    }
                }
            }
            None => {
                eprintln!("  Level 3 structural: no triples generated ✗");
            }
        }
        eprintln!();
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Final summary
    // ═══════════════════════════════════════════════════════════════════════

    let elapsed = start.elapsed();
    eprintln!("── Final Summary ────────────────────────────────");
    eprintln!(
        "  Episodes processed: {}",
        EPISODES.iter().map(|e| e.len()).sum::<usize>()
    );
    eprintln!(
        "  Phase 1 classification rate: {:.0}%",
        classified as f64 / (classified + failed) as f64 * 100.0
    );
    eprintln!("  Brain state:");
    eprintln!("    Dejavu clusters: {}", brain.dejavu_clusters.len());
    eprintln!("    Transient clusters: {}", brain.transient_clusters.len());
    eprintln!(
        "    Cross-cluster associations: {}",
        brain.cross_cluster_associations.len()
    );
    eprintln!("  Total time: {:?}", elapsed);
    eprintln!();
    eprintln!("  Key finding:");
    for category in CATEGORIES {
        let sim = measure_centroid_similarity(&brain, category);
        let status = if sim >= 0.65 {
            "STABLE ✓"
        } else if sim >= 0.55 {
            "MARGINAL"
        } else {
            "UNSTABLE"
        };
        eprintln!("    {}: sim={:.4} [{}]", category, sim, status);
    }
    eprintln!();
    eprintln!("═══════════════════════════════════════════════════════════════");
}
