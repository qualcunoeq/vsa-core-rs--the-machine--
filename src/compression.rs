// ─── Memory Compression Module ──────────────────────────────────────────────
//
// Addresses the unbounded RAM growth in "The Machine" VSA system.
//
// Ported from the enhanced-memory branch of the-machine-enhanced-memory-handling
// by **qualcunoeq** (https://github.com/qualcunoeq/the-machine-enhanced-memory-handling).
//
// ## Strategy
//
// | Layer | Problem | Solution |
// |-------|---------|----------|
// | 0 | visited: HashSet<String> | Counting Bloom filter (fixed 4 MB) |
// | 0 | seed_urls: Vec<String> | Capped VecDeque (max 50,000) |
// | 0 | doc_frequency: HashMap | Exponential decay + eviction |
// | 0 | transient_clusters | Hot/cold freeze (like dejavu) |
// | 0 | TUI clone per frame | Arc<Snapshot> with CAS swap |
// | 1 | accumulator: Vec<u32> (40 KB) | Sparse delta map (4 KB avg) |
// | 2 | entries per cluster | Age-weighted centroid collapse |
// | 3 | cold storage serialization | Centroid-delta + run-length encoding |
// | - | monitoring | Memory profiler tick |

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::mem;

// ─── Counting Bloom Filter ─────────────────────────────────────────────────
//
// Replaces `HashSet<String>` for visited URL tracking.
// Fixed memory: 4 MB regardless of how many URLs are visited.
// False positive rate ~1% for up to 10M URLs.
//
// Uses 4 independent hash functions derived from the string's
// inherent hash via a splitmix64 permutation.

pub struct CountingBloomFilter {
    bits: Vec<u64>,
    num_bits: u64,
    num_hashes: u32,
    count: usize,
}

impl CountingBloomFilter {
    /// Create a new Bloom filter with `num_bits` bits and `num_hashes` hash functions.
    /// Recommended: 4M bits (512 KB) with 4 hashes → ~1% FPR for 100K URLs.
    pub fn new(num_bits: usize, num_hashes: u32) -> Self {
        let word_count = (num_bits + 63) / 64;
        CountingBloomFilter {
            bits: vec![0u64; word_count],
            num_bits: num_bits as u64,
            num_hashes,
            count: 0,
        }
    }

    /// Create a Bloom filter with sensible defaults.
    /// 32M bits (4 MB) with 6 hashes → ~1% FPR for ~2.5M URLs,
    /// ~0.1% FPR for ~1M URLs.
    pub fn default_large() -> Self {
        Self::new(32_000_000, 6)
    }

    fn hash_indices(&self, item: &str) -> Vec<u64> {
        // Use the string's default Hasher to get a seed, then
        // generate `num_hashes` indices via splitmix64.
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        item.hash(&mut hasher);
        let seed = hasher.finish();

        let mut indices = Vec::with_capacity(self.num_hashes as usize);
        let mut x = seed;
        for _ in 0..self.num_hashes {
            // splitmix64 permutation
            x = x.wrapping_add(0x9E3779B97F4A7C15);
            let mut z = x;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
            z = z ^ (z >> 31);
            indices.push(z % self.num_bits);
        }
        indices
    }

    /// Insert an item into the Bloom filter.
    pub fn insert(&mut self, item: &str) {
        for idx in self.hash_indices(item) {
            let word = (idx / 64) as usize;
            let bit = idx % 64;
            if word < self.bits.len() {
                self.bits[word] |= 1u64 << bit;
            }
        }
        self.count += 1;
    }

    /// Check whether an item MAY have been inserted.
    /// Returns `false` definitely; `true` means "maybe" (false positives possible).
    pub fn maybe_contains(&self, item: &str) -> bool {
        for idx in self.hash_indices(item) {
            let word = (idx / 64) as usize;
            let bit = idx % 64;
            if word >= self.bits.len() {
                return false;
            }
            if (self.bits[word] >> bit) & 1 == 0 {
                return false;
            }
        }
        true
    }

    /// Clear the filter (equivalent to creating a new one, but reuses allocation).
    pub fn clear(&mut self) {
        for w in &mut self.bits {
            *w = 0;
        }
        self.count = 0;
    }

    /// Approximate number of unique items inserted.
    /// Based on the fraction of bits set.
    pub fn approx_count(&self) -> f64 {
        let total_bits = self.bits.len() as f64 * 64.0;
        let set_bits: u64 = self.bits.iter().map(|w| w.count_ones() as u64).sum();
        let p = set_bits as f64 / total_bits;
        if p >= 1.0 {
            return self.count as f64;
        }
        -(1.0 - p).ln() * total_bits / self.num_hashes as f64
    }
}

// ─── Capped VecDeque ───────────────────────────────────────────────────────
//
// Fixed-capacity queue that evicts the oldest entry when full.
// Used for seed_urls and other unbounded growable Vecs.

pub struct CappedVecDeque<T> {
    inner: Vec<T>,
    max_len: usize,
}

impl<T> CappedVecDeque<T> {
    pub fn new(max_len: usize) -> Self {
        CappedVecDeque {
            inner: Vec::with_capacity(max_len.min(1024)),
            max_len,
        }
    }

    pub fn push_back(&mut self, value: T) {
        if self.inner.len() >= self.max_len {
            self.inner.remove(0);
        }
        self.inner.push(value);
    }

    pub fn pop_front(&mut self) -> Option<T> {
        if self.inner.is_empty() {
            None
        } else {
            Some(self.inner.remove(0))
        }
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.inner.get(index)
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }

    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.inner.iter()
    }

    pub fn as_vec(&self) -> &Vec<T> {
        &self.inner
    }
}

// ─── Sparse Accumulator ────────────────────────────────────────────────────
//
// Replaces `Vec<u32>` (40 KB fixed per hot cluster) with a sparse
// delta encoding.  Stores only the indices where the accumulator value
// differs from the default (typically 0 or the mean).
//
// For a typical hot cluster where ~10% of bits diverge from the mean:
//   - Dense: 10,240 × 4 = 40,960 bytes
//   - Sparse: ~1,024 × (2 + 2) = ~4,096 bytes  → **10× reduction**
//
// The default value is typically 0 (bits that have never been observed).
// On reconstruction, we compute:  value[i] = default + delta_map.get(i, 0)

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SparseAccumulator {
    /// The default value for all positions not in `deltas`.
    pub default: u32,
    /// Map from position index to delta-from-default.
    /// Stored as a Vec of (index, delta) pairs for better cache locality
    /// than HashMap, since we iterate sequentially on reconstruction.
    pub deltas: Vec<(u16, i32)>,
    /// Total weight (mirrors MemoryCluster.total_weight).
    pub total_weight: u32,
}

impl SparseAccumulator {
    pub fn new(total_weight: u32) -> Self {
        SparseAccumulator {
            default: 0,
            deltas: Vec::with_capacity(1024), // ~10% of 10240
            total_weight,
        }
    }

    /// Get the value at a given dimension index.
    pub fn get(&self, idx: usize) -> u32 {
        let pos = idx as u16;
        // Binary search on the sorted deltas
        if let Ok(i) = self.deltas.binary_search_by_key(&pos, |&(p, _)| p) {
            (self.default as i32 + self.deltas[i].1) as u32
        } else {
            self.default
        }
    }

    /// Set the value at a given dimension index.
    /// Returns the old value (0 if not present).
    pub fn set(&mut self, idx: usize, value: u32) -> u32 {
        let pos = idx as u16;
        let delta = value as i32 - self.default as i32;
        let old_val = self.get(idx);

        match self.deltas.binary_search_by_key(&pos, |&(p, _)| p) {
            Ok(i) => {
                let old_delta = self.deltas[i].1;
                self.deltas[i].1 = delta;
                (self.default as i32 + old_delta) as u32
            }
            Err(i) => {
                self.deltas.insert(i, (pos, delta));
                old_val
            }
        }
    }

    /// Add a value to a given dimension (for accumulator absorption).
    pub fn add(&mut self, idx: usize, val: u32) {
        let pos = idx as u16;
        match self.deltas.binary_search_by_key(&pos, |&(p, _)| p) {
            Ok(i) => {
                self.deltas[i].1 += val as i32;
            }
            Err(i) => {
                if val != self.default {
                    self.deltas.insert(i, (pos, val as i32 - self.default as i32));
                }
            }
        }
    }

    /// Apply a decay factor to all deltas.
    pub fn decay(&mut self, factor: f64) {
        for (_, delta) in &mut self.deltas {
            let abs = (*delta).unsigned_abs();
            let decayed = (abs as f64 * factor).round() as u32;
            *delta = if *delta >= 0 { decayed as i32 } else { -(decayed as i32) };
        }
        // Prune entries that have decayed to zero
        self.deltas.retain(|&(_, d)| d != 0);
    }

    /// Reconstruct the full Vec<u32> for threshold computation.
    /// This is O(D) but only called during centroid recomputation.
    pub fn to_dense(&self) -> Vec<u32> {
        let mut dense = vec![self.default; 10240];
        let mut di = 0;
        let len = self.deltas.len();
        for i in 0..10240usize {
            if di < len && self.deltas[di].0 as usize == i {
                dense[i] = (self.default as i32 + self.deltas[di].1) as u32;
                di += 1;
            }
        }
        dense
    }

    /// Build a sparse accumulator from a dense Vec<u32>.
    pub fn from_dense(dense: &[u32], total_weight: u32) -> Self {
        // Compute the mean as the default
        let sum: u64 = dense.iter().map(|&v| v as u64).sum();
        let default = (sum / dense.len() as u64) as u32;

        let mut deltas: Vec<(u16, i32)> = Vec::with_capacity(dense.len() / 10);
        for (i, &v) in dense.iter().enumerate() {
            let delta = v as i32 - default as i32;
            if delta != 0 {
                deltas.push((i as u16, delta));
            }
        }

        SparseAccumulator {
            default,
            deltas,
            total_weight,
        }
    }

    /// Memory used by this sparse accumulator (approximate).
    pub fn memory_bytes(&self) -> usize {
        mem::size_of::<Self>() + self.deltas.len() * mem::size_of::<(u16, i32)>()
    }

    /// Sparsity ratio (0.0 = fully dense, 1.0 = all default).
    pub fn sparsity(&self) -> f64 {
        1.0 - (self.deltas.len() as f64 / 10240.0)
    }
}

// ─── Memory Profiler ───────────────────────────────────────────────────────
//
// Instruments the system to log cluster counts, accumulator sparsity,
// and entry counts per cluster.  Called periodically by the agent loop.

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct MemorySnapshot {
    pub dejavu_clusters: usize,
    pub hot_clusters: usize,
    pub cold_clusters: usize,
    pub transient_clusters: usize,
    pub total_entries: usize,
    pub total_accumulator_kb: f64,
    pub visited_urls_approx: f64,
    pub seed_queue_len: usize,
    pub doc_frequency_entries: usize,
    pub experiences_len: usize,
    pub broker_clusters: usize,
}

pub fn log_memory_snapshot(snapshot: &MemorySnapshot) {
    println!(
        "[MEMORY] clusters: {} (hot: {}, cold: {}) | transient: {} | entries: {} | accumulators: {:.1} KB | visited: ~{:.0} | seeds: {} | doc_freq: {} | experiences: {}",
        snapshot.dejavu_clusters,
        snapshot.hot_clusters,
        snapshot.cold_clusters,
        snapshot.transient_clusters,
        snapshot.total_entries,
        snapshot.total_accumulator_kb,
        snapshot.visited_urls_approx,
        snapshot.seed_queue_len,
        snapshot.doc_frequency_entries,
        snapshot.experiences_len,
    );
}

// ─── Layer 2: Entry Merging ─────────────────────────────────────────────────
//
// Age-weighted centroid collapse for MemoryCluster entries.
// See the design doc for the full rationale.

use crate::{DejavuEntry, Hypervector, MemoryCluster, HD_DIMENSION, U64_BLOCKS};

/// Configuration for entry merging.
#[derive(Clone, Debug)]
pub struct MergeConfig {
    /// Merge when a cluster has this many entries (default: 600).
    pub trigger_count: usize,
    /// Don't touch entries younger than this many ticks (default: 50).
    pub young_tick_threshold: u64,
    /// Merge old entries unconditionally beyond this tick age (default: 500).
    pub old_tick_threshold: u64,
    /// Maximum mean pairwise Hamming ratio within a middle-age cohort
    /// before splitting (default: 0.35).  At 0.35, ~3,584 bits differ
    /// out of 10,240 — conservative for VSA.
    pub max_hamming_ratio: f64,
    /// Minimum cohort size to bother merging (default: 3).
    pub min_cohort_size: usize,
}

impl Default for MergeConfig {
    fn default() -> Self {
        MergeConfig {
            trigger_count: 600,
            young_tick_threshold: 50,
            old_tick_threshold: 500,
            max_hamming_ratio: 0.35,
            min_cohort_size: 3,
        }
    }
}

/// Compute the mean pairwise normalized Hamming distance within a cohort
/// of reconstructed hypervectors.  This is the coherence guard check.
///
/// O(n² × D) for n = cohort size.  Since this runs once per merge and
/// merge only fires on large clusters, the cost is acceptable.
pub fn mean_hamming_within_cohort(
    entries: &[DejavuEntry],
    anchor: &Hypervector,
) -> f64 {
    let n = entries.len();
    if n < 2 {
        return 0.0;
    }

    let mut total_dist = 0.0_f64;
    let mut pairs = 0_u64;

    // Reconstruct all vectors first to avoid repeated delta-decoding
    let reconstructed: Vec<Hypervector> = entries
        .iter()
        .map(|e| e.reconstruct(anchor))
        .collect();

    for i in 0..n {
        for j in (i + 1)..n {
            total_dist += reconstructed[i].normalized_hamming_distance(&reconstructed[j]);
            pairs += 1;
        }
    }

    total_dist / pairs as f64
}

/// Single-step VSA k-means bisection of a cohort.
///
/// If a middle-age cohort fails the coherence guard (mean Hamming > threshold),
/// we split it into two sub-groups by:
///
/// 1. Pick the two most distant vectors as initial centroids
/// 2. Assign each vector to the nearer centroid
/// 3. Return the two resulting groups
///
/// Returns (group_a, group_b).  Guarantees both groups are non-empty
/// (if entries.len() >= 2).
pub fn vsa_bisect(
    entries: &[DejavuEntry],
    anchor: &Hypervector,
) -> (Vec<DejavuEntry>, Vec<DejavuEntry>) {
    if entries.len() < 2 {
        return (entries.to_vec(), Vec::new());
    }

    let reconstructed: Vec<Hypervector> = entries
        .iter()
        .map(|e| e.reconstruct(anchor))
        .collect();

    // Step 1: Find the two most distant vectors as initial centroids
    let mut max_dist = -1.0_f64;
    let mut seed_a = 0;
    let mut seed_b = 1;
    for i in 0..entries.len() {
        for j in (i + 1)..entries.len() {
            let d = reconstructed[i].normalized_hamming_distance(&reconstructed[j]);
            if d > max_dist {
                max_dist = d;
                seed_a = i;
                seed_b = j;
            }
        }
    }

    // Step 2: Assign each vector to the nearer centroid
    let centroid_a = reconstructed[seed_a];
    let centroid_b = reconstructed[seed_b];

    let mut group_a: Vec<DejavuEntry> = Vec::new();
    let mut group_b: Vec<DejavuEntry> = Vec::new();

    for i in 0..entries.len() {
        let d_a = reconstructed[i].normalized_hamming_distance(&centroid_a);
        let d_b = reconstructed[i].normalized_hamming_distance(&centroid_b);
        if d_a <= d_b {
            group_a.push(entries[i].clone());
        } else {
            group_b.push(entries[i].clone());
        }
    }

    // If one group is empty (shouldn't happen with distinct seeds, but guard)
    if group_a.is_empty() {
        group_a.push(group_b.pop().unwrap_or_else(|| entries[0].clone()));
    }
    if group_b.is_empty() {
        group_b.push(group_a.pop().unwrap_or_else(|| entries[1].clone()));
    }

    (group_a, group_b)
}

/// Bundle a cohort of entries into a single summary hypervector using
/// majority-rule thresholding.
///
/// Returns (summary_vector, total_weight_in_cohort).
/// The summary bit = 1 iff popcount(bit across all entries) > n/2.
/// Even-bundle ties use the first entry's bit (deterministic).
fn bundle_and_threshold(
    entries: &[DejavuEntry],
    anchor: &Hypervector,
) -> (Hypervector, u32) {
    let n = entries.len();
    if n == 0 {
        return (Hypervector::new_zero(), 0);
    }
    if n == 1 {
        let v = entries[0].reconstruct(anchor);
        return (v, entries[0].weight.max(1));
    }

    // Total weight = sum of individual weights
    let total_weight: u32 = entries.iter().map(|e| e.weight.max(1)).sum();

    // Majority-rule bundling
    let mut result = [0u64; U64_BLOCKS];
    let halfway = n / 2;
    let is_even = n % 2 == 0;

    // Use the first entry's first block as noise seed for tie-breaking
    let tie_seed = entries[0].reconstruct(anchor).bits[0];

    for block_idx in 0..U64_BLOCKS {
        let mut block_consensus = 0u64;
        for bit_idx in 0..64 {
            let mut bit_count = 0u32;
            for entry in entries {
                let v = entry.reconstruct(anchor);
                if ((v.bits[block_idx] >> bit_idx) & 1) == 1 {
                    bit_count += 1;
                }
            }

            if is_even && bit_count as usize == halfway {
                // Tie-break using deterministic seed
                let noise_bit = ((tie_seed >> bit_idx) & 1) == 1;
                if noise_bit {
                    block_consensus |= 1 << bit_idx;
                }
            } else if bit_count as usize > halfway {
                block_consensus |= 1 << bit_idx;
            }
        }
        result[block_idx] = block_consensus;
    }

    (Hypervector { bits: result }, total_weight)
}

/// Main entry point for Layer 2 entry merging.
///
/// Called on a single MemoryCluster when its entry count exceeds
/// `config.trigger_count`.  Partitions entries into three age cohorts
/// (young, middle, old), merges each cohort according to the rules,
/// and replaces the cluster's entries with the merged set.
///
/// After merging, the accumulator is rebuilt from scratch via
/// `rebuild_accumulator_from_entries` to correctly reflect weights.
///
/// Returns the number of entries removed (for logging).
pub fn merge_entries(
    cluster: &mut MemoryCluster,
    config: &MergeConfig,
    current_tick: u64,
) -> usize {
    let before = cluster.entries.len();
    if before < config.trigger_count {
        return 0;
    }

    // Ensure anchor is set for delta-encoded reconstruction
    cluster.ensure_anchor();

    // Partition entries into three age cohorts
    let mut young: Vec<DejavuEntry> = Vec::new();
    let mut middle: Vec<DejavuEntry> = Vec::new();
    let mut old: Vec<DejavuEntry> = Vec::new();

    for entry in cluster.entries.drain(..) {
        let age = current_tick.saturating_sub(entry.creation_tick);
        if age < config.young_tick_threshold {
            young.push(entry);
        } else if age > config.old_tick_threshold {
            old.push(entry);
        } else {
            middle.push(entry);
        }
    }

    let anchor = cluster.anchor;
    let mut merged: Vec<DejavuEntry> = Vec::new();

    // Process young cohort — untouched, just re-add
    merged.extend(young);

    // Process old cohort — merge unconditionally
    if old.len() >= config.min_cohort_size {
        // Merge all old entries into one weighted entry
        let oldest_tick = old.iter().map(|e| e.creation_tick).min().unwrap_or(0);
        let (summary, weight) = bundle_and_threshold(&old, &anchor);
        merged.push(DejavuEntry::new_merged(
            summary,
            format!("merged_old_{}", oldest_tick),
            weight,
            oldest_tick,
        ));
    } else {
        merged.extend(old);
    }

    // Process middle cohort — coherence guard
    if middle.len() >= config.min_cohort_size {
        let mean_dist = mean_hamming_within_cohort(&middle, &anchor);
        if mean_dist < config.max_hamming_ratio {
            // Coherent enough — merge into one
            let oldest_tick = middle.iter().map(|e| e.creation_tick).min().unwrap_or(0);
            let (summary, weight) = bundle_and_threshold(&middle, &anchor);
            merged.push(DejavuEntry::new_merged(
                summary,
                format!("merged_mid_{}", oldest_tick),
                weight,
                oldest_tick,
            ));
        } else {
            // Not coherent — bisect and merge each sub-group separately
            let (group_a, group_b) = vsa_bisect(&middle, &anchor);

            for (group, prefix) in [(group_a, "a"), (group_b, "b")] {
                if group.len() >= config.min_cohort_size {
                    let oldest_tick = group.iter().map(|e| e.creation_tick).min().unwrap_or(0);
                    let (summary, weight) = bundle_and_threshold(&group, &anchor);
                    merged.push(DejavuEntry::new_merged(
                        summary,
                        format!("merged_mid_{}_{}", prefix, oldest_tick),
                        weight,
                        oldest_tick,
                    ));
                } else {
                    merged.extend(group);
                }
            }
        }
    } else {
        merged.extend(middle);
    }

    // Replace cluster entries with merged set
    cluster.entries = merged;

    // Rebuild accumulator from weighted entries
    cluster.rebuild_accumulator_from_entries();

    let removed = before.saturating_sub(cluster.entries.len());
    removed
}

// ─── Layer 3: Cold Cluster Serialization ──────────────────────────────────
//
// Centroid-delta encoding with adaptive Raw vs Delta+GolombRice.
// See the design doc for the full rationale.
//
// ## Bit-level I/O

/// LSB-first bit writer.  Writes into an internal Vec<u8>.
pub struct BitWriter {
    buf: Vec<u8>,
    pos: usize, // bit position within the current byte (0..8)
}

impl BitWriter {
    pub fn new() -> Self {
        BitWriter {
            buf: vec![0u8],
            pos: 0,
        }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    /// Write a single bit (0 or 1).
    pub fn write_bit(&mut self, bit: u8) {
        if self.pos >= 8 {
            self.buf.push(0u8);
            self.pos = 0;
        }
        if bit != 0 {
            *self.buf.last_mut().unwrap() |= 1u8 << self.pos;
        }
        self.pos += 1;
    }

    /// Write `count` bits of `value` (LSB first, `count` ≤ 64).
    pub fn write_bits(&mut self, value: u64, count: u32) {
        for i in 0..count {
            let bit = ((value >> i) & 1) as u8;
            self.write_bit(bit);
        }
    }

    /// Write unary code: `n` ones followed by a zero.
    pub fn write_unary(&mut self, n: u32) {
        for _ in 0..n {
            self.write_bit(1);
        }
        self.write_bit(0);
    }

    /// Pad to byte boundary with zeros (if not already aligned).
    pub fn align_to_byte(&mut self) {
        while self.pos != 0 {
            self.write_bit(0);
        }
    }

    /// Current length in bytes (includes the partial last byte).
    pub fn byte_len(&self) -> usize {
        self.buf.len()
    }
}

/// LSB-first bit reader over a byte slice.
pub struct BitReader<'a> {
    buf: &'a [u8],
    byte_idx: usize,
    pos: usize, // bit position within current byte (0..8)
}

impl<'a> BitReader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        BitReader {
            buf,
            byte_idx: 0,
            pos: 0,
        }
    }

    /// Read a single bit.  Returns 0 or 1.  Panics if out of bounds.
    pub fn read_bit(&mut self) -> u8 {
        if self.pos >= 8 {
            self.byte_idx += 1;
            self.pos = 0;
        }
        let bit = (self.buf[self.byte_idx] >> self.pos) & 1;
        self.pos += 1;
        bit
    }

    /// Read `count` bits and return as a u64 (LSB first, `count` ≤ 64).
    pub fn read_bits(&mut self, count: u32) -> u64 {
        let mut value = 0u64;
        for i in 0..count {
            let bit = self.read_bit() as u64;
            value |= bit << i;
        }
        value
    }

    /// Read unary code: count the 1-bits until a 0-bit.
    pub fn read_unary(&mut self) -> u32 {
        let mut count = 0u32;
        loop {
            let bit = self.read_bit();
            if bit == 0 {
                return count;
            }
            count += 1;
        }
    }

    /// Return current position in bits.
    pub fn bit_pos(&self) -> usize {
        self.byte_idx * 8 + self.pos
    }
}

// ─── Golomb-Rice Coding ───────────────────────────────────────────────────

/// Golomb-Rice parameter constants.
/// For a geometric distribution with mean µ, optimal k = floor(log2(ln(2)·µ)).
/// For 640 set bits in 10,240 positions → mean gap ≈ 16 → k = floor(log2(11.1)) = 3.
pub const GOLOMB_RICE_DEFAULT_K: u32 = 3;

/// Encode sorted u16 indices as Golomb-Rice coded gaps.
/// `indices` must be sorted ascending.  `k` is the Rice parameter (bits for remainder).
/// Returns the encoded byte vector.
pub fn golomb_rice_encode(indices: &[u16], k: u32) -> Vec<u8> {
    let mut writer = BitWriter::new();
    let mut prev: u32 = 0;
    for &idx in indices {
        let gap = (idx as u32).wrapping_sub(prev);
        prev = idx as u32;
        let m = 1u32 << k;
        let q = gap / m;      // quotient
        let r = gap % m;      // remainder
        // Write unary: q ones followed by a zero terminator
        writer.write_unary(q);
        // Write remainder: k bits binary
        writer.write_bits(r as u64, k);
    }
    writer.align_to_byte();
    writer.into_bytes()
}

/// Decode Golomb-Rice coded bytes back into sorted u16 indices.
/// `encoded` is the output of `golomb_rice_encode`.  `num_indices` is how many
/// indices to decode.  Returns the decoded indices.
pub fn golomb_rice_decode(encoded: &[u8], num_indices: usize, k: u32) -> Vec<u16> {
    let mut reader = BitReader::new(encoded);
    let mut indices = Vec::with_capacity(num_indices);
    let mut prev: u32 = 0;
    let m = 1u32 << k;
    for _ in 0..num_indices {
        let q = reader.read_unary();
        let r = reader.read_bits(k) as u32;
        let gap = q * m + r;
        let idx = prev.wrapping_add(gap) as u16;
        indices.push(idx);
        prev = idx as u32;
    }
    indices
}

// ─── Adaptive Entry Encoding ──────────────────────────────────────────────

/// Crossover: below this Hamming weight ratio, Delta+GR encoding wins over Raw.
/// 6.25% of 10,240 bits = 640 bits.
/// Derived from:  Raw = 1280 bytes ≈ 10240 bits.
/// Delta+GR with µ bit-flips ≈ µ * (unary_avg + k) bits.
/// At µ = 640, 640 * (log2(640) + 3) ≈ 640 * 12.3 ≈ 7872 bits ≈ 984 bytes → still better
/// Actually the crossover is closer to 10% in practice due to unary overhead.
/// We use 8% as a conservative threshold.
pub const DELTA_CROSSOVER_BITS: usize = 820; // 8% of 10240

/// Encoded representation of a single entry.
pub enum EncodedEntry {
    /// Raw hypervector (1,280 bytes).  Used when the entry is far from centroid.
    Raw(Hypervector),
    /// Delta encoding: set-bit indices of δ = E ⊕ C, Golomb-Rice compressed.
    Delta {
        /// The Golomb-Rice parameter k used.
        k: u32,
        /// Number of set-bit indices encoded.
        num_indices: u32,
        /// The compressed gap data.
        data: Vec<u8>,
    },
}

/// Adaptive encoding: chooses Raw or Delta+GolombRice based on Hamming distance.
pub fn encode_entry(entry: &DejavuEntry, centroid: &Hypervector) -> EncodedEntry {
    let vec = entry.reconstruct(centroid);
    let delta = vec.bitwise_xor(centroid);
    let set_bits = delta.count_ones();

    if set_bits > DELTA_CROSSOVER_BITS {
        // Too far from centroid — store raw
        EncodedEntry::Raw(vec)
    } else {
        // Find set-bit indices
        let mut indices = Vec::with_capacity(set_bits);
        for block_idx in 0..U64_BLOCKS {
            let word = delta.bits[block_idx];
            if word == 0 {
                continue;
            }
            for bit_idx in 0..64 {
                if ((word >> bit_idx) & 1) == 1 {
                    let abs_idx = (block_idx * 64 + bit_idx) as u16;
                    indices.push(abs_idx);
                }
            }
        }
        // Choose optimal k based on mean gap
        let mean_gap = if set_bits > 1 {
            HD_DIMENSION as f64 / set_bits as f64
        } else {
            HD_DIMENSION as f64
        };
        let optimal_k = ((mean_gap * std::f64::consts::LN_2).log2().round() as u32).max(1).min(8);
        let data = golomb_rice_encode(&indices, optimal_k);
        EncodedEntry::Delta {
            k: optimal_k,
            num_indices: set_bits as u32,
            data,
        }
    }
}

/// Decode an encoded entry back into a DejavuEntry.
pub fn decode_entry(
    encoded: &EncodedEntry,
    centroid: &Hypervector,
    label: &str,
    weight: u32,
    creation_tick: u64,
) -> DejavuEntry {
    let mut entry = match encoded {
        EncodedEntry::Raw(vec) => {
            DejavuEntry::new(*vec, label.to_string(), HashMap::new(), None)
        }
        EncodedEntry::Delta { k, num_indices, data } => {
            let indices = golomb_rice_decode(data, *num_indices as usize, *k);
            // Reconstruct δ from indices
            let mut delta_bits = [0u64; U64_BLOCKS];
            for &idx in &indices {
                let block = (idx as usize) / 64;
                let bit = (idx as usize) % 64;
                delta_bits[block] |= 1u64 << bit;
            }
            let delta_hv = Hypervector { bits: delta_bits };
            // Reconstruct original: E = C ⊕ δ
            let original = centroid.bitwise_xor(&delta_hv);
            DejavuEntry::new(original, label.to_string(), HashMap::new(), None)
        }
    };
    entry.weight = weight.max(1);
    entry.creation_tick = creation_tick;
    entry
}

// ─── Cold Cluster Serialization ───────────────────────────────────────────

/// Magic bytes for cold cluster serialization ("MACH").
const COLD_CLUSTER_MAGIC: u32 = 0x4D414348;
const COLD_CLUSTER_VERSION: u32 = 1;

/// Serialize a MemoryCluster into a compact binary representation.
///
/// Format (not valid Rust — illustrative only):
/// ```ignore
/// u32: magic (0x4D414348 "MACH")
/// u32: version (1)
/// u16: num_entries
///
/// [u8; 1280]: centroid bytes (raw)
///
/// u32: total_weight
/// u32: num_nonzero_accumulator_entries
/// for each:
///   u16: index
///   u32: value
///
/// for each entry:
///   u32: weight
///   u64: creation_tick
///   u8:  tag (0=Raw, 1=Delta+GR)
///   if Raw:     [u8; 1280] hypervector
///   if Delta:   u8(k) + u32(num_indices) + u16(data_len) + [u8; data_len]
/// ```
pub fn serialize_cold_cluster(cluster: &MemoryCluster) -> Vec<u8> {
    use std::io::{Write, Cursor};

    let mut buf = Cursor::new(Vec::new());

    // Header
    buf.write_all(&COLD_CLUSTER_MAGIC.to_le_bytes()).unwrap();
    buf.write_all(&COLD_CLUSTER_VERSION.to_le_bytes()).unwrap();
    buf.write_all(&(cluster.entries.len() as u16).to_le_bytes()).unwrap();

    // Centroid
    buf.write_all(&cluster.centroid.to_bytes()).unwrap();

    // Accumulator
    buf.write_all(&cluster.total_weight.to_le_bytes()).unwrap();
    if cluster.is_hot() {
        let mut acc_entries: Vec<(u16, u32)> = Vec::new();
        for i in 0..HD_DIMENSION {
            if i < cluster.accumulator.len() && cluster.accumulator[i] != 0 {
                acc_entries.push((i as u16, cluster.accumulator[i]));
            }
        }
        buf.write_all(&(acc_entries.len() as u32).to_le_bytes()).unwrap();
        for (idx, val) in &acc_entries {
            buf.write_all(&idx.to_le_bytes()).unwrap();
            buf.write_all(&val.to_le_bytes()).unwrap();
        }
    } else {
        buf.write_all(&0u32.to_le_bytes()).unwrap();
    }

    // Entries
    for entry in &cluster.entries {
        buf.write_all(&entry.weight.to_le_bytes()).unwrap();
        buf.write_all(&entry.creation_tick.to_le_bytes()).unwrap();

        let encoded = encode_entry(entry, &cluster.centroid);
        match encoded {
            EncodedEntry::Raw(vec) => {
                buf.write_all(&[0u8]).unwrap();
                buf.write_all(&vec.to_bytes()).unwrap();
            }
            EncodedEntry::Delta { k, num_indices, data } => {
                buf.write_all(&[1u8]).unwrap(); // tag
                buf.write_all(&(k as u8).to_le_bytes()).unwrap();   // 1 byte k
                buf.write_all(&num_indices.to_le_bytes()).unwrap(); // 4 bytes count
                buf.write_all(&(data.len() as u16).to_le_bytes()).unwrap(); // 2 bytes data len
                buf.write_all(&data).unwrap();
            }
        }
    }

    buf.into_inner()
}

/// Deserialize a MemoryCluster from bytes produced by `serialize_cold_cluster`.
/// Returns None if the magic doesn't match or data is truncated.
pub fn deserialize_cold_cluster(bytes: &[u8]) -> Option<MemoryCluster> {
    use std::io::{Read, Cursor};

    let mut cursor = Cursor::new(bytes);

    // Header
    let mut magic_buf = [0u8; 4];
    cursor.read_exact(&mut magic_buf).ok()?;
    if u32::from_le_bytes(magic_buf) != COLD_CLUSTER_MAGIC {
        return None;
    }

    let mut version_buf = [0u8; 4];
    cursor.read_exact(&mut version_buf).ok()?;

    let mut num_entries_buf = [0u8; 2];
    cursor.read_exact(&mut num_entries_buf).ok()?;
    let num_entries = u16::from_le_bytes(num_entries_buf) as usize;

    // Centroid
    let mut centroid_bytes = [0u8; 1280];
    cursor.read_exact(&mut centroid_bytes).ok()?;
    let centroid = Hypervector::from_bytes(&centroid_bytes);

    // Accumulator
    let mut tw_buf = [0u8; 4];
    cursor.read_exact(&mut tw_buf).ok()?;
    let total_weight = u32::from_le_bytes(tw_buf);

    let mut acc_count_buf = [0u8; 4];
    cursor.read_exact(&mut acc_count_buf).ok()?;
    let acc_count = u32::from_le_bytes(acc_count_buf) as usize;

    let mut accumulator = vec![0u32; HD_DIMENSION];
    for _ in 0..acc_count {
        let mut idx_buf = [0u8; 2];
        cursor.read_exact(&mut idx_buf).ok()?;
        let idx = u16::from_le_bytes(idx_buf) as usize;
        let mut val_buf = [0u8; 4];
        cursor.read_exact(&mut val_buf).ok()?;
        let val = u32::from_le_bytes(val_buf);
        if idx < HD_DIMENSION {
            accumulator[idx] = val;
        }
    }

    // Entries
    let mut entries = Vec::with_capacity(num_entries);
    for _ in 0..num_entries {
        let mut weight_buf = [0u8; 4];
        cursor.read_exact(&mut weight_buf).ok()?;
        let weight = u32::from_le_bytes(weight_buf);

        let mut tick_buf = [0u8; 8];
        cursor.read_exact(&mut tick_buf).ok()?;
        let creation_tick = u64::from_le_bytes(tick_buf);

        let mut tag_buf = [0u8; 1];
        cursor.read_exact(&mut tag_buf).ok()?;
        let tag = tag_buf[0];

        let entry = match tag {
            0 => {
                let mut raw_bytes = [0u8; 1280];
                cursor.read_exact(&mut raw_bytes).ok()?;
                let vec = Hypervector::from_bytes(&raw_bytes);
                decode_entry(
                    &EncodedEntry::Raw(vec),
                    &centroid, "cold_restore", weight, creation_tick,
                )
            }
            1 => {
                let mut k_buf = [0u8; 1];
                cursor.read_exact(&mut k_buf).ok()?;
                let k = k_buf[0] as u32;

                let mut count_buf = [0u8; 4];
                cursor.read_exact(&mut count_buf).ok()?;
                let num_indices = u32::from_le_bytes(count_buf);

                let mut len_buf = [0u8; 2];
                cursor.read_exact(&mut len_buf).ok()?;
                let data_len = u16::from_le_bytes(len_buf) as usize;

                let mut data = vec![0u8; data_len];
                cursor.read_exact(&mut data).ok()?;

                decode_entry(
                    &EncodedEntry::Delta { k, num_indices, data },
                    &centroid, "cold_restore", weight, creation_tick,
                )
            }
            _ => return None,
        };
        entries.push(entry);
    }

    let anchor = centroid;

    Some(MemoryCluster {
        centroid,
        entries,
        reverberation: 0.5,
        last_reinforced_tick: 0,
        anchor,
        accumulator,
        total_weight,
        last_access_tick: 0,
    })
}

// ─── Cold Storage Manager ─────────────────────────────────────────────────

/// Manages cold (serialized) clusters for VSABrain.
///
/// When a cluster is frozen, its entries and accumulator are serialized
/// into a byte buffer and stored in this manager, keyed by the cluster's
/// index in VSABrain's `dejavu_clusters` vec.  The in-memory cluster
/// keeps its centroid and anchor (for LSH routing) but drops entries
/// and accumulator.
///
/// On write access, the cluster is thawed: deserialized from cold storage
/// back into entries and accumulator.
#[derive(Clone, Default)]
pub struct ColdStorageManager {
    /// Maps cluster index → serialized bytes.
    /// The index must be stable — it's the cluster's position in
    /// `VSABrain::dejavu_clusters`.
    storage: HashMap<usize, Vec<u8>>,
}

impl ColdStorageManager {
    pub fn new() -> Self {
        ColdStorageManager {
            storage: HashMap::new(),
        }
    }

    /// Store a serialized cluster.  Replaces any existing entry at `idx`.
    pub fn store(&mut self, idx: usize, serialized: Vec<u8>) {
        self.storage.insert(idx, serialized);
    }

    /// Retrieve and remove serialized data for a cluster.
    /// Returns `None` if not in cold storage.
    pub fn take(&mut self, idx: usize) -> Option<Vec<u8>> {
        self.storage.remove(&idx)
    }

    /// Check if a cluster is in cold storage.
    pub fn contains(&self, idx: usize) -> bool {
        self.storage.contains_key(&idx)
    }

    /// Remove a cluster from cold storage (e.g. when the cluster itself is deleted).
    pub fn remove(&mut self, idx: usize) {
        self.storage.remove(&idx);
    }

    /// Number of clusters in cold storage.
    pub fn len(&self) -> usize {
        self.storage.len()
    }

    /// Total bytes in cold storage.
    pub fn total_bytes(&self) -> usize {
        self.storage.values().map(|v| v.len()).sum()
    }

    /// Clear all cold storage.
    pub fn clear(&mut self) {
        self.storage.clear();
    }
}

/// Estimate the byte size of an entry once serialized.
pub fn estimate_serialized_entry_size(entry: &DejavuEntry, centroid: &Hypervector) -> usize {
    // Header: weight(4) + creation_tick(8) + tag(1) = 13 bytes
    let header = 13;

    let vec = entry.reconstruct(centroid);
    let delta = vec.bitwise_xor(centroid);
    let set_bits = delta.count_ones();

    if set_bits > DELTA_CROSSOVER_BITS {
        // Raw: 1280 bytes
        header + 1280
    } else {
        // Delta: k(1) + num_indices(4) + data_len(2) + GR data
        let mean_gap = if set_bits > 0 {
            HD_DIMENSION as f64 / set_bits as f64
        } else {
            HD_DIMENSION as f64
        };
        let k = ((mean_gap * std::f64::consts::LN_2).log2().round() as u32).max(1).min(8);
        // Average bits per gap: unary(~log2(gap)) + k + 1 (terminator)
        let avg_bits_per_gap = (mean_gap.log2().ceil() as u32 + k + 1) as f64;
        let total_bits = set_bits as f64 * avg_bits_per_gap;
        let gr_data_bytes = (total_bits / 8.0).ceil() as usize;
        header + 1 + 4 + 2 + gr_data_bytes
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bloom_filter_basic() {
        let mut bf = CountingBloomFilter::new(1_000_000, 4);
        bf.insert("https://example.com/page1");
        bf.insert("https://example.com/page2");
        assert!(bf.maybe_contains("https://example.com/page1"));
        assert!(bf.maybe_contains("https://example.com/page2"));
        // False positives possible but unlikely with 2 items in 1M bits
        assert!(!bf.maybe_contains("https://never-visited.com"));
    }

    #[test]
    fn test_bloom_filter_clear() {
        let mut bf = CountingBloomFilter::new(1_000_000, 4);
        bf.insert("test");
        bf.clear();
        assert!(!bf.maybe_contains("test"));
    }

    #[test]
    fn test_capped_vecdeque() {
        let mut v = CappedVecDeque::new(3);
        v.push_back(1);
        v.push_back(2);
        v.push_back(3);
        assert_eq!(v.len(), 3);
        v.push_back(4); // should evict 1
        assert_eq!(v.len(), 3);
        assert_eq!(v.get(0), Some(&2));
        assert_eq!(v.get(2), Some(&4));
        assert_eq!(v.pop_front(), Some(2));
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn test_sparse_accumulator_basic() {
        let mut sa = SparseAccumulator::new(10);
        assert_eq!(sa.get(0), 0);
        sa.add(0, 5);
        assert_eq!(sa.get(0), 5);
        sa.add(0, 3);
        assert_eq!(sa.get(0), 8);
        assert_eq!(sa.get(1), 0); // untouched
    }

    #[test]
    fn test_sparse_accumulator_from_dense() {
        // Create a dense vector where 99% of values are the same (0),
        // and only 1% are non-zero.  This gives high sparsity.
        let mut dense = vec![0u32; 10240];
        for i in 0..102 { // ~1% of entries are non-zero
            dense[i * 100] = 42;
        }
        let sa = SparseAccumulator::from_dense(&dense, 50);
        assert!(sa.sparsity() > 0.95, "sparsity was {}", sa.sparsity()); // highly sparse
        let reconstructed = sa.to_dense();
        for i in 0..10240 {
            assert_eq!(reconstructed[i], dense[i], "Mismatch at index {}", i);
        }
    }

    #[test]
    fn test_sparse_accumulator_decay() {
        let mut sa = SparseAccumulator::new(10);
        sa.add(0, 100);
        sa.add(1, 200);
        assert_eq!(sa.deltas.len(), 2);
        sa.decay(0.5);
        // After 0.5 decay: 100→50, 200→100
        assert_eq!(sa.get(0), 50);
        assert_eq!(sa.get(1), 100);
    }

    #[test]
    fn test_sparse_accumulator_memory_smaller() {
        let mut sa = SparseAccumulator::new(100);
        // Simulate ~10% sparsity
        for i in 0..1024 {
            sa.add(i * 10, 42);
        }
        let sparse_bytes = sa.memory_bytes();
        let dense_bytes = 10240 * 4; // 40 KB
        assert!(
            sparse_bytes < dense_bytes / 2,
            "Sparse should be < half of dense: {} vs {}",
            sparse_bytes,
            dense_bytes
        );
    }
}
