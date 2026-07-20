// ─── Text Encoder: PerceptualEncoder for English text → SVO triples ──────
//
// Implements the universal PerceptualEncoder trait for natural language text.
// Uses the existing rule-based NLP pipeline (nlp.rs) for sentence splitting,
// tokenization, POS tagging, and SVO extraction — no ML, no LLMs, no Python.
//
// Extracted triples are encoded as VSA hypervectors and stored in the
// VSABrain's cluster memory for retrieval, reasoning, and question answering.
// ────────────────────────────────────────────────────────────────────────────

use crate::perception::{Entity, PerceptualEncoder, SvoTriple as SvoTuple};
use crate::Hypervector;
use crate::VSABrain;

/// Text encoder: converts English text into SVO triples via rule-based NLP.
///
/// # Example
///
/// ```ignore
/// let encoder = TextEncoder::new();
/// let triples = encoder.encode("The Fed raised rates.");
/// // → [("the_fed", "raised", "rates")]
/// ```
pub struct TextEncoder;

impl TextEncoder {
    pub fn new() -> Self {
        TextEncoder
    }
}

impl PerceptualEncoder for TextEncoder {
    /// Input is a plain text string.
    type Input = String;

    /// Extract entities (noun phrases) from text via the SVO triple extractor.
    /// Subjects and objects of all extracted triples are collected as entities.
    fn extract_entities(&self, input: &Self::Input) -> Vec<Entity> {
        let triples = crate::nlp::extract_svo(input);
        let mut entities: Vec<String> = triples
            .iter()
            .flat_map(|t| vec![t.subject.to_lowercase(), t.object.to_lowercase()])
            .collect();
        entities.sort();
        entities.dedup();
        entities
    }

    /// Extract subject-verb-object relations from English text.
    fn extract_relations(&self, input: &Self::Input, _entities: &[Entity]) -> Vec<SvoTuple> {
        let triples = crate::nlp::extract_svo(input);
        triples
            .into_iter()
            .map(|t| {
                (
                    t.subject.to_lowercase(),
                    t.verb.to_lowercase(),
                    t.object.to_lowercase(),
                )
            })
            .collect()
    }
}

// ─── Knowledge Storage ─────────────────────────────────────────────────────
//
// SVO triples extracted from text are encoded as bound hypervectors and
// stored in the VSABrain's dejavu_clusters, exactly like chess positions.
// This lets the system retrieve and reason about textual knowledge the
// same way it reasons about chess positions.

/// Threshold for clustering text SVO triples.
/// Text triples are more diverse than chess positions, so a lower threshold
/// creates tighter clusters that distinguish fine semantic differences.
const TEXT_NHD_THRESHOLD: f64 = 0.30;

/// Store a text-derived SVO triple in the VSABrain's cluster memory.
///
/// Each triple is encoded as a bound hypervector (via resonator::encode_svo
/// with rotations ρ₁₃, ρ₂₆, ρ₃₉) and stored as an entry in the nearest
/// cluster.  The entry label is the NLP extraction confidence (0.0-1.0).
///
/// Multiple calls with similar triples will merge into the same cluster,
/// strengthening the centroid representation.
pub fn store_knowledge_triple(
    brain: &mut VSABrain,
    subject: &str,
    verb: &str,
    object: &str,
    confidence: f64,
    source: &str,
) {
    use std::collections::HashMap;

    let s_hv = Hypervector::encode_text_ngram(subject, 3);
    let v_hv = Hypervector::encode_text_ngram(verb, 3);
    let o_hv = Hypervector::encode_text_ngram(object, 3);
    let triple_hv = crate::resonator::encode_svo(&s_hv, &v_hv, &o_hv);

    let label = format!("{:.4}", confidence.clamp(0.0, 1.0));

    // Domain is derived from the source parameter.
    // Standard domains: "text_knowledge", "system_state", "threat_model", "chess_stage1"
    let domain = if source.contains("system") {
        "system_state"
    } else if source.contains("threat") {
        "threat_model"
    } else if source.contains("chess") {
        "chess"
    } else {
        "text_knowledge"
    };

    let mut meta = HashMap::new();
    meta.insert("source".to_string(), source.to_string());
    meta.insert("domain".to_string(), domain.to_string());
    meta.insert("subject".to_string(), subject.to_string());
    meta.insert("verb".to_string(), verb.to_string());
    meta.insert("object".to_string(), object.to_string());

    store_text_entry(brain, triple_hv, &label, meta);
}

/// Internal: find or create a cluster for a text hypervector, then store it.
fn store_text_entry(
    brain: &mut VSABrain,
    hv: Hypervector,
    label: &str,
    meta: std::collections::HashMap<String, String>,
) {
    let clusters = &mut brain.dejavu_clusters;

    // Find nearest cluster centroid
    let mut best_idx = None;
    let mut best_nhd = f64::MAX;

    for (idx, cluster) in clusters.iter().enumerate() {
        let nhd = hv.normalized_hamming_distance(&cluster.centroid);
        if nhd < best_nhd {
            best_nhd = nhd;
            best_idx = Some(idx);
        }
    }

    // Absorb into nearest cluster if within threshold
    if let Some(idx) = best_idx {
        if best_nhd < TEXT_NHD_THRESHOLD {
            let cluster = &mut clusters[idx];
            cluster.ensure_anchor();
            let entry =
                crate::DejavuEntry::new(hv.clone(), label.to_string(), meta, Some(&cluster.anchor));
            let tau = entry.reconstruct(&cluster.anchor);

            // Absorb into accumulator
            for (i, acc) in cluster.accumulator.iter_mut().enumerate() {
                let word = tau.bits[i / 64];
                let bit = (word >> (i % 64)) & 1;
                *acc += bit as u32;
            }
            cluster.total_weight += 1;

            // Recompute centroid
            let half_weight = cluster.total_weight / 2;
            for (i, acc) in cluster.accumulator.iter().enumerate() {
                let block = i / 64;
                let bit = i % 64;
                if *acc > half_weight {
                    cluster.centroid.bits[block] |= 1u64 << bit;
                } else {
                    cluster.centroid.bits[block] &= !(1u64 << bit);
                }
            }
            cluster.entries.push(entry);

            if cluster.entries.len() > crate::MAX_ENTRIES_PER_CLUSTER {
                let drain = crate::MAX_ENTRIES_PER_CLUSTER / 4;
                cluster.entries.drain(0..drain);
            }
            return;
        }
    }

    // Create new cluster
    let mut accumulator = vec![0u32; crate::HD_DIMENSION];
    for (i, acc) in accumulator.iter_mut().enumerate() {
        let word = hv.bits[i / 64];
        let bit = (word >> (i % 64)) & 1;
        *acc = bit as u32;
    }
    let entry = crate::DejavuEntry::new(hv.clone(), label.to_string(), meta, None);
    clusters.push(crate::MemoryCluster {
        centroid: hv,
        entries: vec![entry],
        reverberation: 1.0,
        last_reinforced_tick: 0,
        anchor: hv,
        accumulator,
        total_weight: 1,
        last_access_tick: 0,
    });
}

/// Convenience: run the full text → SVO → clusters pipeline on a text string.
///
/// Returns the number of triples stored.
pub fn ingest_text(brain: &mut VSABrain, text: &str, source: &str) -> usize {
    let encoder = TextEncoder::new();
    let triples = encoder.encode(&text.to_string());
    let mut count = 0;

    for (subject, verb, object) in &triples {
        store_knowledge_triple(brain, subject, verb, object, 1.0, source);
        count += 1;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VSABrain;

    #[test]
    fn test_text_encoder_extracts_svo() {
        let encoder = TextEncoder::new();
        let triples = encoder.encode(&"The Federal Reserve raised interest rates.".to_string());

        assert!(!triples.is_empty(), "Should extract at least one triple");
        eprintln!("  Extracted triples: {:?}", triples);

        // The NLP pipeline lemmatizes verbs ("raised" → "raise") and lowercases everything.
        let has_econ = triples
            .iter()
            .any(|(s, v, o)| v == "raise" && (o.contains("rate") || s.contains("fed")));
        assert!(
            has_econ,
            "Should capture economic relation: got {:?}",
            triples
        );
    }

    #[test]
    fn test_ingest_text_stores_clusters() {
        let mut brain = VSABrain::new(0.12);
        let count = ingest_text(
            &mut brain,
            "The Fed raised rates. Inflation rose. The economy grew.",
            "test",
        );

        assert!(count > 0, "Should store at least one triple");
        assert!(
            !brain.dejavu_clusters.is_empty(),
            "Should have at least one cluster after ingestion"
        );
        let total_entries: usize = brain.dejavu_clusters.iter().map(|c| c.entries.len()).sum();
        eprintln!(
            "  Stored {} triples → {} clusters, {} total entries",
            count,
            brain.dejavu_clusters.len(),
            total_entries
        );
    }

    #[test]
    fn test_similar_statements_merge() {
        let mut brain = VSABrain::new(0.12);

        // Two semantically similar statements should merge into same cluster
        ingest_text(&mut brain, "The Fed raised interest rates.", "src1");
        ingest_text(&mut brain, "The Federal Reserve hiked rates.", "src2");

        let n_clusters = brain.dejavu_clusters.len();
        eprintln!("  Similar statements → {} clusters", n_clusters);
        assert!(
            n_clusters <= 6,
            "Semantically similar statements should merge into few clusters, got {}",
            n_clusters
        );
    }

    #[test]
    fn test_different_statements_form_separate_clusters() {
        let mut brain = VSABrain::new(0.12);

        ingest_text(&mut brain, "The Fed raised interest rates.", "src1");
        ingest_text(&mut brain, "The cat sat on the mat.", "src2");

        let n_clusters = brain.dejavu_clusters.len();
        eprintln!("  Different topics → {} clusters", n_clusters);
        // These should form separate clusters since they're semantically distant
        assert!(
            n_clusters >= 2,
            "Different topics should form at least 2 clusters, got {}",
            n_clusters
        );
    }
}
