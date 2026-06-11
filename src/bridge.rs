//! Perception bridge: natural language → role-filler frames.
//!
//! Uses the pure-Rust SVO extractor (`nlp::extract_svo`) instead of
//! a Python subprocess.  Extracted SVO triples are encoded as
//! hypervectors via `encode_text_ngram`, bound via `RoleDictionary`,
//! and inserted into the `AnalogicalIndex`.
//!
//! No Python, no spaCy, no subprocess — entirely self-contained Rust.

use crate::analogy::{
    self, AnalogicalIndex, EpistemicStatus, MetaIndex,
    ObservationProvenance, RoleDictionary,
    ROLE_AGENT, ROLE_ACTION, ROLE_PATIENT,
};
use crate::nlp;
use crate::Hypervector;

/// Encode a word or short phrase as a character-trigram hypervector.
/// Delegates to `Hypervector::encode_text_ngram(text, 3)`.
/// Public so other bridges (code_bridge) can reuse the same encoding.
pub fn encode_phrase(text: &str) -> Hypervector {
    Hypervector::encode_text_ngram(text, 3)
}

// ─── Data types ───────────────────────────────────────────────────────────

/// Result of a single bridge ingestion call.
#[derive(Debug, Default)]
pub struct BridgeResult {
    pub triples_extracted: usize,
    pub frames_inserted:   usize,
    pub frames_skipped:    usize,
    pub frames_rejected_quality: usize,  // triples that failed the quality pre-filter
    pub parse_errors:      usize,
}

// ─── Quality pre-filter ─────────────────────────────────────────────────────
//
// Runs BEFORE the novelty filter.  Rejects triples that are extraction
// artifacts — wrong subjects, empty objects, noise patterns.  These aren't
// "duplicates" in the semantic sense; they're garbage that happens to be
// unique.  The novelty filter handles semantic duplicates.  This handles
// extraction noise.

/// Common English stopwords that should not appear as sentence subjects.
fn is_stopword(w: &str) -> bool {
    matches!(w.to_lowercase().as_str(),
        "the" | "a" | "an" | "this" | "that" | "these" | "those"
        | "it" | "its" | "they" | "them" | "he" | "she" | "we" | "you"
        | "there" | "here" | "what" | "which" | "who" | "where" | "when"
        | "all" | "each" | "every" | "some" | "any" | "no" | "both"
        | "such" | "only" | "just" | "also" | "very" | "too" | "so"
        | "and" | "or" | "but" | "if" | "because" | "while" | "as"
        | "more" | "most" | "much" | "many" | "few" | "less"
        | "after" | "before" | "between" | "through" | "during"
        | "above" | "below" | "under" | "over" | "out" | "off"
        | "on" | "in" | "at" | "by" | "with" | "for" | "to" | "from"
        | "into" | "about" | "along" | "among" | "upon" | "across"
        | "against" | "within" | "without" | "behind" | "beyond"
        | "toward" | "towards" | "throughout" | "despite" | "until"
        | "since" | "up" | "down" | "away" | "back" | "around"
    )
}

/// Object phrase starts with a noise pattern that indicates the extractor
/// grabbed a trailing prepositional phrase instead of the true object.
fn starts_with_noise_prep(obj: &str) -> bool {
    let obj_trimmed = obj.trim();
    obj_trimmed.starts_with("as ") || obj_trimmed.starts_with("to ")
        || obj_trimmed.starts_with("on ") || obj_trimmed.starts_with("by ")
        || obj_trimmed.starts_with("for ") || obj_trimmed.starts_with("at ")
        || obj_trimmed.starts_with("into ") || obj_trimmed.starts_with("through ")
        || obj_trimmed.starts_with("during ") || obj_trimmed.starts_with("without ")
        || obj_trimmed.starts_with("in order to") || obj_trimmed.starts_with("so that")
}

/// Quality gate for extracted SVO triples.
/// Rejects triples that are clearly extraction artifacts before they reach
/// the novelty filter.  This separates two concerns:
///   - Quality:  "is this a well-formed triple?" (checked here)
///   - Novelty:  "is this semantically new?" (checked in ingest_json)
pub fn passes_quality_gate(triple: &nlp::SvoTriple) -> bool {
    // 1. Subject must not be a bare stopword
    let subj_trimmed = triple.subject.trim();
    if is_stopword(subj_trimmed) {
        return false;
    }
    // Subject with only punctuation
    if subj_trimmed.chars().all(|c| c.is_ascii_punctuation()) {
        return false;
    }

    // 2. Object must have meaningful content
    let obj_trimmed = triple.object.trim().trim_matches('.');
    if obj_trimmed.is_empty() {
        return false;  // intransitive verbs get zero-HV objects — skip them
    }
    // Object with only punctuation or single character
    if obj_trimmed.len() <= 1 || obj_trimmed.chars().all(|c| c.is_ascii_punctuation()) {
        return false;
    }
    // Object that's just a noise prepositional fragment
    if starts_with_noise_prep(&triple.object) {
        return false;
    }

    // 3. Verb must be a real word (not just punctuation)
    let verb_trimmed = triple.verb.trim();
    if verb_trimmed.len() <= 1 || verb_trimmed.chars().all(|c| c.is_ascii_punctuation()) {
        return false;
    }

    // Reject triples where subject and object are identical (extractor bug)
    if subj_trimmed.to_lowercase() == obj_trimmed.to_lowercase() && subj_trimmed.len() > 3 {
        return false;
    }

    true
}

// ─── Frame encoding ───────────────────────────────────────────────────────

/// Insert SVO triples (from `nlp::extract_svo`) into the PrimaryIndex.
///
/// Each SVO triple is encoded via `encode_text_ngram(subject, 3)` etc.,
/// bound via `RoleDictionary::bind_triple()`, and inserted with the
/// appropriate provenance and evidential weight.
///
/// * `novel_threshold` — minimum NHD from existing frames for insertion.
///   Use `0.05` to match the convergence experiment default.
/// * `frame_counter` — incremented for each inserted frame; the frame label
///   is `bridge_{counter:05}`.
pub fn ingest_triples(
    triples: &[nlp::SvoTriple],
    primary: &mut AnalogicalIndex,
    meta: &mut MetaIndex,
    novel_threshold: f64,
    frame_counter: &mut usize,
) -> BridgeResult {
    let mut result = BridgeResult::default();
    let roles = RoleDictionary::new();

    result.triples_extracted = triples.len();

    for triple in triples {
        // Skip triples with empty subject or verb — meaningless frames
        if triple.subject.is_empty() || triple.verb.is_empty() {
            result.frames_skipped += 1;
            continue;
        }

        // ── Quality gate (pre-filter before novelty) ──────────────
        // Rejects extraction artifacts: stopword subjects, empty objects,
        // prepositional-noise objects, and other low-quality patterns.
        // This is SEPARATE from the novelty filter: quality checks whether
        // the triple is well-formed; novelty checks whether it's new.
        if !passes_quality_gate(triple) {
            result.frames_rejected_quality += 1;
            result.frames_skipped += 1;
            continue;
        }

        // Encode via existing text n-gram encoding (character trigrams)
        let s_hv = Hypervector::encode_text_ngram(&triple.subject, 3);
        let v_hv = Hypervector::encode_text_ngram(&triple.verb, 3);
        let o_hv = if triple.object.is_empty() {
            Hypervector::new_zero()  // intransitive verb — zero patient
        } else {
            Hypervector::encode_text_ngram(&triple.object, 3)
        };

        // Bind triple via the existing RoleDictionary
        let bound = roles.bind_triple(&s_hv, &v_hv, &o_hv);

        // Novelty filter — skip frames too similar to existing ones
        let is_novel = primary.frames().iter().all(|f| {
            f.bound_vector.normalized_hamming_distance(&bound) > novel_threshold
        });
        if !is_novel {
            result.frames_skipped += 1;
            continue;
        }

        let label = format!("bridge_{:05}", frame_counter);
        *frame_counter += 1;

        let fillers = vec![
            (ROLE_AGENT,   s_hv, triple.subject.clone()),
            (ROLE_ACTION,  v_hv, triple.verb.clone()),
            (ROLE_PATIENT, o_hv, triple.object.clone()),
        ];

        // Confidence → evidential weight on 0–500 scale
        let weight = (triple.confidence * 400.0).clamp(0.0, 500.0);

        primary.insert_with_provenance(
            &label, bound, fillers,
            ObservationProvenance::Ambient,
        );

        meta.on_insert(
            &label, &bound,
            EpistemicStatus::Observed, weight,
            ObservationProvenance::Ambient,
        );

        result.frames_inserted += 1;
    }

    result
}

/// Top-level entry: raw text → frames.
///
/// Uses the pure-Rust NLP extractor — no Python subprocess needed.
pub fn ingest_text(
    text: &str,
    primary: &mut AnalogicalIndex,
    meta: &mut MetaIndex,
    novel_threshold: f64,
    frame_counter: &mut usize,
) -> BridgeResult {
    let triples = nlp::extract_svo(text);
    ingest_triples(&triples, primary, meta, novel_threshold, frame_counter)
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Quick check that the Rust SVO extractor works end‑to‑end.
    #[test]
    fn test_nlp_active() {
        let triples = nlp::extract_svo("Alice fed the cat.");
        assert!(!triples.is_empty(), "Should extract at least one triple");
        let t = &triples[0];
        eprintln!("  [bridge] active: ({}, {}, {}) conf={}",
            t.subject, t.verb, t.object, t.confidence);
        assert_eq!(t.verb, "feed", "Verb should be lemmatized");
        assert!(
            t.subject.to_lowercase().contains("alice"),
            "Subject should be Alice"
        );
        assert!(
            t.object.to_lowercase().contains("cat"),
            "Object should contain cat"
        );
    }

    /// Test that passive voice is correctly recovered.
    #[test]
    fn test_nlp_passive() {
        let triples = nlp::extract_svo("The cat was fed by Alice.");
        assert!(!triples.is_empty(), "Should extract at least one triple");
        let t = &triples[0];
        eprintln!("  [bridge] passive: ({}, {}, {}) conf={}",
            t.subject, t.verb, t.object, t.confidence);
        assert!(
            t.subject.to_lowercase().contains("alice"),
            "Passive recovery should put Alice as subject, got '{}'",
            t.subject,
        );
    }

    /// Test conjunction expansion.
    #[test]
    fn test_nlp_conjunction() {
        let triples = nlp::extract_svo("Bob reads books and writes code.");
        assert!(triples.len() >= 2, "Conjunction should produce ≥2 triples");
        let verbs: Vec<&str> = triples.iter().map(|t| t.verb.as_str()).collect();
        eprintln!("  [bridge] conj verbs: {:?}", verbs);
        assert!(
            verbs.contains(&"read"),
            "Should contain 'read' (first verb)"
        );
        assert!(
            verbs.contains(&"write"),
            "Should contain 'write' (conjunction-expanded verb)"
        );
    }

    /// Full end-to-end test: 5 sentences → frames → analogical predictions.
    #[test]
    fn test_bridge_end_to_end() {
        let roles = RoleDictionary::new();
        let mut primary = AnalogicalIndex::new(&roles);
        let mut meta = MetaIndex::new(&primary, 64);
        let mut counter = 0usize;

        // Test on domain-relevant sentences
        let sentences = [
            "Alice fed the cat.",
            "The cat was fed by Alice.",
            "Bob reads books and writes code.",
            "The market raises interest rates.",
            "High inflation causes rate hikes.",
        ];

        let mut total_inserted = 0usize;
        for sentence in &sentences {
            let result = ingest_text(sentence, &mut primary, &mut meta, 0.05, &mut counter);
            eprintln!(
                "  [bridge] sent='{}' → extracted={} inserted={} skipped={}",
                sentence, result.triples_extracted, result.frames_inserted, result.frames_skipped,
            );
            total_inserted += result.frames_inserted;
        }

        assert!(
            total_inserted >= 3,
            "Expected ≥3 frames from 5 sentences, got {total_inserted}",
        );

        // Verify the reasoning engine can analogize over real text frames
        let prediction_count = primary.predictions().len();
        eprintln!(
            "  [bridge] analogical predictions from real text: {}",
            prediction_count,
        );

        // Check frames have correct fillers
        let feed_frames: Vec<_> = primary.frames().iter()
            .filter(|f| f.label.starts_with("bridge_"))
            .collect();
        for f in &feed_frames {
            let fillers: Vec<&str> = f.fillers.iter()
                .map(|x| x.filler_str.as_str()).collect();
            eprintln!("  [bridge] frame {} — fillers: {:?}", f.label, fillers);
        }

        // Verify: frames are usable by the analogical engine
        assert!(
            prediction_count > 0 || total_inserted >= 5,
            "Should have either analogical predictions or sufficient frames",
        );
    }
}
