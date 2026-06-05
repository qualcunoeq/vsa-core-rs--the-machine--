use crate::Hypervector;
use crate::HD_DIMENSION;
use std::collections::HashMap;

pub struct ResonatorVocabulary {
    pub terms: HashMap<String, Hypervector>,
}

impl ResonatorVocabulary {
    pub fn new() -> Self {
        let mut vocab = ResonatorVocabulary {
            terms: HashMap::new(),
        };
        // Bootstrap with baseline dictionary of system commands and common grammar tokens
        let baseline = vec![
            "sys_read",
            "sys_write",
            "execute_bash",
            "tcp_send",
            "Agent-1",
            "Agent-2",
            "Agent-3",
            "Broker",
            "Finch",
            "Breach",
            "Crisis",
            "Stable",
            "Attack",
            "Stealth",
            "Lehman",
            "Market",
            "News",
            "Infra",
            "hosts",
            "ledger",
            "read",
            "write",
            "execute",
            "panic",
            "sync",
            "What",
            "is",
            "the",
            "crisis",
            "breached",
            "server",
            "admin",
        ];
        for term in baseline {
            vocab.register_term(term);
        }
        vocab
    }

    pub fn register_term(&mut self, term: &str) {
        if !self.terms.contains_key(term) {
            self.terms
                .insert(term.to_string(), Hypervector::new_random());
        }
    }

    pub fn get_vector(&self, term: &str) -> Option<&Hypervector> {
        self.terms.get(term)
    }

    /// Cleanup a noisy vector by matching it to the closest vocabulary vector
    pub fn cleanup(&self, vector: &Hypervector) -> (String, f64) {
        if self.terms.is_empty() {
            return ("".to_string(), 0.0);
        }
        let mut best_term = "".to_string();
        let mut best_sim = -1.0;
        for (term, vec) in &self.terms {
            let sim = 1.0 - vector.normalized_hamming_distance(vec);
            if sim > best_sim {
                best_sim = sim;
                best_term = term.clone();
            }
        }
        (best_term, best_sim)
    }

    /// Cleanup a noisy vector by matching it against a specific subset of candidate terms
    pub fn cleanup_subset(&self, vector: &Hypervector, subset: &[String]) -> (String, f64) {
        if subset.is_empty() {
            return ("".to_string(), 0.0);
        }
        let mut best_term = "".to_string();
        let mut best_sim = -1.0;
        for term in subset {
            if let Some(vec) = self.terms.get(term) {
                let sim = 1.0 - vector.normalized_hamming_distance(vec);
                if sim > best_sim {
                    best_sim = sim;
                    best_term = term.clone();
                }
            }
        }
        (best_term, best_sim)
    }
}

/// Rotates a vector to the right (opposite of rotate_left)
pub fn rotate_right(hv: &Hypervector, shift: usize) -> Hypervector {
    let shift = shift % HD_DIMENSION;
    if shift == 0 {
        return *hv;
    }
    hv.rotate_left(HD_DIMENSION - shift)
}

pub fn factorize_svo(
    thought_vector: &Hypervector,
    vocab: &ResonatorVocabulary,
    subjects: &[String],
    verbs: &[String],
    objects: &[String],
    max_iterations: usize,
) -> Option<(String, String, String)> {
    if vocab.terms.is_empty() || subjects.is_empty() || verbs.is_empty() || objects.is_empty() {
        return None;
    }

    // Initialize guesses from the slot-specific candidates
    let v_vectors: Vec<&Hypervector> = verbs.iter().filter_map(|t| vocab.get_vector(t)).collect();
    let o_vectors: Vec<&Hypervector> = objects.iter().filter_map(|t| vocab.get_vector(t)).collect();

    let mut current_v = Hypervector::bundle(&v_vectors);
    let mut current_o = Hypervector::bundle(&o_vectors);

    let mut last_s_str = "".to_string();
    let mut last_v_str = "".to_string();
    let mut last_o_str = "".to_string();

    for _ in 0..max_iterations {
        // 1. Estimate Subject (Slot 1, shift = 13)
        let v_rot = current_v.rotate_left(2 * 13);
        let o_rot = current_o.rotate_left(3 * 13);
        let s_raw = rotate_right(
            &thought_vector.bitwise_xor(&v_rot).bitwise_xor(&o_rot),
            1 * 13,
        );
        let (s_str, _) = vocab.cleanup_subset(&s_raw, subjects);
        let next_s = vocab
            .get_vector(&s_str)
            .cloned()
            .unwrap_or(Hypervector::new_random());

        // 2. Estimate Verb (Slot 2, shift = 26)
        let s_rot = next_s.rotate_left(1 * 13);
        let v_raw = rotate_right(
            &thought_vector.bitwise_xor(&s_rot).bitwise_xor(&o_rot),
            2 * 13,
        );
        let (v_str, _) = vocab.cleanup_subset(&v_raw, verbs);
        let next_v = vocab
            .get_vector(&v_str)
            .cloned()
            .unwrap_or(Hypervector::new_random());

        // 3. Estimate Object (Slot 3, shift = 39)
        let next_v_rot = next_v.rotate_left(2 * 13);
        let o_raw = rotate_right(
            &thought_vector.bitwise_xor(&s_rot).bitwise_xor(&next_v_rot),
            3 * 13,
        );
        let (o_str, _) = vocab.cleanup_subset(&o_raw, objects);
        let next_o = vocab
            .get_vector(&o_str)
            .cloned()
            .unwrap_or(Hypervector::new_random());

        // Convergence Check
        if s_str == last_s_str && v_str == last_v_str && o_str == last_o_str {
            return Some((s_str, v_str, o_str));
        }

        last_s_str = s_str;
        last_v_str = v_str;
        last_o_str = o_str;

        current_v = next_v;
        current_o = next_o;
    }

    Some((last_s_str, last_v_str, last_o_str))
}
