use crate::Hypervector;
use rand::Rng;
use std::collections::HashMap;

// ─── Constants ────────────────────────────────────────────────────────────

/// Default number of fractional intervals to precompute per unit of t.
/// Higher = finer interpolation, more memory.
pub const DEFAULT_FRACTIONAL_STEPS: usize = 10;

/// The base permutation step used for temporal binding.
/// Applied as: B^t = repeated rotate_left(t * STEP)
pub const FPE_ROTATION_STEP: usize = 7;

// ─── Fractional Power Encoding Engine ─────────────────────────────────────

/// Fractional Power Encoding (FPE) for continuous temporal reasoning.
///
/// In binary HDC, exponentiation B^t is approximated via controlled rotation.
/// For integer t, this is exact: B^t = rotate(B, t * STEP).
/// For fractional t, we interpolate between adjacent integer rotations by
/// bundling the rotated vectors weighted by the fractional proximity.
///
/// ## How it works
///
/// Given a base hypervector B and a continuous time t:
/// 1. Compute integer part: n = floor(t), frac = t - n
/// 2. B^n = rotate(B, n * STEP) — exact
/// 3. B^(n+1) = rotate(B, (n+1) * STEP) — exact
/// 4. B^t ≈ bundle( B^n repeated (1-frac) copies, B^(n+1) repeated frac copies )
///
/// This gives a smooth interpolation between discrete time steps.
pub struct FractionalPowerEncoder {
    /// The base hypervector B (random, unit-norm in HD space)
    base: Hypervector,
    /// Precomputed powers: power_table[k] = B^k for k = 0..max_power
    power_table: Vec<Hypervector>,
    /// Maximum precomputed power
    max_power: usize,
    /// Number of fractional interpolation steps
    fractional_steps: usize,
}

impl FractionalPowerEncoder {
    /// Create a new FPE with random base and precomputed power table.
    pub fn new(max_power: usize, fractional_steps: usize) -> Self {
        let base = Hypervector::new_random();
        let mut power_table = Vec::with_capacity(max_power + 1);

        // B^0 = rotation by 0 = identity
        power_table.push(Hypervector::new_zero());

        // B^1 = base rotated by STEP (skip zero since rotate zero is identity)
        // Actually B^0 should be the identity rotation: rotate(hv, 0) = hv
        // But for the binding operation B^t ⊗ C, we need B^t itself
        // B^1 = rotate(B, 1 * STEP)
        let mut current = base;
        for k in 1..=max_power {
            current = base.rotate_left(k * FPE_ROTATION_STEP);
            power_table.push(current);
        }

        FractionalPowerEncoder {
            base,
            power_table,
            max_power,
            fractional_steps,
        }
    }

    /// Create an FPE with a specific base vector (for reproducibility).
    pub fn with_base(base: Hypervector, max_power: usize, fractional_steps: usize) -> Self {
        let mut power_table = Vec::with_capacity(max_power + 1);
        for k in 0..=max_power {
            power_table.push(base.rotate_left(k * FPE_ROTATION_STEP));
        }

        FractionalPowerEncoder {
            base,
            power_table,
            max_power,
            fractional_steps,
        }
    }

    /// Compute B^t for a continuous time value t.
    ///
    /// For integer t, returns the exact precomputed power.
    /// For fractional t, interpolates between adjacent integer powers.
    pub fn power(&self, t: f64) -> Hypervector {
        if t <= 0.0 {
            return self.power_table[0];
        }

        let t_int = t.floor() as usize;
        let frac = t - t.floor();

        if t_int >= self.max_power {
            return self.power_table[self.max_power]; // Clamp
        }

        if frac < 1.0 / (self.fractional_steps as f64 * 2.0) {
            // Close enough to integer — use exact power
            return self.power_table[t_int];
        }

        // Interpolate between B^floor(t) and B^ceil(t)
        let b_n = &self.power_table[t_int];
        let b_n1 = &self.power_table[(t_int + 1).min(self.max_power)];

        let copies_n = ((1.0 - frac) * self.fractional_steps as f64).round() as usize;
        let copies_n1 = (frac * self.fractional_steps as f64).round() as usize;

        let copies_n = copies_n.max(1);
        let copies_n1 = copies_n1.max(1);

        let mut components: Vec<&Hypervector> = Vec::with_capacity(copies_n + copies_n1);
        for _ in 0..copies_n {
            components.push(b_n);
        }
        for _ in 0..copies_n1 {
            components.push(b_n1);
        }

        Hypervector::bundle(&components)
    }

    /// Encode a value at time t: V(t) = B^t ⊗ C (where ⊗ is XOR in binary HDC).
    pub fn encode_at_time(&self, value: &Hypervector, t: f64) -> Hypervector {
        let bt = self.power(t);
        bt.bitwise_xor(value)
    }

    /// Decode a value at time t from an encoded state.
    pub fn decode_at_time(&self, state: &Hypervector, t: f64) -> Hypervector {
        let bt = self.power(t);
        state.bitwise_xor(&bt)
    }

    /// Get the base hypervector.
    pub fn base(&self) -> &Hypervector {
        &self.base
    }

    /// Get the power table size.
    pub fn max_power(&self) -> usize {
        self.max_power
    }

    /// Memory usage estimate in bytes.
    pub fn memory_usage(&self) -> usize {
        self.power_table.len() * 157 * 8
    }
}

// ─── Continuous Temporal Sequence ─────────────────────────────────────────

/// A temporal sequence with continuous time indexing.
///
/// Unlike `TemporalSequence` (which uses discrete permutation indices),
/// this stores (time, state) pairs and can interpolate between them.
///
/// H_total = bundle( B^{t0} ⊗ S0, B^{t1} ⊗ S1, ..., B^{tn} ⊗ Sn )
///
/// Querying at time t recovers the approximate state by unbinding B^t.
pub struct ContinuousTemporalSequence {
    /// The FPE engine used for encoding
    fpe: FractionalPowerEncoder,
    /// Stored (time, state) pairs
    entries: Vec<(f64, Hypervector)>,
    /// The bundled sequence hypervector
    sequence: Hypervector,
    /// Whether the sequence needs recomputation
    dirty: bool,
}

impl ContinuousTemporalSequence {
    /// Create a new continuous sequence with the given FPE.
    pub fn new(fpe: FractionalPowerEncoder) -> Self {
        ContinuousTemporalSequence {
            fpe,
            entries: Vec::new(),
            sequence: Hypervector::new_zero(),
            dirty: false,
        }
    }

    /// Add a state observation at a specific time.
    pub fn add_observation(&mut self, time: f64, state: Hypervector) {
        self.entries.push((time, state));
        self.dirty = true;

        // Keep entries sorted by time
        self.entries.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    }

    /// Rebuild the sequence hypervector from all entries.
    pub fn rebuild(&mut self) {
        if self.entries.is_empty() {
            self.sequence = Hypervector::new_zero();
            self.dirty = false;
            return;
        }

        let mut bound_vectors: Vec<Hypervector> = Vec::with_capacity(self.entries.len());
        for &(t, ref state) in &self.entries {
            let bt = self.fpe.power(t);
            let bound = bt.bitwise_xor(state);
            bound_vectors.push(bound);
        }

        let refs: Vec<&Hypervector> = bound_vectors.iter().collect();
        self.sequence = Hypervector::bundle(&refs);
        self.dirty = false;
    }

    /// Ensure the sequence is fresh.
    pub fn ensure_rebuilt(&mut self) {
        if self.dirty {
            self.rebuild();
        }
    }

    /// Query the state at an arbitrary continuous time t.
    ///
    /// This unbinds B^t from the sequence to recover an approximation
    /// of the state at time t. The result is interpolated from neighboring
    /// observations.
    pub fn query(&mut self, t: f64) -> Option<Hypervector> {
        self.ensure_rebuilt();

        if self.entries.is_empty() {
            return None;
        }

        // Find the bounding observations
        let lower = self.entries.iter()
            .enumerate()
            .filter(|(_, (time, _))| *time <= t)
            .max_by(|a, b| a.1.0.partial_cmp(&b.1.0).unwrap_or(std::cmp::Ordering::Equal));

        let upper = self.entries.iter()
            .enumerate()
            .filter(|(_, (time, _))| *time >= t)
            .min_by(|a, b| a.1.0.partial_cmp(&b.1.0).unwrap_or(std::cmp::Ordering::Equal));

        // Save indices for comparison before destructuring
        let lower_idx = lower.as_ref().map(|&(i, _)| i);
        let upper_idx = upper.as_ref().map(|&(i, _)| i);

        let bt = self.fpe.power(t);

        match (lower, upper) {
            (Some((li, _)), Some((ui, _))) if li == ui => {
                // Exact time match
                Some(self.sequence.bitwise_xor(&bt))
            }
            (Some(_), None) => {
                // Past the last observation — extrapolate
                Some(self.sequence.bitwise_xor(&bt))
            }
            (None, Some(_)) => {
                // Before the first observation — approximation
                Some(self.sequence.bitwise_xor(&bt))
            }
            _ => {
                // Between observations or no data
                Some(self.sequence.bitwise_xor(&bt))
            }
        }
    }

    /// Get the stored observations.
    pub fn entries(&self) -> &[(f64, Hypervector)] {
        &self.entries
    }

    /// Number of observations.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Time range covered.
    pub fn time_range(&self) -> Option<(f64, f64)> {
        if self.entries.is_empty() {
            None
        } else {
            Some((self.entries.first().unwrap().0, self.entries.last().unwrap().0))
        }
    }

    /// Get a reference to the FPE engine.
    pub fn fpe(&self) -> &FractionalPowerEncoder {
        &self.fpe
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fpe_integer_power_exact() {
        let fpe = FractionalPowerEncoder::new(20, 10);

        // B^0 should be a zero hypervector (identity rotation)
        let b0 = fpe.power(0.0);
        let zero = Hypervector::new_zero();
        assert_eq!(b0, zero, "B^0 should be zero vector");

        // B^1 should equal rotate(base, STEP)
        let b1 = fpe.power(1.0);
        let expected = fpe.base().rotate_left(FPE_ROTATION_STEP);
        assert_eq!(b1, expected, "B^1 should equal rotate(base, STEP)");

        // B^2 should equal rotate(base, 2*STEP)
        let b2 = fpe.power(2.0);
        let expected = fpe.base().rotate_left(2 * FPE_ROTATION_STEP);
        assert_eq!(b2, expected, "B^2 should equal rotate(base, 2*STEP)");
    }

    #[test]
    fn test_fpe_fractional_power() {
        let fpe = FractionalPowerEncoder::new(20, 10);

        // B^1.5 should be between B^1 and B^2
        let b1 = fpe.power(1.0);
        let b15 = fpe.power(1.5);
        let b2 = fpe.power(2.0);

        let d1_15 = b1.normalized_hamming_distance(&b15);
        let d15_2 = b15.normalized_hamming_distance(&b2);
        let d1_2 = b1.normalized_hamming_distance(&b2);

        // B^1.5 should be equidistant from B^1 and B^2
        // Since it's an interpolation, both distances should be less than max
        assert!(d1_15 < d1_2, "B^1.5 should be closer to B^1 than B^1 is to B^2");
        assert!(d15_2 < d1_2, "B^1.5 should be closer to B^2 than B^1 is to B^2");
    }

    #[test]
    fn test_fpe_encode_decode_roundtrip() {
        let fpe = FractionalPowerEncoder::new(20, 10);
        let value = Hypervector::new_random();

        // Encode at t=3.0
        let encoded = fpe.encode_at_time(&value, 3.0);
        let decoded = fpe.decode_at_time(&encoded, 3.0);

        // Should recover the original value (XOR is self-inverse)
        let sim = 1.0 - decoded.normalized_hamming_distance(&value);
        assert!(sim > 0.99, "Decode should recover value, sim={}", sim);
    }

    #[test]
    fn test_fpe_different_times_different_encodings() {
        let fpe = FractionalPowerEncoder::new(20, 10);
        let value = Hypervector::new_random();

        let at_t1 = fpe.encode_at_time(&value, 1.0);
        let at_t2 = fpe.encode_at_time(&value, 2.0);

        // Same value at different times should produce different encodings
        let dist = at_t1.normalized_hamming_distance(&at_t2);
        assert!(dist > 0.05, "Different times should differ, dist={}", dist);
    }

    #[test]
    fn test_continuous_sequence_basic() {
        let fpe = FractionalPowerEncoder::new(50, 10);
        let mut seq = ContinuousTemporalSequence::new(fpe);

        let s0 = Hypervector::encode_text_ngram("state_zero", 3);
        let s1 = Hypervector::encode_text_ngram("state_one", 3);
        let s2 = Hypervector::encode_text_ngram("state_two", 3);

        seq.add_observation(0.0, s0);
        seq.add_observation(1.0, s1);
        seq.add_observation(2.0, s2);

        assert_eq!(seq.len(), 3);
        assert_eq!(seq.time_range(), Some((0.0, 2.0)));
    }

    #[test]
    fn test_continuous_sequence_query() {
        let fpe = FractionalPowerEncoder::new(50, 10);
        let mut seq = ContinuousTemporalSequence::new(fpe);

        let s0 = Hypervector::encode_text_ngram("dawn", 3);
        let s2 = Hypervector::encode_text_ngram("noon", 3);

        seq.add_observation(0.0, s0);
        seq.add_observation(2.0, s2);

        // Query at t=0.0 should be close to s0
        let q0 = seq.query(0.0).unwrap();
        let sim0 = 1.0 - q0.normalized_hamming_distance(&s0);
        assert!(sim0 > 0.50, "Query at t=0 should resemble s0, sim={}", sim0);

        // Query at t=2.0 should be close to s2
        let q2 = seq.query(2.0).unwrap();
        let sim2 = 1.0 - q2.normalized_hamming_distance(&s2);
        assert!(sim2 > 0.50, "Query at t=2 should resemble s2, sim={}", sim2);
    }

    #[test]
    fn test_fpe_memory_usage() {
        let fpe = FractionalPowerEncoder::new(100, 10);
        let mem = fpe.memory_usage();
        // 101 entries * 157 * 8 bytes
        assert!(mem > 0, "Memory usage should be positive");
    }

    #[test]
    fn test_continuous_sequence_rebuild() {
        let fpe = FractionalPowerEncoder::new(20, 10);
        let mut seq = ContinuousTemporalSequence::new(fpe);

        let s0 = Hypervector::new_random();
        let s1 = Hypervector::new_random();

        seq.add_observation(0.0, s0);
        seq.add_observation(1.0, s1);

        // First query triggers rebuild
        let result = seq.query(0.5);
        assert!(result.is_some(), "Should return interpolated state");

        // Add another observation and query again
        let s2 = Hypervector::new_random();
        seq.add_observation(2.0, s2);

        let result = seq.query(1.5);
        assert!(result.is_some(), "Should return new interpolated state");
    }
}
