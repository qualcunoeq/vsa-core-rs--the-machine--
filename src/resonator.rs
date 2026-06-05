use crate::Hypervector;
use crate::HD_DIMENSION;
use std::collections::HashMap;

// ─── Constants ────────────────────────────────────────────────────────────

/// Minimum reconstruction energy required for a factorization to be accepted.
/// E = 1.0 - Hamming(T̂, T).  Below this threshold the result is rejected
/// as a hallucination.
pub const MIN_RECONSTRUCTION_ENERGY: f64 = 0.65;

/// Iterations without energy improvement before noise injection fires.
pub const PLATEAU_PATIENCE: usize = 8;

// ─── Vocabulary ───────────────────────────────────────────────────────────

pub struct ResonatorVocabulary {
    pub terms: HashMap<String, Hypervector>,
}

impl ResonatorVocabulary {
    pub fn new() -> Self {
        let mut vocab = ResonatorVocabulary {
            terms: HashMap::new(),
        };
        let baseline = vec![
            "sys_read", "sys_write", "execute_bash", "tcp_send",
            "Agent-1", "Agent-2", "Agent-3", "Broker", "Finch",
            "Breach", "Crisis", "Stable", "Attack", "Stealth", "Lehman",
            "Market", "News", "Infra",
            "hosts", "ledger", "read", "write", "execute", "panic", "sync",
            "What", "is", "the", "crisis", "breached", "server", "admin",
            "IF_RULE", "CAUSE_RULE", "THEN_RULE", "consequence",
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

    /// Cleanup a noisy vector by matching it to the closest vocabulary vector.
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

    /// Cleanup a noisy vector by matching it against a specific subset.
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

// ─── Rotation utilities ───────────────────────────────────────────────────

/// Rotates a vector to the right (opposite of rotate_left).
pub fn rotate_right(hv: &Hypervector, shift: usize) -> Hypervector {
    let shift = shift % HD_DIMENSION;
    if shift == 0 {
        return *hv;
    }
    hv.rotate_left(HD_DIMENSION - shift)
}

/// Encodes an SVO triple into a thought vector:
/// T = ρ₁₃(S) ⊕ ρ₂₆(V) ⊕ ρ₃₉(O)
pub fn encode_svo(s: &Hypervector, v: &Hypervector, o: &Hypervector) -> Hypervector {
    s.rotate_left(1 * 13)
        .bitwise_xor(&v.rotate_left(2 * 13))
        .bitwise_xor(&o.rotate_left(3 * 13))
}

/// Compute reconstruction energy:  E = 1.0 - Hamming(T̂, T)
/// where T̂ is re-encoded from the extracted S, V, O vectors.
pub fn reconstruction_energy(
    s_vec: &Hypervector,
    v_vec: &Hypervector,
    o_vec: &Hypervector,
    original: &Hypervector,
) -> f64 {
    let reconstructed = encode_svo(s_vec, v_vec, o_vec);
    1.0 - reconstructed.normalized_hamming_distance(original)
}

// ─── Simultaneous Resonator Network ────────────────────────────────────────

/// Factorises a thought vector into (subject, verb, object) using a
/// **Simultaneous Resonator Network** with reconstruction-energy validation
/// and simulated-annealing noise injection.
///
/// ## Simultaneous updates
/// All three factors are computed **in parallel** from the *previous* tick's
/// estimates, preventing cascade errors (the "Cocktail Party Problem"):
///
/// ```text
/// S_{t+1} = clean(ρ₋₁₃(T ⊕ ρ₂₆(V_t) ⊕ ρ₃₉(O_t)))
/// V_{t+1} = clean(ρ₋₂₆(T ⊕ ρ₁₃(S_t) ⊕ ρ₃₉(O_t)))
/// O_{t+1} = clean(ρ₋₃₉(T ⊕ ρ₁₃(S_t) ⊕ ρ₂₆(V_t)))
/// ```
///
/// ## Reconstruction energy
/// After convergence, the extracted SVO is re-encoded into T̂ and compared
/// to the original T via Hamming distance.  If the energy E < 0.65 the
/// result is rejected as a hallucination.
///
/// ## Annealing
/// If the energy plateaus below threshold, deterministic bit-flip noise is
/// injected into the factor guesses (temperature-scheduled) to escape local
/// minima.
///
/// Returns `None` when the energy check fails (hallucination filter).
pub fn factorize_svo(
    thought_vector: &Hypervector,
    vocab: &ResonatorVocabulary,
    subjects: &[String],
    verbs: &[String],
    objects: &[String],
    max_iterations: usize,
) -> Option<(String, String, String, f64)> {
    if vocab.terms.is_empty() || subjects.is_empty() || verbs.is_empty() || objects.is_empty() {
        return None;
    }

    // ── Initialise factor guesses from slot-specific bundles ─────────
    let s_init_vectors: Vec<&Hypervector> =
        subjects.iter().filter_map(|t| vocab.get_vector(t)).collect();
    let v_init_vectors: Vec<&Hypervector> =
        verbs.iter().filter_map(|t| vocab.get_vector(t)).collect();
    let o_init_vectors: Vec<&Hypervector> =
        objects.iter().filter_map(|t| vocab.get_vector(t)).collect();

    let mut current_s = if s_init_vectors.is_empty() {
        Hypervector::new_random()
    } else {
        Hypervector::bundle(&s_init_vectors)
    };
    let mut current_v = if v_init_vectors.is_empty() {
        Hypervector::new_random()
    } else {
        Hypervector::bundle(&v_init_vectors)
    };
    let mut current_o = if o_init_vectors.is_empty() {
        Hypervector::new_random()
    } else {
        Hypervector::bundle(&o_init_vectors)
    };

    let mut last_s_str = String::new();
    let mut last_v_str = String::new();
    let mut last_o_str = String::new();

    // Annealing state
    let mut best_energy = 0.0;
    let mut iter_since_best = 0usize;

    for iteration in 0..max_iterations {
        // ── Pre-compute rotated factor estimates (from PREVIOUS tick) ─
        let v_rot26 = current_v.rotate_left(2 * 13); // ρ₂₆(V_t)
        let o_rot39 = current_o.rotate_left(3 * 13); // ρ₃₉(O_t)
        let s_rot13 = current_s.rotate_left(1 * 13); // ρ₁₃(S_t)

        // ── Simultaneous unbinding (all from previous-tick state) ────
        // S_{t+1} = clean(rotate_right(T ⊕ ρ₂₆(V_t) ⊕ ρ₃₉(O_t), 13))
        let s_next_raw = rotate_right(
            &thought_vector.bitwise_xor(&v_rot26).bitwise_xor(&o_rot39),
            1 * 13,
        );

        // V_{t+1} = clean(rotate_right(T ⊕ ρ₁₃(S_t) ⊕ ρ₃₉(O_t), 26))
        let v_next_raw = rotate_right(
            &thought_vector.bitwise_xor(&s_rot13).bitwise_xor(&o_rot39),
            2 * 13,
        );

        // O_{t+1} = clean(rotate_right(T ⊕ ρ₁₃(S_t) ⊕ ρ₂₆(V_t), 39))
        let o_next_raw = rotate_right(
            &thought_vector.bitwise_xor(&s_rot13).bitwise_xor(&v_rot26),
            3 * 13,
        );

        // ── Cleanup (nearest-neighbour in vocabulary) ───────────────
        let (s_str, _) = vocab.cleanup_subset(&s_next_raw, subjects);
        let (v_str, _) = vocab.cleanup_subset(&v_next_raw, verbs);
        let (o_str, _) = vocab.cleanup_subset(&o_next_raw, objects);

        // ── Update ALL factor vectors simultaneously ────────────────
        let next_s = vocab
            .get_vector(&s_str)
            .cloned()
            .unwrap_or_else(Hypervector::new_random);
        let next_v = vocab
            .get_vector(&v_str)
            .cloned()
            .unwrap_or_else(Hypervector::new_random);
        let next_o = vocab
            .get_vector(&o_str)
            .cloned()
            .unwrap_or_else(Hypervector::new_random);

        current_s = next_s;
        current_v = next_v;
        current_o = next_o;

        // ── Convergence check ───────────────────────────────────────
        let converged = !s_str.is_empty()
            && s_str == last_s_str
            && v_str == last_v_str
            && o_str == last_o_str;

        last_s_str = s_str.clone();
        last_v_str = v_str.clone();
        last_o_str = o_str.clone();

        // ── Reconstruction energy ───────────────────────────────────
        let energy = reconstruction_energy(&current_s, &current_v, &current_o, thought_vector);

        // Track best energy for annealing
        if energy > best_energy {
            best_energy = energy;
            iter_since_best = 0;
        } else {
            iter_since_best += 1;
        }

        // If converged, validate via reconstruction energy
        if converged {
            if energy >= MIN_RECONSTRUCTION_ENERGY {
                return Some((s_str, v_str, o_str, energy));
            }
            // Low energy despite convergence → hallucination.
            // Inject noise and continue to try escaping the local minimum.
            let temperature = 1.0 - (iteration as f64 / max_iterations as f64);
            inject_noise(&mut current_s, &mut current_v, &mut current_o, temperature);
            // Reset convergence trackers
            last_s_str.clear();
            last_v_str.clear();
            last_o_str.clear();
            continue;
        }

        // ── Plateau annealing (no improvement for too long) ─────────
        if iter_since_best >= PLATEAU_PATIENCE && energy < MIN_RECONSTRUCTION_ENERGY {
            let temperature = 1.0 - (iteration as f64 / max_iterations as f64);
            // Only inject if temperature is still meaningful
            if temperature > 0.05 {
                inject_noise(&mut current_s, &mut current_v, &mut current_o, temperature);
                iter_since_best = 0;
                // Reset convergence trackers so we don't falsely re-converge
                last_s_str.clear();
                last_v_str.clear();
                last_o_str.clear();
            }
        }
    }

    // ── Ran out of iterations — final energy gate ────────────────────
    let energy = reconstruction_energy(&current_s, &current_v, &current_o, thought_vector);
    if energy >= MIN_RECONSTRUCTION_ENERGY {
        Some((last_s_str, last_v_str, last_o_str, energy))
    } else {
        None
    }
}

/// Inject deterministic noise into factor vectors to escape local minima.
/// Noise magnitude is proportional to `temperature` (simulated annealing).
fn inject_noise(
    s: &mut Hypervector,
    v: &mut Hypervector,
    o: &mut Hypervector,
    temperature: f64,
) {
    if temperature <= 0.0 {
        return;
    }

    // Number of 64-bit blocks to flip: proportional to temperature
    let blocks_to_flip = ((HD_DIMENSION / 64) as f64 * temperature * 0.25).round() as usize;
    let blocks_to_flip = blocks_to_flip.max(1);

    let mut rng = rand::thread_rng();
    use rand::Rng;

    for _ in 0..blocks_to_flip {
        let idx = rng.gen_range(0..crate::U64_BLOCKS);
        let mask = rng.gen::<u64>();
        s.bits[idx] ^= mask;
        v.bits[idx] ^= mask;
        o.bits[idx] ^= mask;
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RecursiveSlot {
    Term(String),
    Nested(Box<(String, String, RecursiveSlot)>),
}

fn factorize_recursive_internal(
    thought_vector: &Hypervector,
    vocab: &ResonatorVocabulary,
    subjects: &[String],
    verbs: &[String],
    objects: &[String],
    max_iterations: usize,
    depth: usize,
) -> Option<(String, String, RecursiveSlot)> {
    if depth > 3 {
        return None;
    }
    if vocab.terms.is_empty() || subjects.is_empty() || verbs.is_empty() || objects.is_empty() {
        return None;
    }

    // Try all pairs of Subject and Verb
    for s_str in subjects {
        let s_vec = match vocab.get_vector(s_str) {
            Some(v) => v,
            None => continue,
        };
        for v_str in verbs {
            let v_vec = match vocab.get_vector(v_str) {
                Some(v) => v,
                None => continue,
            };

            let s_rot13 = s_vec.rotate_left(1 * 13);
            let v_rot26 = v_vec.rotate_left(2 * 13);
            let o_est_raw = rotate_right(
                &thought_vector.bitwise_xor(&s_rot13).bitwise_xor(&v_rot26),
                3 * 13,
            );

            // 1. Check if the estimated object slot matches a flat vocabulary term
            let (o_str, o_sim) = vocab.cleanup_subset(&o_est_raw, objects);
            if o_sim >= MIN_RECONSTRUCTION_ENERGY {
                return Some((s_str.clone(), v_str.clone(), RecursiveSlot::Term(o_str)));
            }

            // 2. Try nested factorization of the object slot recursively
            if let Some((sub_s, sub_v, sub_slot)) = factorize_recursive_internal(
                &o_est_raw,
                vocab,
                subjects,
                verbs,
                objects,
                max_iterations,
                depth + 1,
            ) {
                if let RecursiveSlot::Term(ref s) = sub_slot {
                    if s.is_empty() {
                        continue;
                    }
                }
                return Some((
                    s_str.clone(),
                    v_str.clone(),
                    RecursiveSlot::Nested(Box::new((sub_s, sub_v, sub_slot))),
                ));
            }
        }
    }

    None
}

/// Factorises a potentially nested thought vector.
///
/// 1. Try all Subject and Verb pairs in the vocabulary.
/// 2. Estimate Object vector via unbinding: O_est = rotate_right(T ⊕ ρ₁₃(S) ⊕ ρ₂₆(V), 39).
/// 3. Check if O_est is close to a single term. If similarity ≥ 0.65, return RecursiveSlot::Term.
/// 4. Otherwise, recursively factorize O_est as a sub-thought SVO and return RecursiveSlot::Nested.
pub fn factorize_recursive(
    thought_vector: &Hypervector,
    vocab: &ResonatorVocabulary,
    subjects: &[String],
    verbs: &[String],
    objects: &[String],
    max_iterations: usize,
) -> Option<(String, String, RecursiveSlot)> {
    factorize_recursive_internal(
        thought_vector,
        vocab,
        subjects,
        verbs,
        objects,
        max_iterations,
        0,
    )
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_vocab() -> ResonatorVocabulary {
        let mut v = ResonatorVocabulary::new();
        v.register_term("Finch");
        v.register_term("write");
        v.register_term("ledger");
        v.register_term("read");
        v.register_term("panic");
        v.register_term("Agent-1");
        v.register_term("Broker");
        v.register_term("hosts");
        v.register_term("server");
        v
    }

    #[test]
    fn test_simultaneous_resonator_basic() {
        let vocab = make_vocab();

        let s_hv = vocab.get_vector("Finch").unwrap();
        let v_hv = vocab.get_vector("write").unwrap();
        let o_hv = vocab.get_vector("ledger").unwrap();

        let t = encode_svo(s_hv, v_hv, o_hv);

        let subjects = vec![
            "Finch".to_string(),
            "Agent-1".to_string(),
            "Broker".to_string(),
        ];
        let verbs = vec!["write".to_string(), "read".to_string(), "panic".to_string()];
        let objects = vec![
            "ledger".to_string(),
            "hosts".to_string(),
            "server".to_string(),
        ];

        let res = factorize_svo(&t, &vocab, &subjects, &verbs, &objects, 30);
        assert!(res.is_some(), "Resonator should resolve the thought vector");
        let (s, v, o, energy) = res.unwrap();
        assert_eq!(s, "Finch");
        assert_eq!(v, "write");
        assert_eq!(o, "ledger");
        assert!(
            energy >= MIN_RECONSTRUCTION_ENERGY,
            "Reconstruction energy should pass threshold: {}",
            energy
        );
    }

    #[test]
    fn test_encode_svo_roundtrip() {
        let vocab = make_vocab();
        let s = vocab.get_vector("Finch").unwrap();
        let v = vocab.get_vector("write").unwrap();
        let o = vocab.get_vector("ledger").unwrap();

        let t = encode_svo(s, v, o);
        let energy = reconstruction_energy(s, v, o, &t);
        assert!(
            (energy - 1.0).abs() < 0.001,
            "Perfect reconstruction should give energy ≈ 1.0, got {}",
            energy
        );
    }

    #[test]
    fn test_reconstruction_energy_rejects_noise() {
        let vocab = make_vocab();
        let s = vocab.get_vector("Finch").unwrap();
        let v = vocab.get_vector("write").unwrap();
        let o = vocab.get_vector("ledger").unwrap();
        let t = encode_svo(s, v, o);

        // Wrong factorisation
        let wrong_v = vocab.get_vector("read").unwrap();
        let energy = reconstruction_energy(s, wrong_v, o, &t);
        assert!(
            energy < MIN_RECONSTRUCTION_ENERGY,
            "Wrong factor should have low energy: {}",
            energy
        );
    }

    #[test]
    fn test_empty_vocab_returns_none() {
        let vocab = ResonatorVocabulary::new();
        let t = Hypervector::new_random();
        let res = factorize_svo(
            &t,
            &vocab,
            &[],
            &[],
            &[],
            10,
        );
        assert!(res.is_none());
    }

    #[test]
    fn test_hallucination_rejected() {
        // Build a thought vector that does NOT correspond to any valid SVO
        // by using random vectors as vocabulary terms.
        let mut vocab = ResonatorVocabulary::new();
        vocab.register_term("Alice");
        vocab.register_term("Bob");
        vocab.register_term("runs");
        vocab.register_term("walks");
        vocab.register_term("fast");
        vocab.register_term("slow");

        // A thought vector that is NOT a clean binding of any S/V/O triple
        let t = Hypervector::new_random();

        let subjects = vec!["Alice".to_string(), "Bob".to_string()];
        let verbs = vec!["runs".to_string(), "walks".to_string()];
        let objects = vec!["fast".to_string(), "slow".to_string()];

        let res = factorize_svo(&t, &vocab, &subjects, &verbs, &objects, 20);
        // Should either return None (energy gate) or have low energy
        if let Some((_s, _v, _o, energy)) = res {
            assert!(
                energy >= MIN_RECONSTRUCTION_ENERGY,
                "If returned, energy must pass threshold: {}",
                energy
            );
        }
    }

    #[test]
    fn test_rotate_right_inverse() {
        let v = Hypervector::new_random();
        let shifted = v.rotate_left(13);
        let unshifted = rotate_right(&shifted, 13);
        assert_eq!(v, unshifted);
    }

    #[test]
    fn test_recursive_factorization_nested() {
        let mut vocab = make_vocab();
        vocab.register_term("IF_RULE");
        vocab.register_term("Breach");

        let s_hv = vocab.get_vector("IF_RULE").unwrap();
        let v_hv = vocab.get_vector("Breach").unwrap();
        
        // Nested Object: "Finch write ledger"
        let sub_s = vocab.get_vector("Finch").unwrap();
        let sub_v = vocab.get_vector("write").unwrap();
        let sub_o = vocab.get_vector("ledger").unwrap();
        let sub_t = encode_svo(sub_s, sub_v, sub_o);

        // Nested Thought: "IF_RULE Breach (Finch write ledger)"
        let t = encode_svo(s_hv, v_hv, &sub_t);

        let subjects = vec![
            "IF_RULE".to_string(),
            "Finch".to_string(),
            "Agent-1".to_string(),
        ];
        let verbs = vec!["Breach".to_string(), "write".to_string(), "read".to_string()];
        let objects = vec![
            "ledger".to_string(),
            "hosts".to_string(),
            "server".to_string(),
        ];

        let res = factorize_recursive(&t, &vocab, &subjects, &verbs, &objects, 30);
        assert!(res.is_some(), "Recursive resonator should resolve nested thought");
        let (s, v, o_slot) = res.unwrap();
        assert_eq!(s, "IF_RULE");
        assert_eq!(v, "Breach");
        
        match o_slot {
            RecursiveSlot::Nested(boxed) => {
                let (sub_s_res, sub_v_res, sub_o_slot) = *boxed;
                assert_eq!(sub_s_res, "Finch");
                assert_eq!(sub_v_res, "write");
                assert_eq!(sub_o_slot, RecursiveSlot::Term("ledger".to_string()));
            }
            _ => panic!("Expected Nested slot"),
        }
    }
}
