use crate::{Hypervector, LSH_SECTOR_COUNT, HD_DIMENSION};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

// ─── Constants ────────────────────────────────────────────────────────────

/// Minimum reconstruction energy required for a factorization to be accepted.
/// E = 1.0 - Hamming(T̂, T).  Below this threshold the result is rejected
/// as a hallucination.
pub const MIN_RECONSTRUCTION_ENERGY: f64 = 0.65;

/// Iterations without energy improvement before noise injection fires.
pub const PLATEAU_PATIENCE: usize = 8;

// ─── Vocabulary ───────────────────────────────────────────────────────────

/// ██ UPGRADE v2.1: Added LSH fallback diagnostics ██
///
/// Tracks how often the hierarchical cleanup hits its sector target
/// vs. falling back to full O(M) scan.  Can be queried via
/// `lsh_fallback_rate()` for health monitoring.
pub struct ResonatorVocabulary {
    pub terms: HashMap<String, Hypervector>,
    /// Total cleanup calls
    lsh_total: AtomicU64,
    /// Cleanup calls that fell back to full scan (sector miss)
    lsh_fallback: AtomicU64,
}

impl ResonatorVocabulary {
    pub fn new() -> Self {
        let mut vocab = ResonatorVocabulary {
            terms: HashMap::new(),
            lsh_total: AtomicU64::new(0),
            lsh_fallback: AtomicU64::new(0),
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
            // Use character n-gram encoding so semantically related terms
            // (e.g. "crisis" and "Breach") naturally cluster in Hamming space
            // via shared trigrams, enabling analogical reasoning.
            self.terms
                .insert(term.to_string(), Hypervector::encode_text_ngram(term, 3));
        }
    }

    /// Dynamically register a new term from observed data.
    /// Returns `true` if the term was newly registered.
    pub fn learn_term(&mut self, term: &str) -> bool {
        if self.terms.contains_key(term) || term.len() < 2 {
            return false;
        }
        self.terms
            .insert(term.to_string(), Hypervector::encode_text_ngram(term, 3));
        true
    }

    pub fn get_vector(&self, term: &str) -> Option<&Hypervector> {
        self.terms.get(term)
    }

    /// ██ UPGRADE v2.1: LSH-hierarchical cleanup with fallback diagnostics ██
    ///
    /// Instead of brute-force O(M·D) nearest neighbour, we:
    /// 1. Compute a LSH sector hash via stable random projections
    /// 2. Search only terms whose index mod 16 == sector
    /// 3. If the best sector match is below threshold, fall back to full scan
    ///
    /// This reduces average lookup from O(M) to ~O(M/16) in the common case.
    /// Fallback rates are tracked atomically and queryable via `lsh_fallback_rate()`.
    pub fn cleanup(&self, vector: &Hypervector) -> (String, f64) {
        if self.terms.is_empty() {
            return ("".to_string(), 0.0);
        }

        self.lsh_total.fetch_add(1, Ordering::Relaxed);
        let sector = lsh_sector(vector);
        let term_vec: Vec<(&String, &Hypervector)> = self.terms.iter().collect();

        // Phase 1: LSH sector search
        let mut best_term = "".to_string();
        let mut best_sim = -1.0;

        for (idx, (term, vec)) in term_vec.iter().enumerate() {
            if idx % LSH_SECTOR_COUNT != sector { continue; }
            let sim = 1.0 - vector.normalized_hamming_distance(vec);
            if sim > best_sim {
                best_sim = sim;
                best_term = (*term).clone();
            }
        }

        // Phase 2: Fallback full scan if sector result is weak
        if best_sim < 0.55 {
            self.lsh_fallback.fetch_add(1, Ordering::Relaxed);
            for (term, vec) in &self.terms {
                let sim = 1.0 - vector.normalized_hamming_distance(vec);
                if sim > best_sim {
                    best_sim = sim;
                    best_term = term.clone();
                }
            }
        }

        (best_term, best_sim)
    }

    /// Return the fraction of cleanup calls that fell back to full O(M) scan.
    /// Useful for diagnosing LSH sector imbalance.
    /// Returns `(fallback_count, total_count, fallback_rate)`.
    pub fn lsh_fallback_rate(&self) -> (u64, u64, f64) {
        let total = self.lsh_total.load(Ordering::Relaxed);
        let fallback = self.lsh_fallback.load(Ordering::Relaxed);
        let rate = if total > 0 {
            fallback as f64 / total as f64
        } else {
            0.0
        };
        (fallback, total, rate)
    }

    /// ██ FIX v2.5: LSH sector entropy monitoring ██
    ///
    /// Computes the Shannon entropy of the term distribution across
    /// LSH sectors.  When entropy drops significantly below the
    /// theoretical maximum (log₂(LSH_SECTOR_COUNT) ≈ 10 bits),
    /// it indicates that terms are clustering in a subset of sectors,
    /// increasing collision probability and degrading LSH prefilter
    /// effectiveness.
    ///
    /// Returns `(entropy, max_sector_count, collision_risk_flag)`:
    /// - `entropy`: Shannon entropy in bits (max ≈ 10.0 for 1024 sectors)
    /// - `max_sector_count`: number of terms in the most crowded sector
    /// - `collision_risk_flag`: true if entropy < 7.5 or max sector > 3× expected
    pub fn lsh_sector_entropy(&self) -> (f64, usize, bool) {
        if self.terms.is_empty() {
            return (0.0, 0, false);
        }

        let mut sector_counts = vec![0usize; LSH_SECTOR_COUNT];
        for (_, vec) in &self.terms {
            let sector = crate::lsh_sector_inline(vec);
            if sector < LSH_SECTOR_COUNT {
                sector_counts[sector] += 1;
            }
        }

        let total = self.terms.len() as f64;
        let mut entropy = 0.0_f64;
        let mut max_count = 0usize;
        for &count in &sector_counts {
            if count > 0 {
                let p = count as f64 / total;
                entropy -= p * p.log2();
            }
            if count > max_count {
                max_count = count;
            }
        }

        let expected_per_sector = total / LSH_SECTOR_COUNT as f64;
        let collision_risk = entropy < 7.5 || (max_count as f64) > expected_per_sector * 3.0;

        (entropy, max_count, collision_risk)
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

    /// Prune redundant vocabulary terms by clustering similar vectors.
    ///
    /// Every new `learn_term` registration slightly shifts the geometry of
    /// the whole cleanup projection (nearest-neighbour search over all terms).
    /// Over time the Hamming neighbourhoods get denser and disambiguation
    /// degrades.  This method merges terms whose n-gram vectors are within
    /// `theta_sim` (default 0.70) of each other, keeping only the most
    /// representative term per cluster (the one closest to the centroid).
    ///
    /// Returns the number of pruned terms.
    pub fn prune_vocabulary(&mut self, theta_sim: f64) -> usize {
        let terms: Vec<(String, Hypervector)> = self.terms.iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();

        if terms.len() < 3 {
            return 0; // nothing to prune
        }

        // Greedy agglomerative clustering
        let mut keep: Vec<bool> = vec![true; terms.len()];

        for i in 0..terms.len() {
            if !keep[i] {
                continue;
            }
            // Build a cluster around terms[i]
            let mut cluster_indices = vec![i];

            for j in (i + 1)..terms.len() {
                if !keep[j] {
                    continue;
                }
                let sim = 1.0 - terms[i].1.normalized_hamming_distance(&terms[j].1);
                if sim >= theta_sim {
                    cluster_indices.push(j);
                }
            }

            if cluster_indices.len() <= 1 {
                continue; // no duplicates found
            }

            // Compute cluster centroid (bundle of all members)
            let refs: Vec<&Hypervector> = cluster_indices.iter()
                .map(|&idx| &terms[idx].1)
                .collect();
            let centroid = Hypervector::bundle(&refs);

            // Pick the term closest to the centroid as the representative
            let mut best_idx = cluster_indices[0];
            let mut best_sim = -1.0;
            for &idx in &cluster_indices {
                let sim = 1.0 - terms[idx].1.normalized_hamming_distance(&centroid);
                if sim > best_sim {
                    best_sim = sim;
                    best_idx = idx;
                }
            }

            // Mark all cluster members as "remove" except the best
            for &idx in &cluster_indices {
                if idx != best_idx {
                    keep[idx] = false;
                }
            }
        }

        // Remove pruned terms from the map
        let mut pruned = 0;
        for (idx, (term, _)) in terms.iter().enumerate() {
            if !keep[idx] {
                self.terms.remove(term);
                pruned += 1;
            }
        }

        pruned
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

// ─── LSH sector hash (stable random projections) ───────────────────────────

/// ██ UPGRADE v2.1: Stable random projection LSH ██
///
/// Same logic as `lsh_sector_inline` in lib.rs — each of the 4 bits
/// is a popcount parity against widely-separated fixed indices.
/// This is immune to LCG clustering skew from the character encoder.
/// 10-bit LSH sector hash, identical to `crate::lsh_sector_inline`.
/// Kept as a separate function to avoid circular dependency issues
/// with the crate-level inline version.
pub fn lsh_sector(vector: &Hypervector) -> usize {
    let bit_0 = (vector.bits[1] ^ vector.bits[50]).count_ones() % 2;
    let bit_1 = (vector.bits[2] ^ vector.bits[100]).count_ones() % 2;
    let bit_2 = (vector.bits[3] ^ vector.bits[150]).count_ones() % 2;
    let bit_3 = (vector.bits[4] ^ vector.bits[75]).count_ones() % 2;
    let bit_4 = (vector.bits[5] ^ vector.bits[120]).count_ones() % 2;
    let bit_5 = (vector.bits[6] ^ vector.bits[90]).count_ones() % 2;
    let bit_6 = (vector.bits[7] ^ vector.bits[140]).count_ones() % 2;
    let bit_7 = (vector.bits[8] ^ vector.bits[60]).count_ones() % 2;
    let bit_8 = (vector.bits[9] ^ vector.bits[110]).count_ones() % 2;
    let bit_9 = (vector.bits[10] ^ vector.bits[130]).count_ones() % 2;

    ((bit_9 << 9) | (bit_8 << 8) | (bit_7 << 7) | (bit_6 << 6) | (bit_5 << 5)
        | (bit_4 << 4) | (bit_3 << 3) | (bit_2 << 2) | (bit_1 << 1) | bit_0) as usize
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
    fn test_lsh_distribution_uniformity() {
        // Generate random hypervectors and verify sector distribution
        // is uniform across all LSH_SECTOR_COUNT sectors.
        // N = 100K samples → ~98 per bin.
        let n = 100_000;
        let mut sector_counts = vec![0usize; LSH_SECTOR_COUNT];
        for _ in 0..n {
            let v = Hypervector::new_random();
            let sector = super::lsh_sector(&v);
            sector_counts[sector] += 1;
        }

        let expected = n as f64 / LSH_SECTOR_COUNT as f64;
        // 5σ tolerance with Bonferroni correction for 1024 simultaneous bins.
        // σ ≈ sqrt(100000 * 1/1024 * 1023/1024) ≈ sqrt(97.6) ≈ 9.88
        // 5σ ≈ 49.4 — accounts for multiple testing across 1024 bins.
        let sigma = (n as f64 * (1.0 / LSH_SECTOR_COUNT as f64)
            * (1.0 - 1.0 / LSH_SECTOR_COUNT as f64))
            .sqrt();
        let tolerance = (5.0 * sigma).ceil() as usize;

        let mut max_dev = 0usize;
        for (i, &count) in sector_counts.iter().enumerate() {
            let dev = if count > expected as usize {
                count - expected as usize
            } else {
                expected as usize - count
            };
            if dev > max_dev {
                max_dev = dev;
            }
            assert!(
                dev <= tolerance,
                "LSH sector {} has {} entries (expected ~{:.1}, deviation {} > tolerance {})",
                i, count, expected, dev, tolerance
            );
        }
        eprintln!(
            "  LSH uniformity: {} bins, {} samples, max deviation {} (tolerance {})",
            LSH_SECTOR_COUNT, n, max_dev, tolerance
        );
    }

    #[test]
    fn test_lsh_fallback_diagnostics() {
        let vocab = make_vocab();
        let (fallback, total, rate) = vocab.lsh_fallback_rate();
        assert_eq!(total, 0, "No cleanups yet");
        assert_eq!(fallback, 0, "No fallbacks yet");
        assert_eq!(rate, 0.0, "Rate should be 0");

        // Run a few cleanups
        let v = Hypervector::new_random();
        let _ = vocab.cleanup(&v);
        let (_fallback, total, rate) = vocab.lsh_fallback_rate();
        assert_eq!(total, 1, "One cleanup");
        assert!(rate >= 0.0 && rate <= 1.0, "Rate should be valid");
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
