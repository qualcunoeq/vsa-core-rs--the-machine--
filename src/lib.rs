// Allow Greek characters (θ, ε, ρ, σ, τ) in doc comments and identifiers
// to match the mathematical notation in the formal specification.
#![allow(mixed_script_confusables)]
use rand::Rng;
use std::collections::HashMap;

pub mod abstractor;
pub mod action;
pub mod bond_feeder;
pub mod self_model;
pub mod analogy;
pub mod autonomy;
pub mod bridge;
pub mod broker;
pub mod code_bridge;
pub mod compression;
pub mod defense;
pub mod drift;
pub mod drives;
pub mod forager;
pub mod hierarchy;
pub mod hnsw;
pub mod ledger;
pub mod narrative;
pub mod nlp;
pub mod planning;
pub mod predictive;
pub mod qa;
pub mod reason;
pub mod resonator;
pub mod sensory;
pub mod simulator;
pub mod sleep;
pub mod socket;
pub mod temporal;
pub mod workspace;

// CUDA-accelerated parallel centroid projection (feature gated).
// Enable with: cargo build --features cuda
#[cfg(feature = "cuda")]
pub mod cuda_projector;

// ─── DIMENSION UPGRADE v2.0 ────────────────────────────────────────────────
// D = 10240 = 160 × 64 = 40 × 256-bit AVX2 registers.
// The prime 157 was a SIMD alignment bottleneck; 160 ensures
// full vectorisation on every XOR, rotation, popcount.
pub const HD_DIMENSION: usize = 10240;
pub const U64_BLOCKS: usize = 160;

/// Number of levels for Fractional Power Encoding (FPE).
/// Each registered variable pre-generates this many level hypervectors.
pub const FPE_RESOLUTION: usize = 128;

// ─── Role vectors ──────────────────────────────────────────────────────────

impl Hypervector {
    pub fn role_market() -> Self {
        Self::encode_text_ngram("ROLE_MARKET_STATE", 3)
    }
    pub fn role_news() -> Self {
        Self::encode_text_ngram("ROLE_NEWS_STATE", 3)
    }
    pub fn role_infra() -> Self {
        Self::encode_text_ngram("ROLE_INFRA_STATE", 3)
    }
}

// ─── Serde helpers for [u64; 160] ──────────────────────────────────────────

mod array_u64_160 {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(array: &[u64; 160], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let vec: Vec<u64> = array.iter().cloned().collect();
        vec.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u64; 160], D::Error>
    where
        D: Deserializer<'de>,
    {
        let vec = Vec::<u64>::deserialize(deserializer)?;
        if vec.len() != 160 {
            return Err(serde::de::Error::custom(format!(
                "Expected array of size 160, found {}",
                vec.len()
            )));
        }
        let mut array = [0u64; 160];
        array.copy_from_slice(&vec);
        Ok(array)
    }
}

// ─── Hypervector ───────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Hypervector {
    #[serde(with = "array_u64_160")]
    pub bits: [u64; U64_BLOCKS],
}

impl Hypervector {
    /// Random hypervector (50% density)
    pub fn new_random() -> Self {
        let mut rng = rand::thread_rng();
        let mut bits = [0u64; U64_BLOCKS];
        for b in bits.iter_mut() {
            *b = rng.gen();
        }
        Hypervector { bits }
    }

    /// All-zero hypervector
    pub fn new_zero() -> Self {
        Hypervector {
            bits: [0u64; U64_BLOCKS],
        }
    }

    /// All-ones hypervector (every bit = 1)
    pub fn new_ones() -> Self {
        Hypervector {
            bits: [!0u64; U64_BLOCKS],
        }
    }

    /// Popcount (number of 1-bits).
    /// Scalar popcount on each u64 is optimal without avx512_vpopcntdq.
    pub fn count_ones(&self) -> usize {
        self.bits.iter().map(|b| b.count_ones() as usize).sum()
    }

    // ── HW-Accelerated VSA Operations (AVX-512) ──────────────────────────
    // The CPU is confirmed to have avx512f, avx512bw, avx512vl, etc.
    // These methods use 512-bit SIMD to process 8× u64 in parallel.

    /// AVX-512 XOR: processes 8 u64 blocks per instruction.
    #[cfg(target_feature = "avx512f")]
    fn avx512_xor(a: &[u64; U64_BLOCKS], b: &[u64; U64_BLOCKS], out: &mut [u64; U64_BLOCKS]) {
        use core::arch::x86_64::*;
        unsafe {
            let chunks = U64_BLOCKS / 8;
            for i in 0..chunks {
                let a_ptr = &a[i * 8] as *const u64 as *const __m512i;
                let b_ptr = &b[i * 8] as *const u64 as *const __m512i;
                let a_reg = _mm512_loadu_si512(a_ptr);
                let b_reg = _mm512_loadu_si512(b_ptr);
                let r = _mm512_xor_si512(a_reg, b_reg);
                let out_ptr = &mut out[i * 8] as *mut u64 as *mut __m512i;
                _mm512_storeu_si512(out_ptr, r);
            }
        }
    }

    /// AVX-512 XOR + scalar popcount: process 8 blocks at a time.
    #[cfg(target_feature = "avx512f")]
    fn avx512_xor_popcount(a: &[u64; U64_BLOCKS], b: &[u64; U64_BLOCKS]) -> u64 {
        use core::arch::x86_64::*;
        unsafe {
            let mut total = 0u64;
            let chunks = U64_BLOCKS / 8;
            for i in 0..chunks {
                let a_ptr = &a[i * 8] as *const u64 as *const __m512i;
                let b_ptr = &b[i * 8] as *const u64 as *const __m512i;
                let a_reg = _mm512_loadu_si512(a_ptr);
                let b_reg = _mm512_loadu_si512(b_ptr);
                let xored = _mm512_xor_si512(a_reg, b_reg);
                let lanes: [u64; 8] = core::mem::transmute(xored);
                for lane in &lanes {
                    total += lane.count_ones() as u64;
                }
            }
            total
        }
    }

    // ── Core VSA operations ───────────────────────────────────────────────

    /// Binding: A ⊕ B (bitwise XOR)
    pub fn bitwise_xor(&self, other: &Self) -> Self {
        #[cfg(target_feature = "avx512f")]
        {
            let mut result = [0u64; U64_BLOCKS];
            Self::avx512_xor(&self.bits, &other.bits, &mut result);
            return Hypervector { bits: result };
        }
        #[cfg(not(target_feature = "avx512f"))]
        {
            let mut result = [0u64; U64_BLOCKS];
            for i in 0..U64_BLOCKS {
                result[i] = self.bits[i] ^ other.bits[i];
            }
            Hypervector { bits: result }
        }
    }

    /// Normalized Hamming distance [0, 1]
    pub fn normalized_hamming_distance(&self, other: &Self) -> f64 {
        #[cfg(target_feature = "avx512f")]
        {
            let diff_count = Self::avx512_xor_popcount(&self.bits, &other.bits);
            return (diff_count as f64) / (HD_DIMENSION as f64);
        }
        #[cfg(not(target_feature = "avx512f"))]
        {
            let mut diff_count: u64 = 0;
            for i in 0..U64_BLOCKS {
                let xor_val = self.bits[i] ^ other.bits[i];
                diff_count += xor_val.count_ones() as u64;
            }
            (diff_count as f64) / (HD_DIMENSION as f64)
        }
    }

    /// Cyclic left-rotation of the bit-vector (sequence/role encoding)
    /// Note: AVX-512 not used here because the cross-64-bit carry
    /// makes SIMD rotation complex.  The scalar loop is already fast
    /// (160 iterations, no function calls).
    pub fn rotate_left(&self, shift: usize) -> Self {
        let shift = shift % HD_DIMENSION;
        if shift == 0 {
            return *self;
        }
        let word_shift = shift / 64;
        let bit_shift = shift % 64;

        let mut result = [0u64; U64_BLOCKS];
        for i in 0..U64_BLOCKS {
            let src_idx1 = (i + word_shift) % U64_BLOCKS;
            let src_idx2 = (i + word_shift + 1) % U64_BLOCKS;

            if bit_shift == 0 {
                result[i] = self.bits[src_idx1];
            } else {
                let part1 = self.bits[src_idx1] >> bit_shift;
                let part2 = self.bits[src_idx2] << (64 - bit_shift);
                result[i] = part1 | part2;
            }
        }
        Hypervector { bits: result }
    }

    // ── Character encoding ────────────────────────────────────────────────

    /// Chaotic Shift-XOR Character Encoder (unchanged from v1)
    pub fn encode_char(c: char, index_seed: usize) -> Self {
        let mut hv = [0u64; U64_BLOCKS];
        let char_val = c as u64;

        for i in 0..U64_BLOCKS {
            let mut x = char_val
                .wrapping_add(i as u64)
                .wrapping_mul(0x9E3779B97F4A7C15);
            x ^= x >> 30;
            x = x.wrapping_mul(0xBF58476D1CE4E5B9);
            x ^= x >> 27;
            hv[i] = x ^ (index_seed as u64);
        }
        Hypervector { bits: hv }
    }

    // ── String encoding ───────────────────────────────────────────────────

    /// N-gram encoding: bundles rotated char-vectors within each window
    pub fn encode_text_ngram(text: &str, k: usize) -> Self {
        if text.is_empty() {
            return Self::new_zero();
        }

        let chars: Vec<char> = text.chars().collect();
        if chars.len() < k {
            let mut ngram_vec = Self::new_zero();
            for (i, &c) in chars.iter().enumerate() {
                let char_vec = Self::encode_char(c, i);
                let rotated = char_vec.rotate_left(i);
                if i == 0 {
                    ngram_vec = rotated;
                } else {
                    ngram_vec = ngram_vec.bitwise_xor(&rotated);
                }
            }
            return ngram_vec;
        }

        let mut ngrams = Vec::new();
        for i in 0..=(chars.len() - k) {
            let mut ngram_vec = Self::new_zero();
            for j in 0..k {
                let char_vec = Self::encode_char(chars[i + j], j);
                let rotated = char_vec.rotate_left(j);
                if j == 0 {
                    ngram_vec = rotated;
                } else {
                    ngram_vec = ngram_vec.bitwise_xor(&rotated);
                }
            }
            ngrams.push(ngram_vec);
        }

        let refs: Vec<&Hypervector> = ngrams.iter().collect();
        Self::bundle(&refs)
    }

    /// Sentence encoder: position-permuted word n-grams
    pub fn encode_sentence(text: &str) -> Self {
        let words: Vec<&str> = text
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| c.is_ascii_punctuation()))
            .filter(|w| !w.is_empty())
            .collect();
        if words.is_empty() {
            return Self::new_zero();
        }
        let mut word_vectors = Vec::new();
        for (i, word) in words.iter().enumerate() {
            let word_hv = Self::encode_text_ngram(word, 3);
            let rotated = word_hv.rotate_left(i * 13);
            word_vectors.push(rotated);
        }
        let refs: Vec<&Hypervector> = word_vectors.iter().collect();
        Self::bundle(&refs)
    }

    // ── Bundling (majority rule) ──────────────────────────────────────────

    /// Standard bundling with deterministic tie-breaking
    pub fn bundle(vectors: &[&Self]) -> Self {
        if vectors.is_empty() {
            return Self::new_zero();
        }
        if vectors.len() == 1 {
            return *vectors[0];
        }

        let mut result_bits = [0u64; U64_BLOCKS];
        let num_vectors = vectors.len();
        let halfway = num_vectors / 2;
        let is_even = num_vectors % 2 == 0;

        let noise_vector = if is_even {
            let mut bits = [0u64; U64_BLOCKS];
            let first_bits = vectors.first().map(|v| v.bits[0]).unwrap_or(0);
            for i in 0..U64_BLOCKS {
                let mut x = first_bits
                    .wrapping_add(i as u64)
                    .wrapping_mul(0x9E3779B97F4A7C15);
                x ^= x >> 30;
                x = x.wrapping_mul(0xBF58476D1CE4E5B9);
                x ^= x >> 27;
                bits[i] = x;
            }
            Hypervector { bits }
        } else {
            Self::new_zero()
        };

        for block_idx in 0..U64_BLOCKS {
            let mut block_consensus = 0u64;
            for bit_idx in 0..64 {
                let mut bit_count = 0;
                for vec in vectors {
                    if ((vec.bits[block_idx] >> bit_idx) & 1) == 1 {
                        bit_count += 1;
                    }
                }

                if is_even && bit_count == halfway {
                    let noise_bit = ((noise_vector.bits[block_idx] >> bit_idx) & 1) == 1;
                    if noise_bit {
                        block_consensus |= 1 << bit_idx;
                    }
                } else if bit_count > halfway {
                    block_consensus |= 1 << bit_idx;
                }
            }
            result_bits[block_idx] = block_consensus;
        }
        Hypervector { bits: result_bits }
    }

    /// ██ UPGRADE v2.0: Recency-weighted bundling ██
    ///
    /// Weighs each vector by a recency factor before majority-voting.
    /// `weights` must have the same length as `vectors`.
    ///
    /// ██ FIXED v3.1: exact per-bit weighted majority ██
    /// Old code replicated each vector `round(weight * 8)` times (1/8 quantization)
    /// then used standard bundle. This introduced ~12.5% quantization error
    /// and capped at 32 copies (weights > 4.0 saturated). The new code computes
    /// the exact per-bit weighted majority with no quantization or saturation:
    ///
    ///   output[b] = 1  iff  Σ_i (w_i / Σⱼ wⱼ) · vector_i[b] > 0.5
    ///
    /// This is the unique maximizer of expected overlap with the oracle.
    ///
    /// If `weights` is empty or mismatched, falls back to standard bundle.
    pub fn bundle_weighted(vectors: &[&Self], weights: &[f64]) -> Self {
        if vectors.is_empty() {
            return Self::new_zero();
        }
        if vectors.len() == 1 {
            return *vectors[0];
        }
        if weights.len() != vectors.len() {
            // fallback: standard bundle
            return Self::bundle(vectors);
        }

        // Normalize weights to a proper probability distribution
        let w_sum: f64 = weights.iter().sum();
        if w_sum < 1e-30 {
            return Self::bundle(vectors);
        }
        let norm_weights: Vec<f64> = weights.iter().map(|w| w / w_sum).collect();

        // Exact per-bit weighted majority
        let u64_blocks = vectors[0].bits.len();
        let mut result = [0u64; U64_BLOCKS];
        for block in 0..u64_blocks {
            let mut word = 0u64;
            for bit in 0..64 {
                let mut w1 = 0.0;
                for (i, vec) in vectors.iter().enumerate() {
                    let b = (vec.bits[block] >> bit) & 1;
                    w1 += norm_weights[i] * b as f64;
                }
                if w1 > 0.5 {
                    word |= 1u64 << bit;
                }
            }
            result[block] = word;
        }
        Hypervector { bits: result }
    }

    /// ██ Phase 3: Constitutional bundling ██
    ///
    /// Identical to `bundle()` but uses a fixed `constitution` hypervector
    /// for tie-breaking instead of an order-dependent SplitMix64 hash.
    /// This guarantees idempotent bundling regardless of vector ordering
    /// in the input slice — critical for multi-stage consensus where
    /// bundle(A, B) must always equal bundle(B, A).
    ///
    /// The constitution is a random hypervector generated once at broker
    /// boot and used exclusively for breaking 50/50 bit-level ties:
    ///
    /// $$V_{\text{result}}[i] = \begin{cases}
    /// V[i] & \text{if } \sum\text{ones} > \text{halfway} \\
    /// 0    & \text{if } \sum\text{ones} < \text{halfway} \\
    /// C[i] & \text{if } \sum\text{ones} = \text{halfway}
    /// \end{cases}$$
    pub fn bundle_with_constitution(vectors: &[&Self], constitution: &Hypervector) -> Self {
        if vectors.is_empty() {
            return Self::new_zero();
        }
        if vectors.len() == 1 {
            return *vectors[0];
        }

        let mut result_bits = [0u64; U64_BLOCKS];
        let num_vectors = vectors.len();
        let halfway = num_vectors / 2;
        let is_even = num_vectors % 2 == 0;

        for block_idx in 0..U64_BLOCKS {
            let mut block_consensus = 0u64;
            for bit_idx in 0..64 {
                let mut bit_count = 0;
                for vec in vectors {
                    if ((vec.bits[block_idx] >> bit_idx) & 1) == 1 {
                        bit_count += 1;
                    }
                }

                if is_even && bit_count == halfway {
                    // Constitutional tie-break — order-independent
                    if ((constitution.bits[block_idx] >> bit_idx) & 1) == 1 {
                        block_consensus |= 1 << bit_idx;
                    }
                } else if bit_count > halfway {
                    block_consensus |= 1 << bit_idx;
                }
            }
            result_bits[block_idx] = block_consensus;
        }
        Hypervector { bits: result_bits }
    }

    /// ██ UPGRADE v2.0: Fractional Power Encoding (FPE) ██
    ///
    /// Encodes a continuous scalar `x` by selecting a level hypervector
    /// from a pre-generated ladder.  Unlike the old linear interpolation
    /// (which destroys pseudo-orthogonality), FPE flips a small subset of
    /// bits per step so that Hamming distance ∝ |x₁ - x₂|.
    ///
    /// Pre-generate level vectors by calling `generate_level_vectors()`,
    /// then use `encode_fpe()` to look up the nearest level.
    pub fn encode_fpe(level_vectors: &[Hypervector], val: f64, min_val: f64, max_val: f64) -> Self {
        let clamped = val.clamp(min_val, max_val);
        let fraction = (clamped - min_val) / (max_val - min_val);
        let idx = ((fraction * (level_vectors.len() - 1) as f64).round() as usize)
            .min(level_vectors.len() - 1);
        level_vectors[idx]
    }

    /// Generate a ladder of `num_levels` FPE hypervectors.
    /// Each step flips ~D/200 bits so distance scales with ordinal offset.
    pub fn generate_level_vectors(num_levels: usize) -> Vec<Hypervector> {
        let mut levels = Vec::with_capacity(num_levels);
        let mut current = Self::new_random();
        levels.push(current);

        let flip_count = (HD_DIMENSION / 200).max(1);
        let mut rng = rand::thread_rng();

        for _ in 1..num_levels {
            let mut next = current;
            for _ in 0..flip_count {
                let block = rng.gen_range(0..U64_BLOCKS);
                let bit = rng.gen_range(0..64);
                next.bits[block] ^= 1u64 << bit;
            }
            levels.push(next);
            current = next;
        }

        levels
    }

    // ── Byte serialization (1280 bytes for D=10240) ───────────────────────

    /// Serialize to 1280 bytes (160 × 8)
    pub fn to_bytes(&self) -> [u8; 1280] {
        let mut bytes = [0u8; 1280];
        for i in 0..U64_BLOCKS {
            let block_bytes = self.bits[i].to_le_bytes();
            bytes[i * 8..(i + 1) * 8].copy_from_slice(&block_bytes);
        }
        bytes
    }

    /// Deserialize from 1280 bytes
    pub fn from_bytes(bytes: &[u8; 1280]) -> Self {
        let mut bits = [0u64; U64_BLOCKS];
        for i in 0..U64_BLOCKS {
            let mut block_bytes = [0u8; 8];
            block_bytes.copy_from_slice(&bytes[i * 8..(i + 1) * 8]);
            bits[i] = u64::from_le_bytes(block_bytes);
        }
        Hypervector { bits }
    }
}

// ─── VarConfig (FPE-based) ────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct VarConfig {
    pub id: Hypervector,
    pub min_val: f64,
    pub max_val: f64,
    /// ██ UPGRADE v2.0: FPE level vectors instead of base_min/base_max ██
    pub level_vectors: Vec<Hypervector>,
}

// ─── Memory types ─────────────────────────────────────────────────────────

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DejavuEntry {
    /// The stored vector.  If `delta_encoded == true`, this is
    /// $C_{\text{delta}} = C_{\text{new}} \oplus C_{\text{centroid}}$ —
    /// the orthogonal residual from the cluster centroid.
    /// Exact recovery: $C_{\text{new}} = C_{\text{delta}} \oplus C_{\text{centroid}}$.
    pub vector: Hypervector,
    pub label: String,
    pub metadata: HashMap<String, String>,
    /// ██ UPGRADE v2.2: Continuous Orthogonal Projection ██
    /// If true, `vector` stores $C_{\text{new}} \oplus C_{\text{centroid}}$
    /// rather than the raw $C_{\text{new}}$.  Reconstruction requires
    /// XOR with the owning cluster's current centroid.
    #[serde(default)]
    pub delta_encoded: bool,
    /// ██ FIX v2.6 (Layer 2): Entry weight for age-weighted merging ██
    /// Number of original observations this entry represents.
    /// Normal entries have weight = 1.  Merged entries have weight > 1
    /// and participate proportionally in centroid computation.
    #[serde(default)]
    pub weight: u32,
    /// ██ FIX v2.6 (Layer 2): Tick when this entry was created ██
    /// Used by merge_entries to partition entries into age cohorts.
    #[serde(default)]
    pub creation_tick: u64,
}

impl DejavuEntry {
    /// Reconstruct the original vector from a delta-encoded entry.
    /// If `delta_encoded == false`, returns `self.vector` directly.
    /// Otherwise returns `self.vector ⊕ centroid`.
    pub fn reconstruct(&self, centroid: &Hypervector) -> Hypervector {
        if self.delta_encoded {
            self.vector.bitwise_xor(centroid)
        } else {
            self.vector
        }
    }

    /// Factory: create a new entry, optionally delta-encoding against a centroid.
    pub fn new(
        vector: Hypervector,
        label: String,
        metadata: HashMap<String, String>,
        delta_against: Option<&Hypervector>,
    ) -> Self {
        let (stored_vector, delta_encoded) = match delta_against {
            Some(centroid) => (vector.bitwise_xor(centroid), true),
            None => (vector, false),
        };
        DejavuEntry {
            vector: stored_vector,
            label,
            metadata,
            delta_encoded,
            weight: 1,
            creation_tick: 0,
        }
    }

    /// Create a merged entry with explicit weight and tick provenance.
    pub fn new_merged(
        vector: Hypervector,
        label: String,
        weight: u32,
        creation_tick: u64,
    ) -> Self {
        DejavuEntry {
            vector,
            label,
            metadata: HashMap::new(),
            delta_encoded: false,
            weight,
            creation_tick,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MemoryCluster {
    /// Dynamic semantic centroid — used for LSH routing, Goldilocks Phase 1
    /// matching, and dissonance evaluation.  Updated after every merge.
    pub centroid: Hypervector,
    pub entries: Vec<DejavuEntry>,
    #[serde(default)]
    pub reverberation: f64,
    #[serde(default)]
    pub last_reinforced_tick: usize,
    /// ██ UPGRADE v2.2: Locked Anchor (Reference Frame) ██
    ///
    /// Immutable reference vector set at cluster creation.  ALL delta
    /// encoding/decoding uses this anchor, NOT the drifting centroid.
    /// This guarantees exact recovery forever.
    #[serde(default = "Hypervector::new_zero")]
    pub anchor: Hypervector,
    /// ██ Tier 4: Integer Accumulator (Evidence Integration) ██
    ///
    /// Per-dimension u32 counter tracking the total evidence for each
    /// of the 10240 bits.  The binary `centroid` is the thresholded
    /// version: `centroid[i] = 1 iff accumulator[i] > total_weight / 2`.
    ///
    /// NOT serialized.  Lazily reconstructed from entries or centroid
    /// on first use via `ensure_accumulator()`.  Empty = frozen state
    /// (only binary centroid is live).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accumulator: Vec<u32>,
    /// Total evidence count: |entries| + # of Hebbian refinements.
    #[serde(default)]
    pub total_weight: u32,
    /// ██ Tier 4: Last access tick for hot/cold memory management ██
    /// Updated on every query (read or write).  Clusters whose
    /// `last_access_tick` is more than `FREEZE_AFTER_TICKS` behind the
    /// current tick have their accumulator dropped to save memory.
    #[serde(default)]
    pub last_access_tick: u64,
}

/// EWMA smoothing factor for drift magnitude tracking (Theorem XXIII.4).
/// α = 0.05 gives half-life ≈ ln(2)/ln(1/0.95) ≈ 13.5 ticks.
/// Fast enough to detect drift onset within ~14 ticks, slow enough to
/// reject single-tick noise bursts.
pub const DRIFT_MAGNITUDE_ALPHA: f64 = 0.05;

/// Maximum within-cluster tracking rate (Theorem XXIII.4).
/// Per-tick drift δ in NHD above this value triggers the adaptive novelty
/// gate in `add_to_dejavu_db`, which lowers the absorption threshold to
/// force faster centroid catching-up and bound cluster proliferation.
pub const DELTA_MAX: f64 = 0.00035;

/// Baseline NHD threshold for the `add_to_dejavu_db` cluster absorption gate.
/// Corresponds to similarity 0.65.  The adaptive gate raises or lowers
/// this threshold based on measured drift rate.
pub const THETA_MAIN_BASELINE: f64 = 0.35;

/// Floor of the adaptive novelty gate (NHD).  Must be > θ_merge (0.30)
/// so that even under maximal absorption pressure, the gate never merges
/// clusters that the compactor would keep separate.
pub const THETA_ADAPT_MIN: f64 = 0.32;

/// Default projection threshold (NHD) for cluster anchoring.
/// Derived from θ* = (3ε - 2ε²)/2 with ε = 0.50 (worst-case composition noise).
pub const DEFAULT_PROJECTION_THRESHOLD_NHD: f64 = 0.50;

/// ██ FIX v2.5: Maximum total weight for a single cluster ██
///
/// Prevents unbounded tracking error growth (Theorem XXIII.1).
/// Without a cap, a cluster's centroid becomes increasingly sluggish
/// as W → ∞, requiring O(W) contradictory observations to flip a bit.
///
/// At MAX_CLUSTER_WEIGHT = 500, each new observation moves the centroid
/// by at most 1/500 ≈ 0.2% — responsive enough to track gradual drift
/// while stable enough to filter noise.
///
/// When weight exceeds this cap, the accumulator is rescaled during
/// `absorb_entry` to maintain centroid responsiveness.  This does NOT
/// change the centroid (it's a fixed point of rescaling) but ensures
/// future observations have proportional influence.
pub const MAX_CLUSTER_WEIGHT: u32 = 500;

/// Maximum entries per MemoryCluster before oldest are evicted.
/// Entries accumulate from each novelty gate pass and can grow unbounded.
/// 1000 entries × ~2 KB each = ~2 MB per cluster, negligible memory.
pub const MAX_ENTRIES_PER_CLUSTER: usize = 1000;

/// ██ FIX v2.5: Accumulator decay tick interval ██
///
/// How often the accumulator decay is applied in the agent loop.
/// Every DECAY_INTERVAL_TICKS, each cluster's accumulator is
/// multiplied by DECAY_FACTOR, aging out old evidence and allowing
/// centroid bits to flip from 1→0 when contradicted by recent input.
pub const ACCUMULATOR_DECAY_INTERVAL: usize = 50;
pub const ACCUMULATOR_DECAY_FACTOR: f64 = 0.975;

/// Outcome of the two-threshold novelty gate applied to a MemoryCluster.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateAction {
    /// NHD < 0.15: centroid reinforced (self-reinforcement), no entry.
    HebbianRefine,
    /// 0.15 ≤ NHD < 0.70: new entry appended, accumulator absorbs τ.
    Absorbed,
    /// NHD ≥ 0.70: concept is too distant — caller should create a new cluster.
    NewCluster,
    /// Episode desirability ≤ 0.6: no action taken.
    Discard,
}

impl MemoryCluster {
    /// Ensure the Locked Anchor is initialized.  If still zero (e.g. the
    /// cluster arrived via broker SyncUpdate which may not carry an anchor),
    /// set it to the current centroid.  Returns `true` if anchor was set.
    pub fn ensure_anchor(&mut self) -> bool {
        let is_zero = self.anchor.bits.iter().all(|&b| b == 0);
        if is_zero {
            self.anchor = self.centroid;
            true
        } else {
            false
        }
    }

    // ── Tier 4: Integer Accumulator ─────────────────────────────────

    /// Lazily reconstruct the accumulator from the current centroid
    /// and entry count.  This is the "cold start" path for clusters
    /// deserialized from the ledger or received via SyncUpdate.
    ///
    /// Reconstruction guarantees that `centroid` is a fixed point of
    /// the accumulator threshold: bits that are 1 in the centroid
    /// clear the threshold by the smallest possible margin.
    pub fn ensure_accumulator(&mut self) {
        if !self.accumulator.is_empty() {
            return;
        }
        if self.total_weight == 0 {
            self.total_weight = self.entries.len().max(1) as u32;
        }
        self.accumulator = vec![0u32; HD_DIMENSION];
        let threshold = (self.total_weight / 2) + 1;
        for (i, acc) in self.accumulator.iter_mut().enumerate() {
            let word = self.centroid.bits[i / 64];
            let bit = (word >> (i % 64)) & 1;
            if bit == 1 {
                *acc = threshold;
            } else {
                *acc = threshold - 1;
            }
        }
    }

    /// Self-reinforcement: add the binary centroid to the accumulator.
    ///
    /// The centroid is a **fixed point** under this operation (proved
    /// in the architectural synthesis).  Every bit that is 1 gets +1;
    /// every bit that is 0 gets +0.  Total weight increments by 1.
    /// The binary centroid does not change.
    ///
    /// This is called for routine observations (NHD < 0.15) that
    /// confirm the existing concept without introducing new evidence.
    ///
    /// ██ FIX v2.5: Weight cap ██
    /// Same rescaling as `absorb_entry` to keep centroid responsive.
    pub fn hebbian_refine(&mut self) {
        self.ensure_accumulator();
        for (i, acc) in self.accumulator.iter_mut().enumerate() {
            let word = self.centroid.bits[i / 64];
            let bit = (word >> (i % 64)) & 1;
            *acc += bit as u32;
        }
        self.total_weight += 1;

        // ██ Weight cap ██
        if self.total_weight > MAX_CLUSTER_WEIGHT {
            let centroid_before = self.centroid;
            let scale = MAX_CLUSTER_WEIGHT as f64 / self.total_weight as f64;
            for acc in self.accumulator.iter_mut() {
                *acc = (*acc as f64 * scale).round() as u32;
            }
            self.total_weight = MAX_CLUSTER_WEIGHT;

            // ██ FIX v3.1: Preserve centroid fixed-point under rescaling ██
            let new_threshold = self.total_weight / 2;
            for (i, acc) in self.accumulator.iter_mut().enumerate() {
                let word = centroid_before.bits[i / 64];
                let bit_before = (word >> (i % 64)) & 1;
                let is_above = *acc > new_threshold;
                if bit_before == 1 && !is_above {
                    *acc = new_threshold + 1;
                } else if bit_before == 0 && is_above {
                    *acc = new_threshold;
                }
            }
        }
    }

    /// Absorb a new observation τ into the accumulator and recompute
    /// the binary centroid.
    ///
    /// Called for observations in the drift zone (0.15 ≤ NHD < 0.70)
    /// that contribute genuinely new evidence to the cluster.
    ///
    /// ██ FIX v2.5: Weight cap ██
    /// When `total_weight` exceeds `MAX_CLUSTER_WEIGHT`, the accumulator
    /// and weight are rescaled so that future observations retain
    /// proportional influence.  This prevents the centroid from becoming
    /// pathologically sluggish under persistent drift (Theorem XXIII.1).
    ///
    /// The centroid is a **fixed point** under rescaling IN EXACT ARITHMETIC:
    ///   centroid[i] = 1  ⇔  acc[i] > W/2
    ///   After scaling: acc'[i] = round(acc[i] · s), W' = round(W · s)
    ///   In exact reals the inequality is preserved; with integer rounding
    ///   the centroid can drift (~0.23% per 1000 obs before v3.1 fix).
    ///
    /// ██ FIX v3.1: The rounding in `acc' = round(acc·s)` can flip marginal
    /// bits (1→0 or 0→1) with no contradictory evidence.  Example at W=501,
    /// scale=500/501: 1-bit acc=251 → round(251·500/501) = round(250.499) =
    /// 250, then threshold 250 gives 250 > 250 false → bit flips 1→0.
    /// The fix (below after the rescaling loop) detects and corrects these
    /// false flips, preserving the centroid as a true fixed point.
    ///
    /// Returns (centroid_shift, input_distance) for
    /// joint contraction telemetry (κ_F measurement).
    ///
    ///   centroid_shift = δ(centroid_before, centroid_after)
    ///   input_distance = δ(centroid_before, τ)
    ///
    /// Callers that don't need telemetry can ignore the return value.
    pub fn absorb_entry(&mut self, tau: &Hypervector) -> (f64, f64) {
        let centroid_before = self.centroid;
        let input_dist = centroid_before.normalized_hamming_distance(tau);

        self.ensure_accumulator();
        for (i, acc) in self.accumulator.iter_mut().enumerate() {
            let word = tau.bits[i / 64];
            let bit = (word >> (i % 64)) & 1;
            *acc += bit as u32;
        }
        self.total_weight += 1;

        // ██ Weight cap: keep centroid responsive ██
        if self.total_weight > MAX_CLUSTER_WEIGHT {
            let scale = MAX_CLUSTER_WEIGHT as f64 / self.total_weight as f64;
            for acc in self.accumulator.iter_mut() {
                *acc = (*acc as f64 * scale).round() as u32;
            }
            self.total_weight = MAX_CLUSTER_WEIGHT;

            // ██ FIX v3.1: Preserve centroid fixed-point under rescaling ██
            //
            // Rounding after rescaling can flip marginal bits (those with
            // acc = W/2 + 1 before rescaling) from 1→0 or 0→1 even when
            // no genuine evidence change justifies the flip. This violates
            // the fixed-point theorem (centroid should not change under
            // rescaling alone — only evidence changes should affect it).
            //
            // Example of the bug at W=501, scale=500/501:
            //   1-bit acc=251 → round(251·500/501) = round(250.499) = 250
            //   After: W=500, acc=250, threshold=250 → 250 > 250 is false
            //   → bit flips from 1→0 with NO contradictory evidence.
            //
            // Fix: enforce that bits preserve their centroid status unless
            // the observation genuinely shifted the evidence. Since rescaling
            // is a similarity transform (all values multiplied by same scale),
            // it cannot change which bits are above threshold — only rounding
            // can. We correct the rounding errors here.
            let new_threshold = self.total_weight / 2;
            for (i, acc) in self.accumulator.iter_mut().enumerate() {
                let word = centroid_before.bits[i / 64];
                let bit_before = (word >> (i % 64)) & 1;
                let is_above = *acc > new_threshold;
                if bit_before == 1 && !is_above {
                    // Rescaling falsely rounded this 1-bit below threshold
                    *acc = new_threshold + 1;
                } else if bit_before == 0 && is_above {
                    // Rescaling falsely rounded this 0-bit above threshold
                    *acc = new_threshold;
                }
            }
        }

        self.recompute_centroid();
        let centroid_shift = centroid_before.normalized_hamming_distance(&self.centroid);
        (centroid_shift, input_dist)
    }

    /// Recompute the binary centroid from the accumulator threshold.
    ///
    /// `centroid[i] = 1` iff `accumulator[i] > total_weight / 2`.
    /// This is the "integrate-and-fire" step: bits that cross the
    /// majority threshold turn on; bits that fall below turn off.
    pub fn recompute_centroid(&mut self) {
        let threshold = (self.total_weight / 2) as u64;
        for (word_idx, word_bits) in self.centroid.bits.iter_mut().enumerate() {
            *word_bits = 0;
            let base = word_idx * 64;
            for bit_idx in 0..64 {
                let acc_idx = base + bit_idx;
                if acc_idx < self.accumulator.len() {
                    let acc_val = self.accumulator[acc_idx] as u64;
                    if acc_val > threshold {
                        *word_bits |= 1_u64 << bit_idx;
                    }
                }
            }
        }
        // Enforce ρ-admissible invariant: δ(c, ρ¹³(c)) > 0.
        // Required for Assumption ρ in Theorem XXV.4.
        // This is a no-op for non-pathological centroids.
        self.enforce_rho_admissible();
    }

    /// Record an access (read or write) at the given tick.
    /// Used by the hot/cold memory manager to track recency.
    pub fn touch_access(&mut self, tick: u64) {
        self.last_access_tick = tick;
    }

    /// ██ FIX v2.6 (Layer 2): Rebuild accumulator from entries with weights ██
    ///
    /// This is the correct recomputation after entry merging.  Each entry
    /// contributes `weight` observations to the accumulator.  The centroid
    /// is then recomputed from the rebuilt accumulator.
    ///
    /// This is O(entries × D) so it's only called when merging triggers.
    /// It replaces `ensure_accumulator()` semantics — after this call the
    /// accumulator exactly reflects all stored entries.
    pub fn rebuild_accumulator_from_entries(&mut self) {
        let mut new_acc = vec![0u32; HD_DIMENSION];
        let mut new_total_weight: u32 = 0;

        for entry in &self.entries {
            let vec = entry.reconstruct(&self.anchor);
            let w = entry.weight.max(1);
            new_total_weight = new_total_weight.saturating_add(w);
            for (i, acc) in new_acc.iter_mut().enumerate() {
                let word = vec.bits[i / 64];
                let bit = ((word >> (i % 64)) & 1) as u32;
                *acc = acc.saturating_add(bit * w);
            }
        }

        self.accumulator = new_acc;
        self.total_weight = new_total_weight.max(1);
        self.recompute_centroid();
    }

    /// Drop the accumulator to save memory (freeze).
    /// The centroid is preserved; `ensure_accumulator()` will lazily
    /// reconstruct the accumulator on the next access.
    pub fn freeze(&mut self) {
        self.accumulator.clear();
    }

    /// Return `true` if the accumulator is resident (hot).
    pub fn is_hot(&self) -> bool {
        !self.accumulator.is_empty()
    }

    /// ██ v3.1: Enforce ρ-admissible invariant (Assumption ρ, Theorem XXV.4) ██
    ///
    /// The operator f = nearest ∘ P_τ ∘ ρ¹³ has its centroid transition domain
    /// in ρ²⁶(W_i), not W_i or ρ¹³(W_i).  The Sub-Lemma S constructive proof
    /// requires that NO centroid is a fixed point of ρ¹³, ρ²⁶, or ρ⁵².
    ///
    /// Fixed points of ρ¹³ (shift by 13):
    ///   gcd(13, 10240) = 1 → ρ¹³ generates C_10240.
    ///   Only constant vectors (all-zeros, all-ones) are fixed points.
    ///
    /// Fixed points of ρ²⁶ (shift by 26):
    ///   gcd(26, 10240) = 2 → ρ²⁶ generates C_5120.
    ///   Additional fixed points: period-2 vectors (0101..., 1010...).
    ///   These pass ρ¹³ (δ = 1.0 since 13 odd) but collapse ρ²⁶(W_i) → W_i.
    ///
    /// Fixed points of ρ⁵² (shift by 52):
    ///   gcd(52, 10240) = 4 → ρ⁵² generates C_2560.
    ///   Additional fixed points: period-4 vectors (0011..., 1100...,
    ///   0110..., etc).  These pass ρ¹³ and ρ²⁶ but break the constructive
    ///   witness in Sub-Lemma S (the inequality d_ptr - d_ptc < 2r_i fails
    ///   when d(c_i, ρ⁻⁵²(c_i)) = 0).
    ///
    /// The three checks are cheap: three XOR + popcount at compaction time.
    /// Real-world embeddings never produce these degeneracies, but the
    /// invariant is enforced unconditionally as a formal safety guarantee.
    pub fn enforce_rho_admissible(&mut self) {
        // Check ρ¹³ (shift by 13) — catches constant vectors.
        let r13 = self.centroid.rotate_left(13);
        if self.centroid.normalized_hamming_distance(&r13) == 0.0 {
            // Fixed point of ρ¹³ — flip bit 0 to break symmetry.
            self.centroid.bits[0] ^= 1;
            if !self.accumulator.is_empty() {
                let threshold = self.total_weight / 2;
                self.accumulator[0] = if self.centroid.bits[0] & 1 == 1 {
                    threshold + 1
                } else {
                    threshold
                };
            }
        }

        // Check ρ²⁶ (shift by 26) — catches period-2 vectors.
        // Uses bit 1 (different from bit 0 for ρ¹³) to avoid conflict.
        let r26 = self.centroid.rotate_left(26);
        if self.centroid.normalized_hamming_distance(&r26) == 0.0 {
            self.centroid.bits[1] ^= 1;
            if !self.accumulator.is_empty() {
                let threshold = self.total_weight / 2;
                self.accumulator[1] = if self.centroid.bits[1] & 1 == 1 {
                    threshold + 1
                } else {
                    threshold
                };
            }
        }

        // Check ρ⁵² (shift by 52) — catches period-4 vectors.
        // Uses bit 2 (different from bit 0 for ρ¹³, bit 1 for ρ²⁶).
        // Required by Sub-Lemma S constructive proof (Theorem XXV.5):
        // the witness construction needs d(c_i, ρ⁻⁵²(c_i)) > 0.
        let r52 = self.centroid.rotate_left(52);
        if self.centroid.normalized_hamming_distance(&r52) == 0.0 {
            self.centroid.bits[2] ^= 1;
            if !self.accumulator.is_empty() {
                let threshold = self.total_weight / 2;
                self.accumulator[2] = if self.centroid.bits[2] & 1 == 1 {
                    threshold + 1
                } else {
                    threshold
                };
            }
        }
    }

    /// ██ FIX v2.5: Decay the accumulator to age out old evidence ██
    ///
    /// Each accumulator entry is multiplied by `decay_factor` (0.0–1.0),
    /// and `total_weight` is similarly decayed.  After decay the centroid
    /// is recomputed, which MAY flip bits from 1→0 when their accumulated
    /// evidence drops below the new threshold.
    ///
    /// This directly addresses the **accumulator asymmetry** problem:
    /// without decay, bits that reach 1 are locked forever because the
    /// accumulator only increments.  Decay gives a gradual forgetting
    /// mechanism so contradictory evidence can eventually flip a bit.
    ///
    /// ## Decay schedule
    /// Called every `ACCUMULATOR_DECAY_INTERVAL` ticks (default 50).
    /// At `decay_factor = 0.975` per 50 ticks, the effective half-life
    /// of any accumulator entry is ≈ 1360 observations:
    ///
    ///   t_{1/2} = 50 · ln(0.5) / ln(0.975) ≈ 1368
    ///
    /// This is long enough for stable patterns to entrench, but short
    /// enough for gradual drift to flip bits within ~200 observations.
    pub fn decay_accumulator(&mut self, decay_factor: f64) {
        self.ensure_accumulator();
        for acc in self.accumulator.iter_mut() {
            *acc = (*acc as f64 * decay_factor).round() as u32;
        }
        self.total_weight = (self.total_weight as f64 * decay_factor).round() as u32;
        if self.total_weight < 1 {
            self.total_weight = 1;
        }
        self.recompute_centroid();
    }

    /// Apply the two-threshold novelty gate to an incoming temporal
    /// centroid τ from a completed episode.
    ///
    /// | NHD(τ, centroid) | Interpretation | Action |
    /// |---|---|---|
    /// | < 0.15 | Routine | Hebbian refine (no entry) |
    /// | 0.15 – 0.70 | Drift zone | Append entry + absorb |
    /// | ≥ 0.70 | Novel concept | Return NewCluster |
    ///
    /// Returns the `GateAction` so the caller can manage cluster
    /// proliferation appropriately.
    pub fn novelty_gate(&mut self, tau: &Hypervector, episode_desirability: f64) -> GateAction {
        if episode_desirability <= 0.6 {
            return GateAction::Discard;
        }

        let nhd = tau.normalized_hamming_distance(&self.centroid);

        if nhd < 0.15 {
            self.hebbian_refine();
            GateAction::HebbianRefine
        } else if nhd < 0.70 {
            let entry = DejavuEntry::new(
                *tau,
                format!("ep_{}", self.total_weight),
                std::collections::HashMap::new(),
                None,
            );
            self.entries.push(entry);
            if self.entries.len() > MAX_ENTRIES_PER_CLUSTER {
                let drain = MAX_ENTRIES_PER_CLUSTER / 4;
                self.entries.drain(0..drain);
            }
            self.absorb_entry(tau);
            GateAction::Absorbed
        } else {
            GateAction::NewCluster
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum HiveMessage {
    HandshakeRequest { agent_id: String, role: String },
    HandshakeResponse { permanent_clusters: Vec<MemoryCluster> },
    ConsolidateRequest {
        centroid: Hypervector,
        entries: Vec<DejavuEntry>,
        agent_anxiety: f64,
    },
    SyncUpdate {
        is_new_cluster: bool,
        cluster_index: Option<usize>,
        cluster: MemoryCluster,
    },
    PanicLockdown { attacker_info: String },
    DissonanceAlert {
        consensus_similarity: f64,
        agent_count: usize,
    },
    /// ██ Tier 4: Epistemic update after an agent executes an action.
    /// Broadcast to ALL agents so they can update their private accumulators
    /// with the new world state.  `intent_frequency_increment` controls
    /// whether the intent cluster's frequency is incremented (agreeing
    /// agents) or not (abstaining via Conscience Clause).
    EpistemicUpdate {
        new_world_state: Hypervector,
        intent_id: u64,
        executor_id: String,
        tick: u64,
        /// If true, the receiving agent should increment the intent cluster
        /// frequency (epistemic + instrumental learning).
        /// If false, only epistemic learning (accumulator update, no frequency).
        intent_frequency_increment: bool,
        /// Monotonically increasing serial for idempotent retry detection.
        failure_serial: u64,
    },
    /// ██ Tier 4: Delegated execution.
    /// The broker assigns one agent as executor.  All other agents
    /// wait for the EpistemicUpdate before absorbing the result.
    ExecutionRequest {
        intent: Hypervector,
        executor_id: String,
        failure_serial: u64,
    },
}

#[derive(Clone, Debug)]
pub struct TransientCluster {
    pub centroid: Hypervector,
    pub entries: Vec<DejavuEntry>,
    pub reverberation: f64,
    pub last_reinforced_tick: usize,
    /// Placeholder anchor — always zero.  Transient clusters are never
    /// delta-encoded, so this field exists only for type compatibility
    /// with the `search_clusters!` and `eval_entry!` macros.
    pub anchor: Hypervector,
    /// ██ FIX v2.6: Hot/cold management for transient clusters ██
    /// Tracks the last tick this cluster was accessed (read or write).
    /// Used by `freeze_cold_transient_clusters` to reclaim memory
    /// from idle transient clusters by dropping their entry vectors.
    /// The centroid is preserved (it's small: 1,280 bytes), but the
    /// entries (which can grow to ~2 MB) are dropped.
    pub last_access_tick: u64,
    /// ██ FIX v2.6: Whether this transient cluster is "frozen" ██
    /// Frozen transient clusters have their entries dropped but
    /// preserve their centroid for matching.  New entries can
    /// "thaw" the cluster on access.
    pub frozen: bool,
}

// ─── VSABrain ─────────────────────────────────────────────────────────────

pub struct VSABrain {
    pub variables: HashMap<String, VarConfig>,
    pub concepts: HashMap<String, Hypervector>,
    pub dejavu_clusters: Vec<MemoryCluster>,
    pub transient_clusters: Vec<TransientCluster>,
    pub threshold: f64,
    pub tick_counter: usize,
    pub anxiety: f64,
    pub experiences: Vec<Hypervector>,
    /// Joint contraction telemetry for runtime κ_P and κ_F monitoring.
    pub contraction_telemetry: ContractionTelemetry,
    /// Soft projection temperature τ.
    /// 0.0 = hard projection (default, backward compatible).
    /// 0.08 = recommended sweet spot (76× capacity gain, κ_P ≈ 1.0, empirically calibrated
    ///        via frontier sweep with corrected v3.1 formula).
    /// 0.10 = high-capacity alternative (72× gain, κ_P ≈ 0.89, slight mush).
    /// See `test_soft_projection_frontier_sweep` for calibration data.
    pub soft_projection_tau: f64,
    /// ██ FIX v2.6 (Layer 3): Cold storage for frozen clusters ██
    /// When a cluster is frozen, its entries and accumulator are serialized
    /// and stored here.  On write access, the cluster is thawed.
    pub cold_storage: crate::compression::ColdStorageManager,

    /// ██ UPGRADE v3.0: Cross-Cluster Associative Binding ██
    ///
    /// Maps cluster index → list of (target_cluster_index, association_vector, strength, tick).
    /// When two clusters co-activate within a short window, an association is
    /// learned: association_{ij} = centroid_i ⊕ centroid_j.
    ///
    /// This allows the system to:
    /// 1. Retrieve cluster j by activating cluster i:  j_est = centroid_i ⊕ assoc_{ij}
    /// 2. Learn "semantic nearness" between concepts that co-occur
    /// 3. Cascade activation through chains of associations
    ///
    /// The association vector is stored alongside a strength (0.0–1.0) that
    /// increases with each co-activation and decays with time.
    pub cross_cluster_associations: HashMap<usize, Vec<(usize, Hypervector, f64, u64)>>,

    /// ██ UPGRADE v3.0: Recent cluster activation history ██
    /// Tracks which clusters were activated in recent ticks for
    /// co-occurrence learning.  Maps tick → set of activated cluster indices.
    /// Kept for the last ASSOCIATION_WINDOW_TICKS ticks.
    pub activation_history: HashMap<u64, Vec<usize>>,

    /// ██ Theorem XXIII.4: Drift magnitude EWMA ██
    /// Per-tick drift magnitude δ_measured(t) = popcount(Δ_t) / D, smoothed
    /// via EWMA with α = DRIFT_MAGNITUDE_ALPHA.  Used by `adaptive_novelty_threshold`
    /// to detect when drift exceeds δ_max = 0.00035.
    /// Initialized to 0.0; converges within ~3 half-lives (~42 ticks).
    pub drift_magnitude_ewma: f64,
}

/// Synthetic cold-start regime labels for Tick 0 initialization.
/// These deterministic text encodings produce reproducible hypervectors
/// that span the three BMA regimes (stable, nominal, volatile) with
/// pairwise Hamming variance > 0.38, enabling immediate multi-regime
/// forecasting and non-zero dissonance.
pub const SYNTH_STABLE: &str = "SYNTHETIC REGIME STABLE EQUILIBRIUM";
pub const SYNTH_NOMINAL: &str = "SYNTHETIC REGIME NOMINAL MARKET";
pub const SYNTH_VOLATILE: &str = "SYNTHETIC REGIME VOLATILE CRISIS";
pub const SYNTH_ACTION_NULL: &str = "NULL_ACTION_NOOP";
pub const SYNTH_PARAM_NULL: &str = "NULL_PARAM_BASELINE";

impl VSABrain {
    pub fn new(threshold: f64) -> Self {
        VSABrain {
            variables: HashMap::new(),
            concepts: HashMap::new(),
            dejavu_clusters: Vec::new(),
            transient_clusters: Vec::new(),
            threshold,
            tick_counter: 0,
            anxiety: 0.0,
            experiences: Vec::new(),
            contraction_telemetry: ContractionTelemetry::new(),
            soft_projection_tau: 0.0, // default: hard projection (backward compatible)
            cold_storage: crate::compression::ColdStorageManager::new(),
            cross_cluster_associations: HashMap::new(),
            activation_history: HashMap::new(),
            drift_magnitude_ewma: 0.0,
        }
    }

    /// ██ UPGRADE v2.2: Synthetic Regime Injection (Tick 0) ██
    ///
    /// Seeds the brain's experience buffer with N synthetic observations
    /// that span all three drift regimes.  This immediately:
    ///
    /// 1. Establishes a non-zero baseline for dissonance calculation
    /// 2. Enables outcome-vector learning penalties from the first tick
    /// 3. Provides priors for the Bayesian Model Average forecaster
    ///
    /// Returns a tuple `(stable_state, nominal_state, volatile_state, delta_history)`
    /// that the caller can use to pre-seed the `recent_deltas` and `recent_states`
    /// queues, completely bypassing the 30-tick exploratory phase.
    ///
    /// ## Experience bundle formula
    ///
    /// $$\mathcal{E}_0 = \sum_{k=1}^{N} \left( A_{\text{null}} \otimes P_{\text{null}} \otimes S_{k,\text{synth}} \otimes O_{k,\text{regime}} \right)$$
    /// Maximum number of experience vectors to store.
    /// Older experiences are dropped when this limit is exceeded.
    const MAX_EXPERIENCES: usize = 1000;

    /// Push an experience vector, capping at MAX_EXPERIENCES.
    pub fn push_experience(&mut self, exp: Hypervector) {
        if self.experiences.len() >= Self::MAX_EXPERIENCES {
            let drain = Self::MAX_EXPERIENCES / 4;
            self.experiences.drain(0..drain);
        }
        self.experiences.push(exp);
    }

    pub fn seed_synthetic_regimes(&mut self) -> (Hypervector, Hypervector, Hypervector, Vec<Hypervector>) {
        let a_null = Hypervector::encode_text_ngram(SYNTH_ACTION_NULL, 3);
        let p_null = Hypervector::encode_text_ngram(SYNTH_PARAM_NULL, 3);

        // Three synthetic regime states (deterministic — not random)
        let s_stable = Hypervector::encode_sentence(SYNTH_STABLE);
        let s_nominal = Hypervector::encode_sentence(SYNTH_NOMINAL);
        let s_volatile = Hypervector::encode_sentence(SYNTH_VOLATILE);

        // Outcome labels for each regime
        let o_stable = Hypervector::encode_text_ngram("OUTCOME_STABLE", 3);
        let o_nominal = Hypervector::encode_text_ngram("OUTCOME_NOMINAL", 3);
        let o_volatile = Hypervector::encode_text_ngram("OUTCOME_VOLATILE", 3);

        // Experience bundle: A_null ⊕ P_null ⊕ S_synth ⊕ O_regime
        let exp_stable = a_null
            .bitwise_xor(&p_null)
            .bitwise_xor(&s_stable)
            .bitwise_xor(&o_stable);
        let exp_nominal = a_null
            .bitwise_xor(&p_null)
            .bitwise_xor(&s_nominal)
            .bitwise_xor(&o_nominal);
        let exp_volatile = a_null
            .bitwise_xor(&p_null)
            .bitwise_xor(&s_volatile)
            .bitwise_xor(&o_volatile);

        // Each regime gets 3 copies so the bundle is well-represented
        for _ in 0..3 {
            self.experiences.push(exp_stable);
            self.experiences.push(exp_nominal);
            self.experiences.push(exp_volatile);
        }

        // Compute synthetic deltas: Δ = S_{t+1} ⊕ ρ^{13}(S_t) ⊕ A_null
        // for (stable→nominal), (nominal→volatile), (volatile→stable)
        let delta_sn = s_nominal
            .bitwise_xor(&s_stable.rotate_left(13))
            .bitwise_xor(&a_null);
        let delta_nv = s_volatile
            .bitwise_xor(&s_nominal.rotate_left(13))
            .bitwise_xor(&a_null);
        let delta_vs = s_stable
            .bitwise_xor(&s_volatile.rotate_left(13))
            .bitwise_xor(&a_null);

        // Also register the three regime states as permanent concepts
        // so the résoné network has semantic anchors from tick 0
        self.concepts.insert("SyntheticStable".to_string(), s_stable);
        self.concepts.insert("SyntheticNominal".to_string(), s_nominal);
        self.concepts.insert("SyntheticVolatile".to_string(), s_volatile);

        let delta_history = vec![delta_sn, delta_nv, delta_vs, delta_sn, delta_nv];
        (s_stable, s_nominal, s_volatile, delta_history)
    }

    pub fn generate_vector(&self) -> Hypervector {
        Hypervector::new_random()
    }

    pub fn bind(&self, v1: &Hypervector, v2: &Hypervector) -> Hypervector {
        v1.bitwise_xor(v2)
    }

    pub fn unbind(&self, v1: &Hypervector, v2: &Hypervector) -> Hypervector {
        v1.bitwise_xor(v2)
    }

    pub fn similarity(&self, v1: &Hypervector, v2: &Hypervector) -> f64 {
        1.0 - v1.normalized_hamming_distance(v2)
    }

    /// ██ UPGRADE v2.0: FPE-based variable registration ██
    /// Pre-generates FPE_RESOLUTION level vectors for the variable's range.
    pub fn register_variable(&mut self, name: &str, min_val: f64, max_val: f64) {
        let id = Hypervector::new_random();
        let level_vectors = Hypervector::generate_level_vectors(FPE_RESOLUTION);
        self.variables.insert(
            name.to_string(),
            VarConfig {
                id,
                min_val,
                max_val,
                level_vectors,
            },
        );
    }

    pub fn register_concept(&mut self, name: &str) -> Hypervector {
        let vec = Hypervector::new_random();
        self.concepts.insert(name.to_string(), vec);
        vec
    }

    /// FPE-based continuous encoding (replaces old linear interpolation)
    pub fn encode_continuous(&self, name: &str, val: f64) -> Option<Hypervector> {
        let config = self.variables.get(name)?;
        Some(Hypervector::encode_fpe(
            &config.level_vectors,
            val,
            config.min_val,
            config.max_val,
        ))
    }

    /// Encode and bind: V = id ⊕ encode(val)
    pub fn encode_and_bind_variable(&self, name: &str, val: f64) -> Option<Hypervector> {
        let config = self.variables.get(name)?;
        let val_vector = Hypervector::encode_fpe(
            &config.level_vectors,
            val,
            config.min_val,
            config.max_val,
        );
        Some(config.id.bitwise_xor(&val_vector))
    }

    pub fn bundle(&self, vectors: &[Hypervector]) -> Hypervector {
        let refs: Vec<&Hypervector> = vectors.iter().collect();
        Hypervector::bundle(&refs)
    }

    pub fn compile_state_vector(&self, telemetry: &HashMap<String, f64>) -> Hypervector {
        let mut bound_vectors = Vec::new();
        for (name, val) in telemetry {
            if let Some(bound) = self.encode_and_bind_variable(name, *val) {
                bound_vectors.push(bound);
            }
        }
        let refs: Vec<&Hypervector> = bound_vectors.iter().collect();
        Hypervector::bundle(&refs)
    }

    // ── Memory management ─────────────────────────────────────────────────

    /// ██ UPGRADE v2.2: Continuous Orthogonal Projection + Locked Anchor ██
    ///
    /// When storing `vector` into an EXISTING permanent cluster, we
    /// compute its informational divergence from the cluster's **locked
    /// anchor** (not its drifting centroid):
    ///
    /// $$C_{\text{delta}} = C_{\text{new}} \oplus C_{\text{anchor}}$$
    ///
    /// The anchor is set once at cluster creation and NEVER changes.
    /// This eliminates Reference Frame Drift — reconstructing at any
    /// future time gives exact recovery:
    ///
    /// $$C_{\text{new}} = C_{\text{delta}} \oplus C_{\text{anchor}}$$
    ///
    /// The semantic centroid continues to update dynamically (for LSH
    /// routing and matching) without corrupting stored deltas.
    ///
    /// New clusters (first entry) set both `centroid` and `anchor` to
    /// the entry vector.
    pub fn add_to_dejavu_db(
        &mut self,
        vector: Hypervector,
        label: &str,
        metadata: HashMap<String, String>,
    ) {
        let adaptive_nhd = self.adaptive_novelty_threshold();
        let cluster_threshold = 1.0 - adaptive_nhd;  // similarity = 1 - NHD
        let mut best_idx = None;
        let mut best_sim = -1.0;

        for (idx, cluster) in self.dejavu_clusters.iter().enumerate() {
            let sim = 1.0 - vector.normalized_hamming_distance(&cluster.centroid);
            if sim > best_sim {
                best_sim = sim;
                best_idx = Some(idx);
            }
        }

        if let Some(idx) = best_idx {
            if best_sim >= cluster_threshold {
                // ██ FIX v2.6 (Layer 3): Thaw from cold storage on write access ██
                if self.cold_storage.contains(idx) {
                    if let Some(data) = self.cold_storage.take(idx) {
                        if let Some(thawed) = crate::compression::deserialize_cold_cluster(&data) {
                            let cluster = &mut self.dejavu_clusters[idx];
                            cluster.entries = thawed.entries;
                            cluster.accumulator = thawed.accumulator;
                            cluster.total_weight = thawed.total_weight;
                        }
                    }
                }
                // Ensure the Locked Anchor is initialized
                let cluster = &mut self.dejavu_clusters[idx];
                cluster.ensure_anchor();

                // ██ Delta-encode against the IMMUTABLE anchor ██
                let entry = DejavuEntry::new(
                    vector,
                    label.to_string(),
                    metadata,
                    Some(&cluster.anchor),
                );
                let tau = entry.reconstruct(&cluster.anchor);
                cluster.entries.push(entry);
                if cluster.entries.len() > MAX_ENTRIES_PER_CLUSTER {
                    let drain = MAX_ENTRIES_PER_CLUSTER / 4;
                    cluster.entries.drain(0..drain);
                }

                // ██ Tier 4: Absorb into accumulator (replaces manual bundle) ██
                cluster.absorb_entry(&tau);

                cluster.reverberation = (cluster.reverberation + 0.2).min(1.0);
                cluster.last_reinforced_tick = self.tick_counter;

                // ██ UPGRADE v3.0: Record cluster activation for association learning ██
                self.record_activation(idx);

                return;
            }
        }

        // Spawn new cluster — anchor = centroid = first entry (immutable)
        let hv = vector; // rename for clarity — this IS the anchor
        let entry = DejavuEntry::new(hv, label.to_string(), metadata, None);
        // ██ Tier 4: Initialize accumulator with the first entry.
        let mut accumulator = vec![0u32; HD_DIMENSION];
        for (i, acc) in accumulator.iter_mut().enumerate() {
            let word = hv.bits[i / 64];
            let bit = (word >> (i % 64)) & 1;
            *acc = bit as u32;
        }
        let new_idx = self.dejavu_clusters.len();
        self.dejavu_clusters.push(MemoryCluster {
            centroid: hv,
            entries: vec![entry],
            reverberation: 1.0,
            last_reinforced_tick: self.tick_counter,
            anchor: hv, // Locked Anchor set at birth
            accumulator,
            total_weight: 1,
            last_access_tick: self.tick_counter as u64,
        });

        // ██ UPGRADE v3.0: Record new cluster activation for association learning ██
        self.record_activation(new_idx);
    }

    /// ██ Theorem XXIII.4: Update drift magnitude EWMA ██
    ///
    /// Called once per tick with the residual delta vector
    /// δ_t = S_t ⊕ ρ(S_{t-1}) ⊕ A_{t-1}.  Computes the per-tick drift
    /// magnitude as the normalized popcount of δ_t and updates the
    /// exponential moving average:
    ///
    ///   m_t = α · |δ_t|/D + (1-α) · m_{t-1}
    ///
    /// where α = DRIFT_MAGNITUDE_ALPHA = 0.05.
    ///
    /// See Theorem XXIII.4 and `adaptive_novelty_threshold()`.
    pub fn update_drift_magnitude(&mut self, delta_t: &Hypervector) {
        let magnitude = delta_t.count_ones() as f64 / HD_DIMENSION as f64;
        self.drift_magnitude_ewma =
            DRIFT_MAGNITUDE_ALPHA * magnitude + (1.0 - DRIFT_MAGNITUDE_ALPHA) * self.drift_magnitude_ewma;
    }

    /// ██ Theorem XXIII.3: Adaptive novelty threshold ██
    ///
    /// Returns the NHD threshold for the `add_to_dejavu_db` cluster
    /// absorption gate.  At baseline drift (δ_measured ≤ δ_max), returns
    /// THETA_MAIN_BASELINE = 0.35 NHD (0.65 similarity).  As measured
    /// drift increases, the threshold drops proportionally until it hits
    /// THETA_ADAPT_MIN = 0.32 NHD (0.68 similarity), just above the
    /// compactor merge threshold (0.30 NHD).
    ///
    /// The formula:
    ///
    ///     θ_adapt = max(THETA_ADAPT_MIN, THETA_MAIN_BASELINE · δ_max / δ_measured)
    ///
    /// In similarity space (used by `add_to_dejavu_db`):
    ///
    ///     sim_adapt = 1.0 - θ_adapt
    ///
    /// | δ_measured | θ_adapt (NHD) | sim_adapt | Effect |
    /// |------------|---------------|-----------|--------|
    /// | ≤ δ_max    | 0.35          | 0.65      | Baseline (unchanged) |
    /// | 2× δ_max   | 0.175         | 0.825     | More absorption |
    /// | 4× δ_max   | 0.32 (floor)  | 0.68      | Max absorption pressure |
    pub fn adaptive_novelty_threshold(&self) -> f64 {
        if self.drift_magnitude_ewma <= DELTA_MAX {
            THETA_MAIN_BASELINE
        } else {
            let adapted = THETA_MAIN_BASELINE * (DELTA_MAX / self.drift_magnitude_ewma);
            adapted.max(THETA_ADAPT_MIN)
        }
    }

    /// ██ Theorem XXIII.3: Compact close clusters ██
    ///
    /// Finds the closest pair of clusters by centroid NHD.  If the distance
    /// is ≤ `merge_threshold`, merges the smaller cluster into the larger
    /// one.  The survivor's anchor is preserved; the absorbed cluster's
    /// entries are re-delta-encoded against the survivor's anchor to
    /// guarantee exact reconstruction.  Accumulators are summed and the
    /// centroid is recomputed.
    ///
    /// Repeats until no pair is within the threshold.  Returns the number
    /// of clusters merged.
    ///
    /// O(K²) per call, where K = number of clusters.  Designed to be called
    /// every 50 ticks when the adaptive gate is active (δ_measured > δ_max).
    pub fn compact_clusters(&mut self, merge_threshold: f64) -> usize {
        let mut merges = 0;
        loop {
            if self.dejavu_clusters.len() < 2 {
                break;
            }

            // Find closest pair
            let mut min_dist = f64::MAX;
            let mut min_i = 0;
            let mut min_j = 1;
            for i in 0..self.dejavu_clusters.len() {
                for j in (i + 1)..self.dejavu_clusters.len() {
                    let d = self.dejavu_clusters[i].centroid.normalized_hamming_distance(
                        &self.dejavu_clusters[j].centroid,
                    );
                    if d < min_dist {
                        min_dist = d;
                        min_i = i;
                        min_j = j;
                    }
                }
            }

            if min_dist > merge_threshold {
                break;
            }

            // Ensure the larger cluster (by weight) is the survivor
            if self.dejavu_clusters[min_i].total_weight < self.dejavu_clusters[min_j].total_weight {
                std::mem::swap(&mut min_i, &mut min_j);
            }

            // Ensure both have their accumulators initialized
            self.dejavu_clusters[min_i].ensure_accumulator();
            self.dejavu_clusters[min_j].ensure_accumulator();

            // Re-encode absorbed cluster's entries against survivor's anchor
            // Copy both anchors first to avoid borrow conflicts.
            let j_anchor = self.dejavu_clusters[min_j].anchor;
            let i_anchor = self.dejavu_clusters[min_i].anchor;
            let j_entries: Vec<DejavuEntry> = self.dejavu_clusters[min_j].entries.drain(..).collect();
            for entry in j_entries {
                let reconstructed = entry.reconstruct(&j_anchor);
                let new_entry = DejavuEntry::new(
                    reconstructed,
                    entry.label,
                    entry.metadata,
                    Some(&i_anchor),
                );
                self.dejavu_clusters[min_i].entries.push(new_entry);
            }

            // Enforce entry cap on survivor (keep newest entries)
            if self.dejavu_clusters[min_i].entries.len() > MAX_ENTRIES_PER_CLUSTER {
                let drain = MAX_ENTRIES_PER_CLUSTER / 4;
                self.dejavu_clusters[min_i].entries.drain(0..drain);
            }

            // Merge accumulators
            // Copy both weights + j's accumulator first to avoid borrow conflicts.
            let survivor_total = self.dejavu_clusters[min_i].total_weight;
            let absorbed_total = self.dejavu_clusters[min_j].total_weight;
            let combined = survivor_total + absorbed_total;
            let j_acc: Vec<u32> = self.dejavu_clusters[min_j].accumulator.clone();

            for (a_i, &a_j) in self.dejavu_clusters[min_i]
                .accumulator
                .iter_mut()
                .zip(j_acc.iter())
            {
                *a_i = (*a_i as u64 + a_j as u64) as u32;
            }
            self.dejavu_clusters[min_i].total_weight = combined;

            // Rescale if above MAX_CLUSTER_WEIGHT (same logic as absorb_entry)
            if self.dejavu_clusters[min_i].total_weight > MAX_CLUSTER_WEIGHT {
                let scale = MAX_CLUSTER_WEIGHT as f64 / self.dejavu_clusters[min_i].total_weight as f64;
                // Copy centroid before mutating accumulator
                let centroid_before = self.dejavu_clusters[min_i].centroid;
                for acc in self.dejavu_clusters[min_i].accumulator.iter_mut() {
                    *acc = (*acc as f64 * scale).round() as u32;
                }
                self.dejavu_clusters[min_i].total_weight = MAX_CLUSTER_WEIGHT;
                // Preserve centroid fixed-point under rescaling
                let new_threshold = self.dejavu_clusters[min_i].total_weight / 2;
                for (i, acc) in self.dejavu_clusters[min_i].accumulator.iter_mut().enumerate() {
                    let word = centroid_before.bits[i / 64];
                    let bit_before = (word >> (i % 64)) & 1;
                    let is_above = *acc > new_threshold;
                    if bit_before == 1 && !is_above {
                        *acc = new_threshold + 1;
                    } else if bit_before == 0 && is_above {
                        *acc = new_threshold;
                    }
                }
            }

            // Recompute centroid from merged accumulator
            self.dejavu_clusters[min_i].recompute_centroid();

            // ρ-admissible check on the merged centroid (Theorem XXV.4)
            self.dejavu_clusters[min_i].enforce_rho_admissible();

            // Remove the absorbed cluster
            self.dejavu_clusters.remove(min_j);
            merges += 1;
        }
        merges
    }

    /// ██ Tier 4: Absorb an epistemic update from the broker.
    ///
    /// After an agent executes an action and the broker broadcasts the
    /// new world state, ALL agents absorb it into their private cluster
    /// accumulators.  This is **epistemic learning** (updating the model
    /// of what the world looks like).
    ///
    /// If `increment_intent_frequency` is true, the agent also increments
    /// the intent cluster's frequency — **instrumental learning** (updating
    /// the model of what actions are desirable).  Abstaining agents (via
    /// the Conscience Clause) set this to false.
    pub fn absorb_epistemic_update(
        &mut self,
        new_world_state: &Hypervector,
        _label: &str,
        increment_intent_frequency: bool,
    ) {
        // Find the nearest cluster and absorb via the accumulator
        let mut best_idx = None;
        let mut best_sim = -1.0;
        for (idx, cluster) in self.dejavu_clusters.iter().enumerate() {
            let sim = 1.0 - new_world_state.normalized_hamming_distance(&cluster.centroid);
            if sim > best_sim {
                best_sim = sim;
                best_idx = Some(idx);
            }
        }

        if let Some(idx) = best_idx {
            let adaptive_sim = 1.0 - self.adaptive_novelty_threshold();
            if best_sim >= adaptive_sim {
                // ██ FIX v2.6 (Layer 3): Thaw from cold storage on epistemic update ██
                if self.cold_storage.contains(idx) {
                    if let Some(data) = self.cold_storage.take(idx) {
                        if let Some(thawed) = crate::compression::deserialize_cold_cluster(&data) {
                            let cluster = &mut self.dejavu_clusters[idx];
                            cluster.entries = thawed.entries;
                            cluster.accumulator = thawed.accumulator;
                            cluster.total_weight = thawed.total_weight;
                        }
                    }
                }
                let cluster = &mut self.dejavu_clusters[idx];
                cluster.ensure_anchor();
                let tau = *new_world_state; // not delta-encoded
                let (centroid_shift, input_dist) = cluster.absorb_entry(&tau);
                // Record κ_F telemetry
                self.contraction_telemetry.record_kappa_f(centroid_shift, input_dist);
                if increment_intent_frequency {
                    cluster.reverberation = (cluster.reverberation + 0.1).min(1.0);
                }
                // ██ UPGRADE v3.0: Record cluster activation for association learning ██
                self.record_activation(idx);
                return;
            }
        }

        // If no close cluster, create a new one
        let mut accumulator = vec![0u32; HD_DIMENSION];
        for (i, acc) in accumulator.iter_mut().enumerate() {
            let word = new_world_state.bits[i / 64];
            let bit = (word >> (i % 64)) & 1;
            *acc = bit as u32;
        }
        self.dejavu_clusters.push(MemoryCluster {
            centroid: *new_world_state,
            anchor: *new_world_state,
            entries: Vec::new(),
            reverberation: if increment_intent_frequency { 1.0 } else { 0.5 },
            last_reinforced_tick: self.tick_counter,
            accumulator,
            total_weight: 1,
            last_access_tick: self.tick_counter as u64,
        });
    }

    pub fn collect_learned_crisis_concepts(&self) -> Vec<Hypervector> {
        let mut concepts = Vec::new();
        for cluster in &self.dejavu_clusters {
            for entry in &cluster.entries {
                if entry.metadata.get("type") == Some(&"learned_crisis_pattern".to_string()) {
                    concepts.push(cluster.centroid);
                    break;
                }
            }
        }
        concepts
    }

    /// ██ Tier 4: Hot/Cold memory management ██
    ///
    /// Freezes clusters that haven't been accessed in `staleness_threshold`
    /// ticks, dropping their accumulator (40 KB) to reclaim memory.
    /// The binary centroid is preserved, and the accumulator is lazily
    /// reconstructed on the next access via `ensure_accumulator()`.
    ///
    /// Keeps at most `max_hot` clusters hot.  If more clusters are
    /// active than the cap, the coldest among the hot set are frozen.
    ///
    /// Called periodically (e.g. every 100 ticks) by the agent loop.
    /// ██ Tier 4: Calibrate the optimal projection threshold theta* ██
    ///
    /// Finds the threshold θ that minimizes expected distortion:
    ///   ε*(θ) = mean_{entries} [
    ///     d if d ≤ (θ - ε/2)/(1-ε)
    ///     else ε
    ///   ]
    ///
    /// where d = NHD(entry, centroid) and ε = composition_noise_eps
    /// (typical composition noise without projection, ≈ 0.50 for n ≥ 2).
    ///
    /// Derived optimal (uniform distance model): θ* = (3ε - 2ε²)/2
    /// For ε = 0.50: θ* = 0.50
    ///
    /// The empirical calibration measures the true intra-cluster distance
    /// distribution and finds the minimizing θ by scanning candidates.
    ///
    /// Project a vector through the cluster manifold using the current
    /// soft projection temperature setting.
    ///
    /// When soft_projection_tau = 0 (default), this is equivalent to the
    /// hard nearest-centroid projection (anchor_through_clusters).
    /// When tau > 0, uses the weighted-majority soft projection that
    /// breaks the singular invariant measure (Theorem XXVII.1).
    pub fn project_through_clusters(&self, x: &Hypervector) -> Hypervector {
        crate::reason::soft_project(x, &self.dejavu_clusters, self.soft_projection_tau)
    }

    /// Measure empirical κ_P (projection contraction) by sampling random
    /// pairs from the cluster set and projecting them through nearest-centroid.
    ///
    /// κ_P = mean(δ(P(x), P(y)) / δ(x, y))
    ///
    /// Respects the current soft_projection_tau setting.
    /// Called periodically by the agent loop for joint contraction monitoring.
    pub fn measure_kappa_p(&mut self, n_pairs: usize) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let clusters = &self.dejavu_clusters;
        if clusters.len() < 2 {
            return;
        }
        let tau = self.soft_projection_tau;

        for _ in 0..n_pairs {
            let x = Hypervector::new_random();
            let y = Hypervector::new_random();
            let d_before = x.normalized_hamming_distance(&y);

            let px = crate::reason::soft_project(&x, clusters, tau);
            let py = crate::reason::soft_project(&y, clusters, tau);
            let d_after = px.normalized_hamming_distance(&py);

            self.contraction_telemetry.record_kappa_p(d_before, d_after);
        }
    }

    /// Returns the calibrated threshold (NHD, not similarity).
    pub fn calibrate_projection_threshold(&self, composition_noise_eps: f64) -> f64 {
        if self.dejavu_clusters.is_empty() {
            return DEFAULT_PROJECTION_THRESHOLD_NHD;
        }

        // Collect all intra-cluster distances (entry-to-centroid)
        let mut distances: Vec<f64> = Vec::new();
        for cluster in &self.dejavu_clusters {
            let centroid = &cluster.centroid;
            for entry in &cluster.entries {
                let d = entry.reconstruct(&cluster.anchor)
                    .normalized_hamming_distance(centroid);
                distances.push(d);
            }
        }

        if distances.is_empty() {
            return DEFAULT_PROJECTION_THRESHOLD_NHD;
        }

        // Scan candidate thresholds to find the minimum-distortion θ
        let eps = composition_noise_eps;
        let mut best_theta = DEFAULT_PROJECTION_THRESHOLD_NHD;
        let mut best_error = f64::MAX;

        // Scan from 0.10 to 0.80 in 100 steps
        for step in 0..=100 {
            let θ = 0.10 + (step as f64) * 0.007; // step ≈ 0.007
            let d_crit = (θ - eps / 2.0) / (1.0 - eps);

            if d_crit <= 0.0 {
                // Everything rejected: error = ε
                let error = eps;
                if error < best_error {
                    best_error = error;
                    best_theta = θ;
                }
                continue;
            }

            let mut total_error = 0.0_f64;

            for &d in &distances {
                if d <= d_crit {
                    total_error += d;
                } else {
                    total_error += eps;
                }
            }

            let mean_error = total_error / distances.len() as f64;
            if mean_error < best_error {
                best_error = mean_error;
                best_theta = θ;
            }
        }

        best_theta
    }

    pub fn freeze_cold_clusters(&mut self, current_tick: u64, staleness_threshold: u64, max_hot: usize) {
        let hot_count = self.dejavu_clusters.iter().filter(|c| c.is_hot()).count();
        if hot_count <= max_hot {
            // Under the cap — only freeze clusters past the staleness threshold
            for (idx, cluster) in self.dejavu_clusters.iter_mut().enumerate() {
                if cluster.is_hot()
                    && current_tick.saturating_sub(cluster.last_access_tick) > staleness_threshold
                {
                    // ██ FIX v2.6 (Layer 3): serialize to cold storage before freezing ██
                    let serialized = crate::compression::serialize_cold_cluster(cluster);
                    cluster.entries.clear();
                    cluster.entries.shrink_to_fit();
                    cluster.accumulator.clear();
                    self.cold_storage.store(idx, serialized);
                }
            }
        } else {
            // Over the cap — sort by access tick and freeze the coldest
            let mut indices: Vec<usize> = (0..self.dejavu_clusters.len()).collect();
            indices.sort_by_key(|&i| self.dejavu_clusters[i].last_access_tick);
            for &i in &indices[..indices.len().saturating_sub(max_hot)] {
                if self.dejavu_clusters[i].is_hot() {
                    let serialized = crate::compression::serialize_cold_cluster(&self.dejavu_clusters[i]);
                    self.dejavu_clusters[i].entries.clear();
                    self.dejavu_clusters[i].entries.shrink_to_fit();
                    self.dejavu_clusters[i].accumulator.clear();
                    self.cold_storage.store(i, serialized);
                }
            }
        }
    }

    pub fn decay_permanent_clusters(&mut self, lambda: f64, theta_retain: f64) {
        for cluster in self.dejavu_clusters.iter_mut() {
            cluster.reverberation *= lambda;
        }
        let now = self.tick_counter;
        self.dejavu_clusters.retain(|c| {
            c.reverberation >= theta_retain
                || now.saturating_sub(c.last_reinforced_tick) <= 50
        });
    }

    pub fn add_transient_fact(
        &mut self,
        vector: Hypervector,
        label: &str,
        metadata: HashMap<String, String>,
    ) {
        // Transient entries are never delta-encoded (they're short-lived)
        let entry = DejavuEntry::new(vector, label.to_string(), metadata, None);

        let cluster_threshold = 1.0 - self.adaptive_novelty_threshold();
        let mut best_idx = None;
        let mut best_sim = -1.0;

        for (idx, cluster) in self.transient_clusters.iter().enumerate() {
            let sim = 1.0 - vector.normalized_hamming_distance(&cluster.centroid);
            if sim > best_sim {
                best_sim = sim;
                best_idx = Some(idx);
            }
        }

        if let Some(idx) = best_idx {
            if best_sim >= cluster_threshold {
                // ██ FIX v2.6: Thaw frozen cluster and update access tick ██
                let cluster = &mut self.transient_clusters[idx];
                cluster.frozen = false;
                cluster.last_access_tick = self.tick_counter as u64;
                cluster.entries.push(entry);
                cluster.last_reinforced_tick = self.tick_counter;
                cluster.reverberation += best_sim;
                let refs: Vec<&Hypervector> = cluster
                    .entries
                    .iter()
                    .map(|e| &e.vector)
                    .collect();
                cluster.centroid = Hypervector::bundle(&refs);
                return;
            }
        }

        self.transient_clusters.push(TransientCluster {
            centroid: vector,
            entries: vec![entry],
            reverberation: 1.0,
            last_reinforced_tick: self.tick_counter,
            anchor: Hypervector::new_zero(),
            last_access_tick: self.tick_counter as u64,
            frozen: false,
        });
    }

    /// ██ FIX v2.6: Freeze cold transient clusters ██
    ///
    /// Transient clusters whose entries are accumulating without recent
    /// access have their entry vectors dropped (frozen).  The centroid
    /// is preserved so they can still match queries.  On the next
    /// `add_transient_fact` that matches, the cluster is automatically
    /// thawed.
    ///
    /// This mirrors the hot/cold management of `dejavu_clusters` but
    /// is simpler: instead of accumulators, we drop the entries Vec
    /// (which is the main memory cost for transients).
    pub fn freeze_cold_transient_clusters(&mut self, current_tick: u64, staleness_threshold: u64) {
        for cluster in &mut self.transient_clusters {
            if !cluster.frozen
                && current_tick.saturating_sub(cluster.last_access_tick) > staleness_threshold
            {
                cluster.frozen = true;
                cluster.entries.clear();
                cluster.entries.shrink_to_fit();
            }
        }
    }

    /// ██ FIX v2.6: Combined freeze-cold + decay for transient clusters ██
    /// Called from the agent subconscious loop.  Freezes cold transients,
    /// then runs the regular decay/consolidation on unfrozen clusters.
    ///
    /// Use this in place of calling `freeze_cold_transient_clusters`
    /// and `decay_transient_clusters_distributed` separately.
    pub fn freeze_and_decay_transients(
        &mut self,
        current_tick: u64,
        staleness_threshold: u64,
        lambda: f64,
        theta_resonance: f64,
        theta_coherence: f64,
    ) -> Vec<(Hypervector, Vec<DejavuEntry>)> {
        // Step 1: Freeze cold transient clusters
        self.freeze_cold_transient_clusters(current_tick, staleness_threshold);

        // Step 3: Run the standard decay
        self.decay_transient_clusters_distributed(lambda, theta_resonance, theta_coherence)
    }

    /// UPDATED: U64_BLOCKS = 160, HD_DIMENSION = 10240
    pub fn decay_transient_clusters(
        &mut self,
        lambda: f64,
        theta_resonance: f64,
        theta_coherence: f64,
    ) {
        self.tick_counter = self.tick_counter.wrapping_add(1);

        for cluster in self.transient_clusters.iter_mut() {
            cluster.reverberation *= lambda;
        }

        let mut consolidated_indices = Vec::new();
        for (idx, cluster) in self.transient_clusters.iter().enumerate() {
            if cluster.reverberation > theta_resonance {
                let num_entries = cluster.entries.len();
                if num_entries == 0 {
                    consolidated_indices.push(idx);
                    continue;
                }

                let mut unanimity_count = 0;
                for block_idx in 0..U64_BLOCKS {
                    let mut bit_agreement = [0u32; 64];
                    for entry in &cluster.entries {
                        for bit_idx in 0..64 {
                            if ((entry.vector.bits[block_idx] >> bit_idx) & 1) == 1 {
                                bit_agreement[bit_idx] += 1;
                            }
                        }
                    }
                    for bit_idx in 0..64 {
                        let ones = bit_agreement[bit_idx] as f64 / num_entries as f64;
                        if ones > 0.80 || ones < 0.20 {
                            unanimity_count += 1;
                        }
                    }
                }

                let unanimity_ratio = unanimity_count as f64 / HD_DIMENSION as f64;

                if unanimity_ratio > theta_coherence {
                    let (best_label, sim, _) = self.query_dejavu(&cluster.centroid);

                    if sim >= 0.75 {
                        if let Some(best_lbl) = best_label {
                            if let Some(p_idx) = self
                                .dejavu_clusters
                                .iter()
                                .enumerate()
                                .find(|(_, pc)| {
                                    pc.entries.iter().any(|e| e.label == best_lbl)
                                        || pc.entries.first()
                                            .map(|fe| fe.label.clone())
                                            .unwrap_or_default() == best_lbl
                                })
                                .map(|(i, _)| i)
                            {
                                // Ensure Locked Anchor is initialized
                                let anchor = self.dejavu_clusters[p_idx].anchor;
                                // ██ Tier 4: Absorb each transient entry into the
                                // permanent cluster's accumulator.
                                for entry in &cluster.entries {
                                    let tau = entry.reconstruct(&anchor);
                                    self.dejavu_clusters[p_idx].entries.push(entry.clone());
                                    self.dejavu_clusters[p_idx].absorb_entry(&tau);
                                }
                            } else {
                                // ██ Tier 4: Initialize accumulator from transient centroid ██
                                let mut accumulator = vec![0u32; HD_DIMENSION];
                                for (i, acc) in accumulator.iter_mut().enumerate() {
                                    let word = cluster.centroid.bits[i / 64];
                                    let bit = (word >> (i % 64)) & 1;
                                    *acc = bit as u32;
                                }
                                self.dejavu_clusters.push(MemoryCluster {
                                    centroid: cluster.centroid,
                                    anchor: cluster.centroid,
                                    entries: cluster.entries.clone(),
                                    reverberation: cluster.reverberation,
                                    last_reinforced_tick: self.tick_counter,
                                    accumulator,
                                    total_weight: cluster.entries.len().max(1) as u32,
                                    last_access_tick: self.tick_counter as u64,
                                });
                            }
                        } else {
                            let mut accumulator = vec![0u32; HD_DIMENSION];
                            for (i, acc) in accumulator.iter_mut().enumerate() {
                                let word = cluster.centroid.bits[i / 64];
                                let bit = (word >> (i % 64)) & 1;
                                *acc = bit as u32;
                            }
                            self.dejavu_clusters.push(MemoryCluster {
                                centroid: cluster.centroid,
                                anchor: cluster.centroid,
                                entries: cluster.entries.clone(),
                                reverberation: cluster.reverberation,
                                last_reinforced_tick: self.tick_counter,
                                accumulator,
                                total_weight: cluster.entries.len().max(1) as u32,
                                last_access_tick: self.tick_counter as u64,
                            });
                        }
                    } else if sim >= 0.52 {
                        let mut accumulator = vec![0u32; HD_DIMENSION];
                        for (i, acc) in accumulator.iter_mut().enumerate() {
                            let word = cluster.centroid.bits[i / 64];
                            let bit = (word >> (i % 64)) & 1;
                            *acc = bit as u32;
                        }
                        self.dejavu_clusters.push(MemoryCluster {
                            centroid: cluster.centroid,
                            anchor: cluster.centroid,
                            entries: cluster.entries.clone(),
                            reverberation: cluster.reverberation,
                            last_reinforced_tick: self.tick_counter,
                            accumulator,
                            total_weight: cluster.entries.len().max(1) as u32,
                            last_access_tick: self.tick_counter as u64,
                        });
                    }
                }

                consolidated_indices.push(idx);
            }
        }

        for (idx, cluster) in self.transient_clusters.iter().enumerate() {
            if consolidated_indices.contains(&idx) { continue; }
            if cluster.reverberation < 0.05
                || self.tick_counter.saturating_sub(cluster.last_reinforced_tick) > 50
            {
                consolidated_indices.push(idx);
            }
        }

        consolidated_indices.sort_unstable_by(|a, b| b.cmp(a));
        for idx in consolidated_indices {
            if idx < self.transient_clusters.len() {
                self.transient_clusters.remove(idx);
            }
        }

        let sum_reverberation: f64 = self.transient_clusters
            .iter()
            .map(|c| c.reverberation)
            .sum();
        let normalized_sum = sum_reverberation / theta_resonance;
        self.anxiety = (0.2 * normalized_sum).tanh();
    }

    /// UPDATED: U64_BLOCKS = 160, HD_DIMENSION = 10240
    pub fn decay_transient_clusters_distributed(
        &mut self,
        lambda: f64,
        theta_resonance: f64,
        theta_coherence: f64,
    ) -> Vec<(Hypervector, Vec<DejavuEntry>)> {
        self.tick_counter = self.tick_counter.wrapping_add(1);
        let mut consolidated = Vec::new();

        for cluster in self.transient_clusters.iter_mut() {
            cluster.reverberation *= lambda;
        }

        let mut consolidated_indices = Vec::new();
        for (idx, cluster) in self.transient_clusters.iter().enumerate() {
            if cluster.reverberation > theta_resonance {
                let num_entries = cluster.entries.len();
                if num_entries == 0 {
                    consolidated_indices.push(idx);
                    continue;
                }

                let mut unanimity_count = 0;
                for block_idx in 0..U64_BLOCKS {
                    let mut bit_agreement = [0u32; 64];
                    for entry in &cluster.entries {
                        for bit_idx in 0..64 {
                            if ((entry.vector.bits[block_idx] >> bit_idx) & 1) == 1 {
                                bit_agreement[bit_idx] += 1;
                            }
                        }
                    }
                    for bit_idx in 0..64 {
                        let ones = bit_agreement[bit_idx] as f64 / num_entries as f64;
                        if ones > 0.80 || ones < 0.20 {
                            unanimity_count += 1;
                        }
                    }
                }

                let unanimity_ratio = unanimity_count as f64 / HD_DIMENSION as f64;

                if unanimity_ratio > theta_coherence {
                    consolidated.push((cluster.centroid, cluster.entries.clone()));
                }
                consolidated_indices.push(idx);
            }
        }

        for (idx, cluster) in self.transient_clusters.iter().enumerate() {
            if consolidated_indices.contains(&idx) { continue; }
            if cluster.reverberation < 0.05
                || self.tick_counter.saturating_sub(cluster.last_reinforced_tick) > 50
            {
                consolidated_indices.push(idx);
            }
        }

        consolidated_indices.sort_unstable_by(|a, b| b.cmp(a));
        for idx in consolidated_indices {
            if idx < self.transient_clusters.len() {
                self.transient_clusters.remove(idx);
            }
        }

        let sum_reverberation: f64 = self.transient_clusters
            .iter()
            .map(|c| c.reverberation)
            .sum();
        let normalized_sum = sum_reverberation / theta_resonance;
        self.anxiety = (0.2 * normalized_sum).tanh();

        consolidated
    }

    /// ██ UPGRADE v2.0 + Tier 4: LSH-indexed query with 10-bit sectors ██
    ///
    /// Divides memory into 1024 sectors by a 10-bit locality-sensitive
    /// hash.  Phase 1 searches only clusters whose LOCKED ANCHOR falls
    /// in the query's sector.  Phase 2 falls back to full scan if the
    /// sector-local result is below threshold.  Phase 3 always scans
    /// transient clusters (full scan — they're small).
    ///
    /// Uses `anchor` for delta-encoded reconstruction (immutable
    /// reference frame), NOT `centroid` (which drifts).
    pub fn query_dejavu(
        &self,
        vector: &Hypervector,
    ) -> (Option<String>, f64, HashMap<String, String>) {
        if self.dejavu_clusters.is_empty() && self.transient_clusters.is_empty() {
            return (None, 0.0, HashMap::new());
        }

        let query_sector = lsh_sector_inline(vector);

        let mut best_label: Option<String> = None;
        let mut best_sim: f64 = -1.0;
        let mut best_meta: HashMap<String, String> = HashMap::new();

        // Inline search helper that handles delta-encoded reconstruction
        macro_rules! search_clusters {
            ($clusters:expr, $reconstruct:expr) => {
                for cluster in $clusters {
                    let ref_v = if $reconstruct {
                        let is_zero = cluster.anchor.bits.iter().all(|&b| b == 0);
                        if !is_zero { &cluster.anchor } else { &cluster.centroid }
                    } else {
                        &cluster.centroid
                    };
                    for entry in &cluster.entries {
                        let compare_v = if $reconstruct && entry.delta_encoded {
                            entry.vector.bitwise_xor(ref_v)
                        } else {
                            entry.vector
                        };
                        let entry_sim = 1.0 - vector.normalized_hamming_distance(&compare_v);
                        if entry_sim > best_sim {
                            best_sim = entry_sim;
                            best_label = Some(entry.label.clone());
                            best_meta = entry.metadata.clone();
                        }
                    }
                }
            };
        }

        // Phase 1: Search only clusters in the query's LSH sector.
        // Sector is determined by the cluster's LOCKED ANCHOR, not its
        // index position (which was a bug in the 4-bit version).
        for cluster in self.dejavu_clusters.iter() {
            let cluster_sector = if cluster.anchor.count_ones() > 0 {
                lsh_sector_inline(&cluster.anchor)
            } else {
                lsh_sector_inline(&cluster.centroid)
            };
            if cluster_sector != query_sector {
                continue;
            }
            let ref_v = {
                let is_zero = cluster.anchor.bits.iter().all(|&b| b == 0);
                if !is_zero { &cluster.anchor } else { &cluster.centroid }
            };
            for entry in &cluster.entries {
                let compare_v = if entry.delta_encoded {
                    entry.vector.bitwise_xor(ref_v)
                } else {
                    entry.vector
                };
                let entry_sim = 1.0 - vector.normalized_hamming_distance(&compare_v);
                if entry_sim > best_sim {
                    best_sim = entry_sim;
                    best_label = Some(entry.label.clone());
                    best_meta = entry.metadata.clone();
                }
            }
        }

        // Phase 2: If no good match in sector, fall back to full scan
        if best_sim < 0.55 {
            search_clusters!(&self.dejavu_clusters, true);
        }

        // Phase 3: Always check transient (working memory) — full scan.
        search_clusters!(&self.transient_clusters, false);

        (best_label, best_sim, best_meta)
    }

    /// ██ Tier 4: LSH-indexed evaluate_dejá-vù (mirrors query_dejavu) ██
    ///
    /// Uses the same 10-bit LSH sector routing as query_dejavu:
    /// Phase 1 searches only clusters in the query's sector (by anchor hash).
    /// Phase 2 falls back to full scan.  Phase 3 always scans transients.
    pub fn evaluate_deja_vu(&self, vector: &Hypervector) -> (Option<String>, f64) {
        if self.dejavu_clusters.is_empty() && self.transient_clusters.is_empty() {
            return (None, 1.0);
        }

        let query_sector = lsh_sector_inline(vector);

        let mut best_label = None;
        let mut min_dist = 1.0;

        macro_rules! eval_entry {
            ($entry:expr, $cluster:expr, $reconstruct:expr) => {
                let ref_v = if $reconstruct {
                    let is_zero = $cluster.anchor.bits.iter().all(|&b| b == 0);
                    if !is_zero { &$cluster.anchor } else { &$cluster.centroid }
                } else {
                    &$cluster.centroid
                };
                let compare_v = if $reconstruct && $entry.delta_encoded {
                    $entry.vector.bitwise_xor(ref_v)
                } else {
                    $entry.vector
                };
                let dist = vector.normalized_hamming_distance(&compare_v);
                if dist < min_dist {
                    min_dist = dist;
                    best_label = Some($entry.label.clone());
                }
            };
        }

        // Phase 1: Search only clusters in the query's LSH sector
        for cluster in self.dejavu_clusters.iter() {
            let cluster_sector = if cluster.anchor.count_ones() > 0 {
                lsh_sector_inline(&cluster.anchor)
            } else {
                lsh_sector_inline(&cluster.centroid)
            };
            if cluster_sector != query_sector {
                continue;
            }
            for entry in &cluster.entries {
                eval_entry!(entry, cluster, true);
            }
        }

        // Phase 2: Fallback full scan if sector miss
        if min_dist > 0.55 {
            for cluster in &self.dejavu_clusters {
                for entry in &cluster.entries {
                    eval_entry!(entry, cluster, true);
                }
            }
        }

        // Phase 3: Transient full scan (never delta-encoded)
        for cluster in &self.transient_clusters {
            for entry in &cluster.entries {
                eval_entry!(entry, cluster, false);
            }
        }

        if min_dist <= self.threshold {
            (best_label, min_dist)
        } else {
            (None, min_dist)
        }
    }

    /// UPDATED: Uses FPE encoding for continuous variable decoding
    pub fn decode_variable(
        &self,
        state_vector: &Hypervector,
        var_name: &str,
        resolution: usize,
    ) -> Option<f64> {
        let config = self.variables.get(var_name)?;
        let unbound = state_vector.bitwise_xor(&config.id);

        let mut best_val = config.min_val;
        let mut max_sim = -1.0;

        // Sample `resolution` candidate values and find the best match
        for step in 0..=resolution {
            let fraction = (step as f64) / (resolution as f64);
            let val = config.min_val + fraction * (config.max_val - config.min_val);
            let encoded = Hypervector::encode_fpe(
                &config.level_vectors, val, config.min_val, config.max_val,
            );

            let sim = 1.0 - unbound.normalized_hamming_distance(&encoded);
            if sim > max_sim {
                max_sim = sim;
                best_val = val;
            }
        }
        Some(best_val)
    }

    /// ██ Tier 3: Append a composed rule to the cluster whose centroid
    /// best matches the given antecedent vector.
    ///
    /// This is the **Hebbian storage** step: the composed consequent
    /// C is appended as a new entry to the existing cluster for A.
    /// The centroid rebundles, shifting it slightly toward C, so that
    /// future LSH queries from state A fall more directly into this
    /// cluster — geometrically equivalent to strengthening a synaptic
    /// pathway.
    ///
    /// The entry starts at a **medium-warm** reverberation (0.3) —
    /// above the decay floor so it survives 10+ ticks, but below the
    /// normal reinforcement level so a one-off shortcut self-prunes
    /// naturally.  If the shortcut is genuinely useful, normal cluster
    /// rebundling on subsequent `add_to_dejavu_db` calls will
    /// reinforce it.
    ///
    /// Returns `true` if a matching cluster was found and the entry
    /// was appended.
    pub fn append_composed_rule(
        &mut self,
        antecedent_label: &str,
        consequent: &Hypervector,
    ) -> bool {
        let ante_hv = Hypervector::encode_sentence(antecedent_label);
        for cluster in &mut self.dejavu_clusters {
            let sim = 1.0 - ante_hv.normalized_hamming_distance(&cluster.centroid);
            if sim >= 0.65 {
                let mut meta = std::collections::HashMap::new();
                meta.insert("type".to_string(), "composed_rule".to_string());
                meta.insert("antecedent".to_string(), antecedent_label.to_string());
                let entry = DejavuEntry::new(
                    *consequent,
                    format!("composed_{}", antecedent_label),
                    meta,
                    None, // raw, not delta-encoded
                );
                cluster.entries.push(entry);
                // ██ Tier 4: Absorb into accumulator (replaces manual bundle) ██
                cluster.absorb_entry(consequent);
                cluster.last_reinforced_tick = self.tick_counter;
                // Medium-warm reverberation
                cluster.reverberation = (cluster.reverberation + 0.3).min(1.0);
                return true;
            }
        }
        false
    }
}

// ─── Cross-Cluster Association Constants ──────────────────────────────────

/// ██ UPGRADE v3.0: Association window size in ticks ██
/// If two clusters are activated within this many ticks of each other,
/// their co-occurrence is recorded as an association.
pub const ASSOCIATION_WINDOW_TICKS: u64 = 5;

/// ██ UPGRADE v3.0: Maximum associations per cluster ██
/// Prevents unbounded growth of the association graph.
/// When exceeded, the weakest association is pruned.
pub const MAX_ASSOCIATIONS_PER_CLUSTER: usize = 20;

/// ██ UPGRADE v3.0: Association strength increment per co-occurrence ██
pub const ASSOCIATION_STRENGTH_INC: f64 = 0.15;

/// ██ UPGRADE v3.0: Association decay factor (per maintenance call) ██
///
/// Applied every 50 ticks in the maintenance block.  Effective per-tick
/// decay: 0.995^{1/50} ≈ 0.9999 (negligible).  The 50-tick half-life is:
///
///     t_{1/2} = 50 · ln(0.5) / ln(0.995) ≈ 6,915 ticks ≈ 3.8 hours
///
/// A single co-occurrence (strength 0.15) decays below the pruning
/// floor (ASSOCIATION_MIN_STRENGTH = 0.05) in approximately:
///
///     n = 50 · ln(0.05/0.15) / ln(0.995) ≈ 5,500 ticks ≈ 3 hours
///
/// This is long enough for Level 2 association traversal to remain
/// reliable across a full trading session, but slow enough that stale
/// associations from previous sessions eventually fade.
pub const ASSOCIATION_DECAY: f64 = 0.995;

/// ██ UPGRADE v3.0: Minimum association strength for retrieval ██
/// Associations below this strength are pruned.
pub const ASSOCIATION_MIN_STRENGTH: f64 = 0.05;

/// ██ UPGRADE v3.0: Association cascade depth limit ██
/// How many hops to follow when activating through associations.
pub const ASSOCIATION_CASCADE_DEPTH: usize = 3;

/// ██ UPGRADE v3.0: Association retrieval similarity threshold ██
/// When retrieving an associated cluster's centroid, how close the
/// reconstructed vector must be to an actual centroid to count.
pub const ASSOCIATION_MATCH_THRESHOLD: f64 = 0.65;

/// ██ UPGRADE v3.0: Minimum association strength for concept resolution ██
/// Higher than ASSOCIATION_MIN_STRENGTH (0.05 pruning floor).
/// An association must reach this strength (≈2 co-occurrences at 0.15/inc)
/// before it's trusted for resolving term coreference in the QA engine.
pub const ASSOCIATION_RESOLUTION_THRESHOLD: f64 = 0.30;

// ─── VSABrain: Cross-Cluster Association Methods ─────────────────────────

impl VSABrain {
    /// ██ UPGRADE v3.0: Record a cluster activation at the current tick.
    ///
    /// Call this when a cluster is accessed (read or write). The system uses
    /// this to track co-occurrence patterns for cross-cluster learning.
    ///
    /// `cluster_idx` — index into `self.dejavu_clusters`.
    pub fn record_activation(&mut self, cluster_idx: usize) {
        let tick = self.tick_counter as u64;
        self.activation_history
            .entry(tick)
            .or_insert_with(Vec::new)
            .push(cluster_idx);

        // Prune old activation history
        let cutoff = tick.saturating_sub(ASSOCIATION_WINDOW_TICKS * 2);
        self.activation_history.retain(|&t, _| t >= cutoff);

        // Check for co-occurrences within the window
        let window_start = tick.saturating_sub(ASSOCIATION_WINDOW_TICKS);
        let mut co_occurring = Vec::new();
        for (&hist_tick, indices) in &self.activation_history {
            if hist_tick >= window_start && hist_tick < tick {
                for &idx in indices {
                    if idx != cluster_idx && !co_occurring.contains(&idx) {
                        co_occurring.push(idx);
                    }
                }
            }
        }

        // Record associations
        for &other_idx in &co_occurring {
            self.record_association(cluster_idx, other_idx);
        }
    }

    /// ██ UPGRADE v3.0: Record a bidirectional association between two clusters.
    ///
    /// The association vector is: assoc = centroid_i ⊕ centroid_j
    /// This allows one-way retrieval: given centroid_i, recover centroid_j
    /// via centroid_j_est = centroid_i ⊕ assoc_{ij}.
    fn record_association(&mut self, idx_a: usize, idx_b: usize) {
        if idx_a >= self.dejavu_clusters.len() || idx_b >= self.dejavu_clusters.len() {
            return;
        }

        let tick = self.tick_counter as u64;

        // Association vector: centroid_a ⊕ centroid_b
        let assoc_vec = self.dejavu_clusters[idx_a]
            .centroid
            .bitwise_xor(&self.dejavu_clusters[idx_b].centroid);

        // Record A → B
        self.add_or_strengthen_association(idx_a, idx_b, assoc_vec, tick);
        // Record B → A (same vector, symmetric)
        self.add_or_strengthen_association(idx_b, idx_a, assoc_vec, tick);
    }

    /// Helper: add a new association or strengthen an existing one.
    fn add_or_strengthen_association(
        &mut self,
        from: usize,
        to: usize,
        assoc_vec: Hypervector,
        tick: u64,
    ) {
        let entry = self.cross_cluster_associations.entry(from).or_insert_with(Vec::new);

        // Look for existing association to this target
        if let Some(existing) = entry.iter_mut().find(|(t, _, _, _)| *t == to) {
            // Strengthen existing association
            existing.2 = (existing.2 + ASSOCIATION_STRENGTH_INC).min(1.0);
            existing.3 = tick;
        } else {
            // Add new association
            entry.push((to, assoc_vec, ASSOCIATION_STRENGTH_INC, tick));
        }

        // Prune if over capacity
        if entry.len() > MAX_ASSOCIATIONS_PER_CLUSTER {
            entry.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
            entry.remove(0);
        }
    }

    /// ██ UPGRADE v3.0: Retrieve associated clusters by activating through
    /// the association graph.
    ///
    /// Given a cluster index, returns the centroids of associated clusters
    /// that can be reconstructed with confidence above the threshold.
    ///
    /// Returns `Vec<(associated_cluster_index, similarity, strength)>`.
    /// Retrieve associated clusters from the association graph.
    ///
    /// `min_strength`: optional minimum association strength filter.
    ///   - `None` = return all valid associations (existing tests pass)
    ///   - `Some(threshold)` = only return associations with strength ≥ threshold
    ///     (used by QA engine's `resolve_term` for concept resolution)
    pub fn retrieve_associated(
        &self,
        cluster_idx: usize,
        min_strength: Option<f64>,
    ) -> Vec<(usize, f64, f64)> {
        if cluster_idx >= self.dejavu_clusters.len() {
            return vec![];
        }

        let centroid = &self.dejavu_clusters[cluster_idx].centroid;
        let mut results = Vec::new();

        if let Some(assocs) = self.cross_cluster_associations.get(&cluster_idx) {
            for &(target_idx, ref assoc_vec, strength, _) in assocs {
                if target_idx >= self.dejavu_clusters.len() {
                    continue;
                }

                // Optional strength gate for concept resolution
                if let Some(min_s) = min_strength {
                    if strength < min_s {
                        continue;
                    }
                }

                // Reconstruct target centroid: centroid_j_est = centroid_i ⊕ assoc_{ij}
                let reconstructed = centroid.bitwise_xor(assoc_vec);

                // Check against actual centroid
                let actual = &self.dejavu_clusters[target_idx].centroid;
                let sim = 1.0 - reconstructed.normalized_hamming_distance(actual);

                if sim >= ASSOCIATION_MATCH_THRESHOLD {
                    results.push((target_idx, sim, strength));
                }
            }
        }

        // Sort by similarity descending
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    /// ██ UPGRADE v3.0: Cascade activation through the association graph.
    ///
    /// Starting from `seed_indices`, follows association links for up to
    /// `depth` hops, returning all reachable clusters with their similarity
    /// scores and the number of hops traversed.
    ///
    /// This enables "spreading activation" — when one concept fires, related
    /// concepts get a priming boost.
    pub fn cascade_activation(
        &self,
        seed_indices: &[usize],
        depth: usize,
    ) -> Vec<(usize, f64, usize)> {
        let depth = depth.min(ASSOCIATION_CASCADE_DEPTH);
        let mut visited = std::collections::HashSet::new();
        let mut frontier: Vec<(usize, usize)> = seed_indices.iter().map(|&i| (i, 0)).collect();
        let mut results: Vec<(usize, f64, usize)> = Vec::new();

        while let Some((current, hop)) = frontier.pop() {
            if !visited.insert(current) {
                continue;
            }

            // Record reachable cluster
            if hop > 0 {
                if let Some(cached) = results.iter_mut().find(|(idx, _, _)| *idx == current) {
                    cached.2 = cached.2.min(hop);
                } else {
                    results.push((current, 0.0, hop));
                }
            }

            if hop >= depth {
                continue;
            }

            // Follow outgoing associations
            if let Some(assocs) = self.cross_cluster_associations.get(&current) {
                for &(target, _, strength, _) in assocs {
                    if strength >= ASSOCIATION_MIN_STRENGTH && !visited.contains(&target) {
                        frontier.push((target, hop + 1));
                    }
                }
            }
        }

        // Compute similarity for each reachable cluster
        // The similarity is the average NHD to all seed centroids
        let seed_centroids: Vec<&Hypervector> = seed_indices.iter()
            .filter(|&&i| i < self.dejavu_clusters.len())
            .map(|&i| &self.dejavu_clusters[i].centroid)
            .collect();

        for (idx, sim, _) in results.iter_mut() {
            if *idx < self.dejavu_clusters.len() && !seed_indices.contains(idx) {
                let centroid = &self.dejavu_clusters[*idx].centroid;
                if seed_centroids.is_empty() {
                    *sim = 0.5;
                } else {
                    let total_sim: f64 = seed_centroids.iter()
                        .map(|sc| 1.0 - sc.normalized_hamming_distance(centroid))
                        .sum();
                    *sim = total_sim / seed_centroids.len() as f64;
                }
            }
        }

        results.sort_by(|a, b| a.2.cmp(&b.2).then(b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)));
        results
    }

    /// ██ UPGRADE v3.0: Decay association strengths.
    /// Call this periodically (e.g., every 10 ticks) to gradually weaken
    /// unused associations.
    pub fn decay_associations(&mut self) {
        for (_, assocs) in self.cross_cluster_associations.iter_mut() {
            assocs.retain(|(_, _, strength, _)| *strength >= ASSOCIATION_MIN_STRENGTH);
            for (_, _, strength, _) in assocs.iter_mut() {
                *strength *= ASSOCIATION_DECAY;
            }
        }
        // Remove empty entries
        self.cross_cluster_associations.retain(|_, v| !v.is_empty());
    }

    /// ██ UPGRADE v3.0: Get all associations for a cluster (for debugging / HUD).
    pub fn get_associations(&self, cluster_idx: usize) -> Vec<(usize, f64)> {
        self.cross_cluster_associations
            .get(&cluster_idx)
            .map(|assocs| {
                assocs.iter().map(|(t, _, s, _)| (*t, *s)).collect()
            })
            .unwrap_or_default()
    }

    // ═══════════════════════════════════════════════════════════════════════
    // CROSS-SESSION PERSISTENCE
    // ═══════════════════════════════════════════════════════════════════════
    //
    // Saves and loads the brain state that must survive restarts:
    //   1. Cluster centroids (MemoryCluster struct with all fields)
    //   2. Cross-cluster associations
    //
    // NOT serialized:
    //   - Accumulators (reconstructed via ensure_accumulator() on access)
    //   - Activation history (ephemeral — rebuilt from streaming data)
    //   - Experiences buffer (ephemeral — rebuilt from streaming data)
    //   - Tick counter (resets on reload — tick-based decay is ambiguous
    //     after a clock gap; associations use strength-only decay going forward)
    //   - Drift magnitude EWMA (converges within ~42 ticks of streaming data)
    //
    // On reload, associations with out-of-bounds cluster indices are
    // silently dropped. The tick field in associations is also dropped
    // (set to 0) to avoid ambiguity about the clock gap.

    /// Save the brain state to a JSON file.
    ///
    /// Only persists cluster centroids (with anchors and entries) and
    /// cross-cluster associations. All ephemeral state is excluded.
    ///
    /// Returns the number of clusters and associations saved.
    pub fn save_to_file(&self, path: &str) -> Result<(usize, usize), String> {
        // Strip tick field from associations — it's meaningless after reload
        let associations: Vec<(usize, usize, Hypervector, f64)> = self
            .cross_cluster_associations
            .iter()
            .flat_map(|(from, assocs)| {
                assocs.iter().map(move |(to, vec, strength, _tick)| {
                    (*from, *to, *vec, *strength)
                })
            })
            .collect();

        let snapshot = BrainSnapshot {
            cluster_count: self.dejavu_clusters.len(),
            clusters: self.dejavu_clusters.clone(),
            associations,
            soft_projection_tau: self.soft_projection_tau,
        };

        let json = serde_json::to_string_pretty(&snapshot)
            .map_err(|e| format!("Serialization error: {}", e))?;
        std::fs::write(path, &json)
            .map_err(|e| format!("Write error: {}", e))?;

        Ok((snapshot.clusters.len(), snapshot.associations.len()))
    }

    /// Load the brain state from a JSON file.
    ///
    /// Replaces the current brain state with the deserialized clusters and
    /// associations. Accumulators are left empty (cold) — they will be
    /// lazily reconstructed via `ensure_accumulator()` on first access.
    ///
    /// Associations with out-of-bounds cluster indices (which can occur
    /// if cluster indices shifted during a prior session's compaction)
    /// are silently dropped. This is safe because:
    ///   - Level 2 resolution falls through to Level 3 (raw n-gram) if
    ///     no valid association is found.
    ///   - New associations are learned as the system processes data.
    pub fn load_from_file(&mut self, path: &str) -> Result<(usize, usize), String> {
        let json = std::fs::read_to_string(path)
            .map_err(|e| format!("Read error: {}", e))?;
        let snapshot: BrainSnapshot = serde_json::from_str(&json)
            .map_err(|e| format!("Deserialization error: {}", e))?;

        // Validate cluster count
        if snapshot.clusters.len() != snapshot.cluster_count {
            return Err(format!(
                "Corrupt snapshot: declared {} clusters but found {}",
                snapshot.cluster_count,
                snapshot.clusters.len()
            ));
        }

        // Restore clusters (accumulators are empty — cold state)
        self.dejavu_clusters = snapshot.clusters;
        for cluster in self.dejavu_clusters.iter_mut() {
            // Ensure accumulator is empty — will be lazily reconstructed
            cluster.accumulator = Vec::new();
            // Reset last_access_tick — will be updated on first access
            cluster.last_access_tick = 0;
        }

        // Restore associations with bounds validation
        self.cross_cluster_associations.clear();
        let mut valid_count = 0;
        let mut dropped_count = 0;
        for (from, to, vec, strength) in snapshot.associations {
            if from < self.dejavu_clusters.len() && to < self.dejavu_clusters.len() {
                self.cross_cluster_associations
                    .entry(from)
                    .or_insert_with(Vec::new)
                    .push((to, vec, strength, 0)); // tick = 0 (no clock)
                valid_count += 1;
            } else {
                dropped_count += 1;
            }
        }

        // Restore soft projection tau (or leave default 0.0)
        self.soft_projection_tau = snapshot.soft_projection_tau;

        // Reset ephemeral state
        self.tick_counter = 0;
        self.drift_magnitude_ewma = 0.0;
        self.activation_history.clear();
        self.experiences.clear();

        Ok((self.dejavu_clusters.len(), valid_count))
    }
}

/// Serializable snapshot of the brain for cross-session persistence.
///
/// Contains only the state that must survive restarts:
/// - Cluster centroids (with anchors, entries, weights)
/// - Cross-cluster associations (tick field stripped)
/// - Soft projection tau parameter
///
/// Accumulators are intentionally excluded — they are lazily reconstructed
/// via `MemoryCluster::ensure_accumulator()` on first use after deserialization.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BrainSnapshot {
    /// Number of clusters — used for index validation on reload.
    pub cluster_count: usize,
    /// Serialized MemoryClusters (centroids, anchors, entries, weights).
    pub clusters: Vec<MemoryCluster>,
    /// Associations as (from_idx, to_idx, vector, strength) — tick field
    /// is intentionally stripped; strength-only decay on reload.
    pub associations: Vec<(usize, usize, Hypervector, f64)>,
    /// The soft projection tau parameter, if non-default.
    pub soft_projection_tau: f64,
}

// ─── LSH sector hash (stable random projections) ───────────────────────────

/// ██ UPGRADE v2.1: Stable random projection LSH ██
///
/// Replaces the raw-prefix hash (which was vulnerable to LCG-induced
/// clustering skew from the character encoder).  Each of the 4 bits
/// is derived from a popcount parity of a XOR between two fixed,
/// widely-separated u64 blocks.  This ensures:
///
/// 1. **Distribution uniformity** — popcount parity over 10240 bits
///    is unbiased regardless of LCG regularity in the first blocks.
/// 2. **Stability** — small perturbations in the vector flip ≈1 bit
///    per sector indicator on average (locality-sensitive).
/// 3. **Determinism** — same vector always maps to the same sector.
///
/// Reference implementation uses indices {1,50}, {2,100}, {3,150}, {4,75}.
/// LSH sector count: 10 bits → 1024 sectors.
/// This matches the theoretical bound M = 1024 used in the
/// boundedness proof (Theorem III.1).
pub const LSH_SECTOR_COUNT: usize = 1024;

pub(crate) fn lsh_sector_inline(vector: &Hypervector) -> usize {
    // 10 independent bit-parity projections from well-separated
    // u64 block pairs.  Each pair is at least 20 blocks apart
    // to minimize correlation between hash bits.
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

// ─── Tests ─────────────────────────────────────────────────────────────────

// ═══════════════════════════════════════════════════════════════════════════
// JOINT CONTRACTION TELEMETRY (Theorem XXII.1-R runtime monitoring)
// ═══════════════════════════════════════════════════════════════════════════
//
// Tracks empirical κ_P (projection contraction) and κ_F (manifold contraction)
// to ensure the joint product κ = κ_P · κ_F stays below 1.0.
// The theoretical margin is 0.010 at L_F = 1.0 (worst case).
//
// κ_P measurement:
//   Samples pairs of (pre-projection, post-projection) states from the
//   anchor_through_clusters pipeline. For each pair (x, y):
//     κ_P_sample = δ(P(x), P(y)) / δ(x, y)
//   κ_P = rolling mean of samples.
//
// κ_F measurement:
//   Per cluster absorption:
//     centroid_shift = δ(centroid_before, centroid_after)
//     input_distance = δ(centroid_before, input)
//     κ_F_sample = 1.0 - centroid_shift / max(input_distance, 1e-10)
//   κ_F = rolling mean of samples.
//
// Tripwire:
//   If κ = κ_P · κ_F ≥ 0.995: log WARNING (approaching instability)
//   If κ ≥ 1.001: log CRITICAL (structural divergence detected)

#[derive(Clone, Debug)]
pub struct ContractionTelemetry {
    // κ_P samples (projection contraction)
    pub kappa_p_samples: Vec<f64>,
    pub kappa_p_mean: f64,
    pub kappa_p_count: u64,

    // κ_F samples (manifold contraction)
    pub kappa_f_samples: Vec<f64>,
    pub kappa_f_mean: f64,
    pub kappa_f_count: u64,

    // Joint product tracking
    pub kappa_joint: f64,
    pub kappa_joint_max: f64,

    // Tripwire state
    pub tripwire_triggered: bool,
    pub last_tripwire_tick: u64,

    // Configuration
    pub max_samples: usize,          // rolling window size
    pub tripwire_threshold: f64,     // default 0.995
    pub critical_threshold: f64,     // default 1.001
}

impl ContractionTelemetry {
    pub fn new() -> Self {
        ContractionTelemetry {
            kappa_p_samples: Vec::with_capacity(1000),
            kappa_p_mean: 0.0,
            kappa_p_count: 0,
            kappa_f_samples: Vec::with_capacity(1000),
            kappa_f_mean: 0.0,
            kappa_f_count: 0,
            kappa_joint: 0.0,
            kappa_joint_max: 0.0,
            tripwire_triggered: false,
            last_tripwire_tick: 0,
            max_samples: 1000,
            tripwire_threshold: 0.995,
            critical_threshold: 1.001,
        }
    }

    /// Record a κ_P sample from a projection event.
    /// `d_before` = δ(x, y) before projection, `d_after` = δ(P(x), P(y)) after.
    pub fn record_kappa_p(&mut self, d_before: f64, d_after: f64) {
        if d_before < 1e-10 {
            return; // skip degenerate pairs
        }
        let sample = (d_after / d_before).min(2.0).max(0.0);
        self.kappa_p_samples.push(sample);
        if self.kappa_p_samples.len() > self.max_samples {
            self.kappa_p_samples.remove(0);
        }
        self.kappa_p_mean = self.kappa_p_samples.iter().sum::<f64>()
            / self.kappa_p_samples.len() as f64;
        self.kappa_p_count += 1;
        self.update_joint();
    }

    /// Record a κ_F sample from an absorption event.
    /// `centroid_shift` = δ(c_before, c_after), `input_dist` = δ(c_before, input).
    pub fn record_kappa_f(&mut self, centroid_shift: f64, input_dist: f64) {
        let denom = input_dist.max(1e-10);
        // κ_F_sample = 1 - shift/input_dist → fraction of input NOT absorbed
        let sample = (1.0 - centroid_shift / denom).min(2.0).max(-1.0);
        self.kappa_f_samples.push(sample);
        if self.kappa_f_samples.len() > self.max_samples {
            self.kappa_f_samples.remove(0);
        }
        self.kappa_f_mean = self.kappa_f_samples.iter().sum::<f64>()
            / self.kappa_f_samples.len() as f64;
        self.kappa_f_count += 1;
        self.update_joint();
    }

    fn update_joint(&mut self) {
        self.kappa_joint = self.kappa_p_mean * self.kappa_f_mean;
        if self.kappa_joint > self.kappa_joint_max {
            self.kappa_joint_max = self.kappa_joint;
        }
    }

    /// Check the tripwire. Returns a diagnostic string if breached, None otherwise.
    pub fn check_tripwire(&mut self, tick: u64) -> Option<String> {
        if self.kappa_p_count < 10 || self.kappa_f_count < 10 {
            return None; // not enough data
        }

        if self.kappa_joint >= self.critical_threshold && !self.tripwire_triggered {
            self.tripwire_triggered = true;
            self.last_tripwire_tick = tick;
            return Some(format!(
                "CRITICAL: Joint contraction κ = {:.6} ≥ {:.3}! \
                 (κ_P={:.4}, κ_F={:.4}, samples: P={}, F={}) \
                 System may be structurally diverging!",
                self.kappa_joint, self.critical_threshold,
                self.kappa_p_mean, self.kappa_f_mean,
                self.kappa_p_count, self.kappa_f_count,
            ));
        }

        if self.kappa_joint >= self.tripwire_threshold {
            return Some(format!(
                "WARNING: Joint contraction κ = {:.6} approaching threshold {:.3}. \
                 (κ_P={:.4}, κ_F={:.4})",
                self.kappa_joint, self.tripwire_threshold,
                self.kappa_p_mean, self.kappa_f_mean,
            ));
        }

        None
    }

    /// Generate a summary report string.
    pub fn report(&self) -> String {
        format!(
            "κ_P={:.4} (n={}), κ_F={:.4} (n={}), κ={:.6}, κ_max={:.6}",
            self.kappa_p_mean, self.kappa_p_count,
            self.kappa_f_mean, self.kappa_f_count,
            self.kappa_joint, self.kappa_joint_max,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hamming_distance_ident() {
        let v1 = Hypervector::new_random();
        assert_eq!(v1.normalized_hamming_distance(&v1), 0.0);
        let v2 = Hypervector::new_random();
        let dist = v1.normalized_hamming_distance(&v2);
        assert!(
            dist > 0.40 && dist < 0.60,
            "Hamming distance of random vectors should be around 0.5, got {}",
            dist
        );
    }

    #[test]
    fn test_char_encoder() {
        let v1 = Hypervector::encode_char('a', 42);
        let v2 = Hypervector::encode_char('a', 42);
        let v3 = Hypervector::encode_char('b', 42);
        assert_eq!(v1, v2);
        assert_ne!(v1, v3);
    }

    #[test]
    fn test_ledger_serialization() {
        let v1 = Hypervector::new_random();
        let bytes = v1.to_bytes();
        let v2 = Hypervector::from_bytes(&bytes);
        for i in 0..U64_BLOCKS {
            assert_eq!(v1.bits[i], v2.bits[i]);
        }
    }

    #[test]
    fn test_fpe_encoding() {
        // FPE: distance should be monotonic with value difference
        let levels = Hypervector::generate_level_vectors(128);
        let v_min = Hypervector::encode_fpe(&levels, -3.0, -3.0, 3.0);
        let v_max = Hypervector::encode_fpe(&levels, 3.0, -3.0, 3.0);
        let v_mid = Hypervector::encode_fpe(&levels, 0.0, -3.0, 3.0);

        let d_min_max = v_min.normalized_hamming_distance(&v_max);
        let d_min_mid = v_min.normalized_hamming_distance(&v_mid);
        let d_mid_max = v_mid.normalized_hamming_distance(&v_max);

        // Distance should scale monotonically with value difference
        assert!(d_min_mid < d_min_max, "FPE: d_min_mid={} should be < d_min_max={}", d_min_mid, d_min_max);
        assert!(d_mid_max < d_min_max, "FPE: d_mid_max={} should be < d_min_max={}", d_mid_max, d_min_max);
    }

    #[test]
    fn test_encode_sentence() {
        let s1 = Hypervector::encode_sentence("the hacker breached the server");
        let s2 = Hypervector::encode_sentence("the hacker breached the server");
        let s3 = Hypervector::encode_sentence("the server breached the hacker");

        assert_eq!(s1, s2);
        assert_ne!(s1, s3);

        let dist = s1.normalized_hamming_distance(&s3);
        assert!(
            dist > 0.20,
            "Different word order should have high distance, got {}",
            dist
        );
    }

    #[test]
    fn test_hierarchical_clustering() {
        let mut brain = VSABrain::new(0.43);
        let mut meta = HashMap::new();
        meta.insert("test".to_string(), "1".to_string());

        for i in 0..10 {
            let v = Hypervector::new_random();
            brain.add_to_dejavu_db(v, &format!("label_{}", i), meta.clone());
        }

        assert!(!brain.dejavu_clusters.is_empty());

        let query_vec = Hypervector::new_random();
        let (label, sim, _) = brain.query_dejavu(&query_vec);
        println!("Best query match: {:?}, sim: {}", label, sim);
    }

    #[test]
    fn test_transient_consolidation() {
        let mut brain = VSABrain::new(0.43);
        let mut meta = HashMap::new();
        meta.insert("test".to_string(), "1".to_string());

        let fact_vec = Hypervector::new_random();
        for i in 0..6 {
            brain.add_transient_fact(fact_vec, &format!("persistent_fact_{}", i), meta.clone());
        }

        assert_eq!(brain.transient_clusters.len(), 1);
        assert!(brain.transient_clusters[0].reverberation > 3.0);

        brain.decay_transient_clusters(0.95, 3.0, 0.10);

        assert_eq!(brain.transient_clusters.len(), 0);
        assert!(!brain.dejavu_clusters.is_empty());
        assert_eq!(brain.dejavu_clusters[0].entries.len(), 6);
    }

    #[test]
    fn test_bundle_weighted_recency() {
        // Weighted bundle should be closer to the higher-weighted vector
        let v1 = Hypervector::encode_text_ngram("apple", 3);
        let v2 = Hypervector::encode_text_ngram("banana", 3);

        // Weight v2 heavily
        let result = Hypervector::bundle_weighted(&[&v1, &v2], &[0.1, 10.0]);
        let sim_v1 = 1.0 - result.normalized_hamming_distance(&v1);
        let sim_v2 = 1.0 - result.normalized_hamming_distance(&v2);
        assert!(
            sim_v2 > sim_v1,
            "Weighted bundle should favour v2: sim_v1={}, sim_v2={}",
            sim_v1,
            sim_v2
        );
    }

    #[test]
    fn test_fpe_level_count() {
        let levels = Hypervector::generate_level_vectors(FPE_RESOLUTION);
        assert_eq!(levels.len(), FPE_RESOLUTION);
        // Adjacent levels should be more similar than far-apart levels
        let d_adj = levels[0].normalized_hamming_distance(&levels[1]);
        let d_far = levels[0].normalized_hamming_distance(&levels[127]);
        assert!(
            d_adj < d_far,
            "Adjacent FPE levels should be closer: adj={}, far={}",
            d_adj,
            d_far
        );
    }

    #[test]
    fn test_calibrate_projection_threshold() {
        let mut brain = VSABrain::new(0.43);

        // Create clusters with known intra-cluster distances.
        // Each cluster gets entries at controlled NHD from its centroid.
        let add_cluster_with_noise = |brain: &mut VSABrain, noise: f64| {
            let centroid = Hypervector::new_random();
            let mut meta = std::collections::HashMap::new();
            meta.insert("type".to_string(), "test".to_string());
            // First entry (creates the cluster)
            brain.add_to_dejavu_db(centroid, "test_centroid", std::collections::HashMap::new());
            // Add 10 more entries with controlled noise
            for i in 0..10 {
                let mut noisy = centroid;
                for _ in 0..(noise * 10240.0) as usize {
                    let block = rand::random::<usize>() % 160;
                    let bit = rand::random::<usize>() % 64;
                    noisy.bits[block] ^= 1u64 << bit;
                }
                brain.add_to_dejavu_db(noisy, &format!("entry_{}", i), meta.clone());
            }
        };

        // Two clusters with intra-noise 0.10, one with 0.30
        add_cluster_with_noise(&mut brain, 0.10);
        add_cluster_with_noise(&mut brain, 0.10);
        add_cluster_with_noise(&mut brain, 0.30);

        // Calibrate at various composition noise levels
        for eps in [0.30, 0.50, 0.70] {
            let θ = brain.calibrate_projection_threshold(eps);
            let θ_sim = 1.0 - θ;
            eprintln!(
                "  ε = {:.2}: θ*_NHD = {:.4} (sim ≥ {:.4})",
                eps, θ, θ_sim
            );
            // θ should be in a reasonable range [0.10, 0.70]
            assert!(
                θ >= 0.10 && θ <= 0.70,
                "Calibrated threshold out of range: {}",
                θ
            );
        }

        // Empty brain → falls back to default
        let empty_brain = VSABrain::new(0.43);
        let default_θ = empty_brain.calibrate_projection_threshold(0.50);
        assert!(
            (default_θ - 0.50).abs() < 0.01,
            "Empty brain should return default threshold: {}",
            default_θ
        );
    }

    // ── UPGRADE v3.0: Cross-Cluster Association Tests ───────────────────

    #[test]
    fn test_record_and_retrieve_association() {
        let mut brain = VSABrain::new(0.43);

        // Create two clusters using random vectors that are guaranteed distinct
        // We bypass add_to_dejavu_db to ensure they're in separate clusters
        let mut meta = HashMap::new();
        meta.insert("test".to_string(), "1".to_string());

        // Directly push two clusters with random centroids
        let v1 = Hypervector::new_random();
        let v2 = Hypervector::new_random();
        // Guarantee they're different by XOR-ing with distinct patterns
        let v1 = v1.bitwise_xor(&Hypervector::encode_text_ngram("CLUSTER_A", 3));
        let v2 = v2.bitwise_xor(&Hypervector::encode_text_ngram("CLUSTER_B", 3));

        brain.add_to_dejavu_db(v1, "alpha", HashMap::new());
        brain.add_to_dejavu_db(v2, "beta", HashMap::new());

        assert_eq!(brain.dejavu_clusters.len(), 2, "Should have 2 clusters");

        // Record co-occurrence within the same tick  
        brain.tick_counter = 1;
        brain.record_activation(0);
        brain.record_activation(1);

        // Retrieve associations for cluster 0
        let assocs = brain.get_associations(0);
        assert!(!assocs.is_empty(), "Cluster 0 should have associations, got 0");
        let (target, strength) = assocs[0];
        assert_eq!(target, 1, "Cluster 0 should be associated with cluster 1");
        assert!(
            strength >= ASSOCIATION_STRENGTH_INC * 0.5,
            "Association strength should be positive: {}",
            strength
        );
    }

    #[test]
    fn test_cascade_activation() {
        let mut brain = VSABrain::new(0.43);

        // Create 4 clusters with distinct random vectors
        for i in 0..4 {
            let v = Hypervector::new_random()
                .bitwise_xor(&Hypervector::encode_text_ngram(&format!("CLUSTER_{}", i), 3));
            brain.add_to_dejavu_db(v, &format!("c{}", i), HashMap::new());
        }

        assert!(brain.dejavu_clusters.len() >= 3, "Should have at least 3 clusters");

        // Record associations between consecutive clusters
        for &(a, b) in &[(0usize, 1usize), (1, 2)] {
            if a < brain.dejavu_clusters.len() && b < brain.dejavu_clusters.len() {
                brain.tick_counter = (b + 1) as usize;
                brain.record_activation(a);
                brain.record_activation(b);
            }
        }

        // Cascade from cluster 0
        let cascade = brain.cascade_activation(&[0], 3);
        assert!(!cascade.is_empty(), "Cascade should reach other clusters");
    }

    #[test]
    fn test_association_decay() {
        let mut brain = VSABrain::new(0.43);

        let v1 = Hypervector::new_random()
            .bitwise_xor(&Hypervector::encode_text_ngram("DECAY_A", 3));
        let v2 = Hypervector::new_random()
            .bitwise_xor(&Hypervector::encode_text_ngram("DECAY_B", 3));
        brain.add_to_dejavu_db(v1, "a", HashMap::new());
        brain.add_to_dejavu_db(v2, "b", HashMap::new());

        // Create association
        brain.tick_counter = 1;
        brain.record_activation(0);
        brain.record_activation(1);

        let strength_before = brain.get_associations(0).first().map(|(_, s)| *s).unwrap_or(0.0);
        assert!(strength_before > 0.0, "Association should have positive strength");

        // Decay many times
        for _ in 0..5000 {
            brain.decay_associations();
        }

        let strength_after = brain.get_associations(0).first().map(|(_, s)| *s).unwrap_or(0.0);
        assert!(
            strength_after < strength_before || strength_after.abs() < 1e-10,
            "Association should weaken with decay: before={}, after={}",
            strength_before, strength_after
        );
    }

    #[test]
    fn test_activation_history_pruning() {
        let mut brain = VSABrain::new(0.43);

        // Record activations at various ticks
        brain.tick_counter = 1;
        brain.record_activation(0);
        brain.tick_counter = 100;
        brain.record_activation(1);

        // Old activations should be pruned by the record_activation method
        // when window_start exceeds the old history
        assert!(
            brain.activation_history.len() <= 2,
            "Activation history should be pruned: {} entries",
            brain.activation_history.len()
        );
    }

    /// Verify the association decay half-life matches the documented value.
    ///
    /// A single co-occurrence yields strength ≈ ASSOCIATION_STRENGTH_INC
    /// (0.15).  After N = ln(0.05/0.15) / ln(0.995) ≈ 219 calls, it should
    /// fall below ASSOCIATION_MIN_STRENGTH (0.05) and be pruned.
    ///
    /// The "half-life" (to drop to 0.075) occurs after approx 138 calls.
    #[test]
    fn test_association_decay_half_life() {
        let mut brain = VSABrain::new(0.43);

        let v1 = Hypervector::new_random()
            .bitwise_xor(&Hypervector::encode_text_ngram("HL_A", 3));
        let v2 = Hypervector::new_random()
            .bitwise_xor(&Hypervector::encode_text_ngram("HL_B", 3));
        brain.add_to_dejavu_db(v1, "a", HashMap::new());
        brain.add_to_dejavu_db(v2, "b", HashMap::new());

        // Create a single co-occurrence (strength = 0.15)
        brain.tick_counter = 1;
        brain.record_activation(0);
        brain.record_activation(1);

        let initial = brain.get_associations(0).first().map(|(_, s)| *s).unwrap_or(0.0);
        eprintln!("\n  Association Decay Half-Life Verification:");
        eprintln!("  Initial strength: {:.4}", initial);

        // Half-life: 0.995^n = 0.5 → n = ln(0.5)/ln(0.995) ≈ 138
        let half_life_calls = (0.5_f64.ln() / ASSOCIATION_DECAY.ln()).ceil() as usize;
        eprintln!("  Theoretical half-life: {} calls", half_life_calls);

        for _ in 0..half_life_calls {
            brain.decay_associations();
        }

        let after_hl = brain.get_associations(0).first().map(|(_, s)| *s).unwrap_or(0.0);
        eprintln!("  After {} calls: {:.4} (expected ~{:.4})",
            half_life_calls, after_hl, initial * 0.5);
        assert!(
            (after_hl - initial * 0.5).abs() < 0.02,
            "Half-life should reduce strength by ~50%: start={:.4}, after {} calls={:.4}",
            initial, half_life_calls, after_hl
        );

        // After enough decays, the association should be pruned
        // (fall below ASSOCIATION_MIN_STRENGTH = 0.05)
        let prune_calls = ((ASSOCIATION_MIN_STRENGTH / initial).ln() / ASSOCIATION_DECAY.ln()).ceil() as usize;
        for _ in 0..(prune_calls - half_life_calls) {
            brain.decay_associations();
        }

        let pruned = brain.get_associations(0).first().copied();
        eprintln!("  After {} calls (pruning threshold): {:?} (expected < 0.05 or gone)",
            prune_calls, pruned);
        assert!(
            pruned.is_none() || pruned.unwrap().1 < ASSOCIATION_MIN_STRENGTH,
            "Single-co-occurrence association should be pruned after {} decays",
            prune_calls
        );
        eprintln!("  ✓ Half-life matches theoretical value ({} calls)", half_life_calls);
        eprintln!("  ✓ Pruning occurs at expected threshold");
    }

    /// Verify the ρ-admissible invariant:
    ///   δ(c_k, ρ¹³(c_k)) > 0 for all centroids.
    ///
    /// Since gcd(13, 10240) = 1, the cyclic shift by 13 generates the full
    /// group C_10240. The only fixed points of ρ¹³ on {0,1}^10240 are the
    /// constant vectors (all zeros or all ones). These are detected and
    /// perturbed by flipping a single bit.
    ///
    /// Non-constant centroids pass through unchanged.
    /// Required for Assumption ρ in Theorem XXV.4.
    #[test]
    fn test_rho_admissible_invariant() {
        // Helper to construct a MemoryCluster with a given centroid
        let make_cluster = |centroid: Hypervector| -> MemoryCluster {
            MemoryCluster {
                centroid,
                entries: Vec::new(),
                reverberation: 0.0,
                last_reinforced_tick: 0,
                anchor: Hypervector::new_zero(),
                accumulator: Vec::new(),
                total_weight: 500,
                last_access_tick: 0,
            }
        };

        // 1. All-zeros centroid — a trivial fixed point of ρ¹³.
        let mut cluster = make_cluster(Hypervector::new_zero());

        let rotated_before = cluster.centroid.rotate_left(13);
        let dist_before = cluster.centroid.normalized_hamming_distance(&rotated_before);
        eprintln!("  All-zeros distance to ρ¹³(self) before: {:.6}", dist_before);
        assert_eq!(dist_before, 0.0, "All-zeros should be a fixed point of ρ¹³");

        cluster.enforce_rho_admissible();

        let rotated_after = cluster.centroid.rotate_left(13);
        let dist_after = cluster.centroid.normalized_hamming_distance(&rotated_after);
        eprintln!("  All-zeros distance to ρ¹³(self) after:  {:.6}", dist_after);
        assert!(
            dist_after > 0.0,
            "After enforcement, centroid should NOT be a fixed point"
        );
        // Flipping 1 bit in an otherwise-constant vector creates exactly 2
        // differing bits between c and ρ¹³(c): the flipped position and the
        // position it rotates to. So δ = 2/D.
        assert!(
            0.0 < dist_after && dist_after <= (2.0 / 10240.0 + 1e-10),
            "Perturbation of constant vector should yield δ ≤ 2/D, got {}",
            dist_after
        );
        eprintln!("  ✓ All-zeros perturbed (δ(c,ρ¹³(c)) = {:.6})", dist_after);

        // 2. All-ones centroid — also a fixed point.
        let mut ones_cluster = make_cluster(Hypervector::new_ones());
        let rotated_ones_before = ones_cluster.centroid.rotate_left(13);
        let dist_ones_before = ones_cluster.centroid.normalized_hamming_distance(&rotated_ones_before);
        assert_eq!(dist_ones_before, 0.0, "All-ones should be a fixed point of ρ¹³");

        ones_cluster.enforce_rho_admissible();
        let rotated_ones_after = ones_cluster.centroid.rotate_left(13);
        let dist_ones_after = ones_cluster.centroid.normalized_hamming_distance(&rotated_ones_after);
        assert!(dist_ones_after > 0.0, "All-ones should be perturbed");
        eprintln!("  ✓ All-ones perturbed");

        // 3. Normal (random, non-constant) centroid — passes through unchanged.
        let original = Hypervector::new_random();
        let mut normal_cluster = make_cluster(original);
        let before = normal_cluster.centroid;
        normal_cluster.enforce_rho_admissible();
        assert_eq!(
            before, normal_cluster.centroid,
            "Non-constant centroid should be unchanged"
        );
        eprintln!("  ✓ Non-constant centroid unchanged");

        // 4. Accumulator consistency (with resident accumulator).
        let mut acc_cluster = MemoryCluster {
            centroid: Hypervector::new_zero(),
            entries: Vec::new(),
            reverberation: 0.0,
            last_reinforced_tick: 0,
            anchor: Hypervector::new_zero(),
            accumulator: vec![250u32; 10240],
            total_weight: 500,
            last_access_tick: 0,
        };
        acc_cluster.enforce_rho_admissible();
        // After enforcement, centroid bit 0 should match accumulator[0] > threshold
        let expected_bit = acc_cluster.centroid.bits[0] & 1;
        let threshold = acc_cluster.total_weight / 2;
        let acc_implies_one = acc_cluster.accumulator[0] > threshold;
        assert_eq!(
            expected_bit, acc_implies_one as u64,
            "Accumulator should be consistent with flipped centroid bit 0"
        );
        eprintln!("  ✓ Accumulator consistency verified");

        // 5. Period-2 centroid — fixed point of ρ²⁶, NOT of ρ¹³.
        // Must be caught by the ρ²⁶ check and perturbed.
        let period2_bits = {
            let mut bits = [0u64; 160];
            for i in 0..160 {
                let mut word = 0u64;
                for bit in 0..64 {
                    let pos = i * 64 + bit;
                    if pos % 2 == 0 {
                        word |= 1u64 << bit;
                    }
                }
                bits[i] = word;
            }
            bits
        };
        let mut p2_cluster = make_cluster(Hypervector { bits: period2_bits });

        // Verify: δ(c, ρ¹³(c)) = 1.0 (passes ρ¹³ check)
        let r13_before = p2_cluster.centroid.rotate_left(13);
        let d_r13 = p2_cluster.centroid.normalized_hamming_distance(&r13_before);
        eprintln!("  Period-2 δ(c, ρ¹³(c)) before: {:.4}", d_r13);
        assert!(
            d_r13 > 0.99,
            "Period-2 centroid should NOT be a ρ¹³ fixed point (δ={})",
            d_r13
        );

        // Verify: δ(c, ρ²⁶(c)) = 0.0 (FAILS ρ²⁶ check)
        let r26_before = p2_cluster.centroid.rotate_left(26);
        let d_r26 = p2_cluster.centroid.normalized_hamming_distance(&r26_before);
        eprintln!("  Period-2 δ(c, ρ²⁶(c)) before: {:.6}", d_r26);
        assert_eq!(d_r26, 0.0, "Period-2 centroid MUST be a ρ²⁶ fixed point");

        // Enforce ρ-admissible invariant
        p2_cluster.enforce_rho_admissible();

        // Both ρ¹³ and ρ²⁶ should now have δ > 0
        let r13_after = p2_cluster.centroid.rotate_left(13);
        let d_r13_after = p2_cluster.centroid.normalized_hamming_distance(&r13_after);
        let r26_after = p2_cluster.centroid.rotate_left(26);
        let d_r26_after = p2_cluster.centroid.normalized_hamming_distance(&r26_after);
        eprintln!("  Period-2 δ(c, ρ¹³(c)) after:  {:.6}", d_r13_after);
        eprintln!("  Period-2 δ(c, ρ²⁶(c)) after:  {:.6}", d_r26_after);
        assert!(
            d_r13_after > 0.0,
            "After enforcement, ρ¹³ should also have δ > 0 (bit flip may affect both)"
        );
        assert!(
            d_r26_after > 0.0,
            "After enforcement, ρ²⁶ must have δ > 0"
        );
        eprintln!("  ✓ Period-2 centroid perturbed");

        // 6. Period-4 centroid — fixed point of ρ⁵², NOT of ρ¹³ or ρ²⁶.
        // Must be caught by the ρ⁵² check and perturbed.
        let period4_bits = {
            let mut bits = [0u64; 160];
            for i in 0..160 {
                let mut word = 0u64;
                for bit in 0..64 {
                    let pos = i * 64 + bit;
                    // 0011 repeating pattern
                    if pos % 4 == 0 || pos % 4 == 1 {
                        word |= 1u64 << bit;
                    }
                }
                bits[i] = word;
            }
            bits
        };
        let mut p4_cluster = make_cluster(Hypervector { bits: period4_bits });

        // Verify it passes ρ¹³ and ρ²⁶ checks
        let r13_p4 = p4_cluster.centroid.rotate_left(13);
        let d_p4_r13 = p4_cluster.centroid.normalized_hamming_distance(&r13_p4);
        eprintln!("  Period-4 δ(c, ρ¹³(c)): {:.4}", d_p4_r13);
        assert!(d_p4_r13 > 0.01, "Period-4 centroid should pass ρ¹³ check");

        let r26_p4 = p4_cluster.centroid.rotate_left(26);
        let d_p4_r26 = p4_cluster.centroid.normalized_hamming_distance(&r26_p4);
        eprintln!("  Period-4 δ(c, ρ²⁶(c)): {:.4}", d_p4_r26);
        assert!(d_p4_r26 > 0.01, "Period-4 centroid should pass ρ²⁶ check");

        // Verify it FAILS ρ⁵² check
        let r52_p4 = p4_cluster.centroid.rotate_left(52);
        let d_p4_r52 = p4_cluster.centroid.normalized_hamming_distance(&r52_p4);
        eprintln!("  Period-4 δ(c, ρ⁵²(c)) before: {:.6}", d_p4_r52);
        assert_eq!(d_p4_r52, 0.0, "Period-4 centroid MUST be a ρ⁵² fixed point");

        // Enforce
        p4_cluster.enforce_rho_admissible();

        let r52_p4_after = p4_cluster.centroid.rotate_left(52);
        let d_p4_r52_after = p4_cluster.centroid.normalized_hamming_distance(&r52_p4_after);
        eprintln!("  Period-4 δ(c, ρ⁵²(c)) after:  {:.6}", d_p4_r52_after);
        assert!(
            d_p4_r52_after > 0.0,
            "After enforcement, ρ⁵² must have δ > 0"
        );
        eprintln!("  ✓ Period-4 centroid perturbed");

        eprintln!("  All ρ-admissible invariant checks pass.");
    }

    #[test]
    fn test_ix1_grounding_preservation() {
        // Theorem IX.1: An abstaining agent maintains geometric grounding
        // in shared reality even as its causal reasoning diverges.
        // The accumulator update (absorb_entry) is unconditional;
        // only the cluster.reverberation increment is conditional on
        // increment_intent_frequency.
        let mut brain = VSABrain::new(0.43);

        // Helper: create a mask that flips systematic bits to simulate drift
        fn perturb(hv: &Hypervector, n_flips: usize, seed: usize) -> Hypervector {
            let mut mask_bits = [0u64; U64_BLOCKS];
            for f in 0..n_flips {
                let bit_pos = ((seed * 37 + f * 101) as usize) % HD_DIMENSION;
                let block = bit_pos / 64;
                let bit = bit_pos % 64;
                mask_bits[block] ^= 1u64 << bit;
            }
            let mask = Hypervector { bits: mask_bits };
            hv.bitwise_xor(&mask)
        }

        // Initial cluster seeded with a world state, but with low reverberation
        // so we can test that abstention doesn't increase it.
        let world_0 = Hypervector::new_random()
            .bitwise_xor(&Hypervector::encode_text_ngram("GROUND_TRUTH", 3));
        brain.add_to_dejavu_db(world_0, "truth", HashMap::new());
        assert_eq!(brain.dejavu_clusters.len(), 1);
        // Reset reverberation to a low value to allow testing of increment behavior
        brain.dejavu_clusters[0].reverberation = 0.1;

        let initial_centroid = brain.dejavu_clusters[0].centroid;
        let initial_reverb = brain.dejavu_clusters[0].reverberation;
        let initial_weight = brain.dejavu_clusters[0].total_weight;

        // Send many epistemic updates with abstention flag (no intent frequency increment).
        // The centroid MUST still track reality via the unconditional accumulator update,
        // but reverberation must NOT increase.
        let n_abstain = 200;    // enough to move centroid significantly
        let mut last_world = initial_centroid;
        for i in 0..n_abstain {
            // Gradual drift: i/10 bits flipped per step, growing to 20 bits
            let n_flips = ((i / 10) + 1).min(20);
            let world_state = perturb(&last_world, n_flips, i);
            brain.absorb_epistemic_update(
                &world_state,
                "truth",
                false, // increment_intent_frequency = false (abstaining)
            );
            last_world = world_state;
        }

        // After many abstaining updates:
        // 1. Weight increased (updates were absorbed)
        let weight_after = brain.dejavu_clusters[0].total_weight;
        assert!(
            weight_after > initial_weight + (n_abstain as u32) / 2,
            "Weight must increase during abstention: initial={}, after={}, expected > {}",
            initial_weight, weight_after, initial_weight + (n_abstain as u32) / 2
        );

        // 2. Reverberation MUST NOT have increased (we never set increment_intent_frequency)
        let reverb_after = brain.dejavu_clusters[0].reverberation;
        assert!(
            reverb_after == initial_reverb,
            "Reverberation must NOT increase during abstention: initial={:.4}, after={:.4}",
            initial_reverb, reverb_after
        );

        // 3. Centroid MUST have moved (it tracked the drift through accumulator updates)
        let centroid_after = brain.dejavu_clusters[0].centroid;
        let centroid_shift = initial_centroid.normalized_hamming_distance(&centroid_after);
        assert!(
            centroid_shift > 0.0,
            "Centroid must track reality even during abstention"
        );

        // Now send updates WITH intent frequency increment (quorum agent).
        // Reverberation MUST increase.
        let n_quorum = 100;
        for i in 0..n_quorum {
            let n_flips = ((i / 10) + 1).min(20);
            let world_state = perturb(&last_world, n_flips, i + 5000);
            brain.absorb_epistemic_update(
                &world_state,
                "truth",
                true, // increment_intent_frequency = true (quorum agent)
            );
            last_world = world_state;
        }

        let reverb_final = brain.dejavu_clusters[0].reverberation;
        assert!(
            reverb_final > reverb_after + 0.01,
            "Reverberation must increase for quorum agent: after_abstain={:.4}, after_quorum={:.4}",
            reverb_after, reverb_final
        );

        eprintln!("  ✓ Grounding preserved: {} abstaining + {} quorum updates", n_abstain, n_quorum);
        eprintln!("    Centroid shift during abstention: {:.6}", centroid_shift);
        eprintln!("    Weight: {} → {} → {}", initial_weight, weight_after,
            brain.dejavu_clusters[0].total_weight);
        eprintln!("    Reverb: {:.4} (init) → {:.4} (after abstain) → {:.4} (after quorum)",
            initial_reverb, reverb_after, reverb_final);
    }

    #[test]
    fn test_xii1_promotion_boundedness() {
        // Theorem XII.1: append_composed_rule never creates new clusters.
        // If no existing cluster matches the antecedent (sim >= 0.65),
        // the composed rule is silently dropped (returns false).
        let mut brain = VSABrain::new(0.43);

        // Create two clusters with known centroids
        let c1 = Hypervector::encode_text_ngram("monetary_policy", 3);
        let c2 = Hypervector::encode_text_ngram("fiscal_outlook", 3);
        brain.add_to_dejavu_db(c1, "monetary_policy", HashMap::new());
        brain.add_to_dejavu_db(c2, "fiscal_outlook", HashMap::new());
        let n_clusters_before = brain.dejavu_clusters.len();
        assert_eq!(n_clusters_before, 2);

        // Record entry counts before any promotions
        let entries_before: Vec<usize> = brain.dejavu_clusters.iter()
            .map(|c| c.entries.len()).collect();

        // 1. Promote a rule whose antecedent matches an existing cluster (monetary_policy)
        let consequent1 = Hypervector::encode_text_ngram("rates_rise", 3);
        let stored1 = brain.append_composed_rule("monetary_policy", &consequent1);
        assert!(stored1, "append_composed_rule must succeed when antecedent matches");

        // 2. Promote another rule matching the same cluster
        let consequent2 = Hypervector::encode_text_ngram("bond_yields_up", 3);
        let stored2 = brain.append_composed_rule("monetary_policy", &consequent2);
        assert!(stored2, "Second promotion to same cluster must succeed");

        // 3. Promote a rule whose antecedent matches the OTHER cluster
        let consequent3 = Hypervector::encode_text_ngram("deficit_widens", 3);
        let stored3 = brain.append_composed_rule("fiscal_outlook", &consequent3);
        assert!(stored3, "Promotion to fiscal_outlook must succeed");

        // 4. No new clusters were created
        assert_eq!(
            brain.dejavu_clusters.len(), n_clusters_before,
            "Promotions must NOT create new clusters: before={}, after={}",
            n_clusters_before, brain.dejavu_clusters.len()
        );

        // 5. Entry counts increased only in the matching clusters
        let entries_after: Vec<usize> = brain.dejavu_clusters.iter()
            .map(|c| c.entries.len()).collect();
        assert_eq!(
            entries_after[0] - entries_before[0], 2,
            "Cluster 0 (monetary_policy) should have gained 2 entries"
        );
        assert_eq!(
            entries_after[1] - entries_before[1], 1,
            "Cluster 1 (fiscal_outlook) should have gained 1 entry"
        );

        // 6. Now promote a rule with NO matching antecedent — must return false
        let consequent4 = Hypervector::encode_text_ngram("tech_stocks_rise", 3);

        // Use an antecedent label that won't match any cluster (encoding is via
        // encode_sentence inside append_composed_rule, so we can't easily craft
        // one that doesn't match — but "xyzzy_unknown" should be far enough).
        // Actually, encode_sentence creates a trigram-based embedding, so any
        // text will have some distance. Let's use a highly dissimilar label.
        let stored4 = brain.append_composed_rule("completely_unrelated_topic_xyzzy", &consequent4);
        assert!(!stored4, "append_composed_rule must fail when no cluster matches");

        // 7. Still no new clusters
        assert_eq!(
            brain.dejavu_clusters.len(), n_clusters_before,
            "Failed promotions must NOT create new clusters"
        );

        // 8. Entry counts unchanged by the failed promotion
        let entries_final: Vec<usize> = brain.dejavu_clusters.iter()
            .map(|c| c.entries.len()).collect();
        assert_eq!(entries_final[0], entries_after[0],
            "Failed promotion must not affect cluster 0 entries");
        assert_eq!(entries_final[1], entries_after[1],
            "Failed promotion must not affect cluster 1 entries");

        // 9. Verify the absorbed entries are in the accumulator (centroid shifted)
        let centroid_shift_0 = brain.dejavu_clusters[0].centroid
            .normalized_hamming_distance(
                &Hypervector::encode_text_ngram("monetary_policy", 3));
        assert!(
            centroid_shift_0 > 0.0,
            "Centroid 0 should have shifted from absorbing promotions"
        );

        // 10. Weight increased from promotions
        assert!(
            brain.dejavu_clusters[0].total_weight > 1,
            "Cluster 0 weight should increase from promoted entries"
        );

        eprintln!("  ✓ Promotion boundedness verified: {} clusters before/after, entries d0=+{}, d1=+{}",
            n_clusters_before,
            entries_after[0] - entries_before[0],
            entries_after[1] - entries_before[1]);
        eprintln!("    Failed promotion correctly returned false, no clusters created");
    }
}
