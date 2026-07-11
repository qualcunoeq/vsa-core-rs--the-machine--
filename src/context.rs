// ─── Hierarchical Context Management (Keep-Recent + Fold) ──────────────────
//
// Implements Pattern 8 from GLM-5 (arXiv 2602.15763v2, §2.3):
// hierarchical context management with Keep-Recent and Fold.
//
// ## The Problem
//
// In long-running VSA systems, every observation (state, action, fact)
// produces a 10240-bit hypervector.  Keeping all of them at full resolution
// is O(N) memory.  Folding everything into one bundle is O(1) but loses
// granularity for recent items.
//
// ## The Pattern (Keep-Recent + Fold)
//
// - **Keep-Recent**: A sliding window of the last N items at full resolution.
//   N = RECENT_WINDOW_SIZE (32).  These are the items most likely to be
//   queried (locality of reference in cognitive systems).
//
// - **Fold**: Items older than N are compressed via VSA bundling (weighted
//   majority) into a bundle per chunk of size FOLD_CHUNK_SIZE (64).  The
//   bundle is a single 10240-bit hypervector that statistically represents
//   the cluster of items.  Retrieval from a fold is approximate (bundle
//   similarity), not exact.
//
// - **Unified Query**: `query(hv)` scans recent first (exact Hamming distance
//   on each item), then folded bundles (bundle similarity).  Returns the
//   closest match with a confidence score and an indicator of which tier
//   (Recent vs Folded) produced it.
//
// ## Reference
//
// GLM-5 §2.3 "Hierarchical Context Management":
//   "To reduce the computational overhead of the KV cache during long
//    sequences, we introduce a multi-resolution approach: Keep-Recent for
//    local context and Fold for global context.  The Fold operation
//    compresses contiguous older KV entries into a single representative
//    using an attention-weighted average."
//
// Our VSA analogue replaces attention-weighted average with bundled
// weighted majority (bundle_weighted from lib.rs), which is the natural
// VSA operation for combining multiple hypervectors into a single
// representative.
//
// ## Test Coverage
//
// 1. test_recent_buffer_push_pop  — bounded sliding window
// 2. test_folded_memory_bundle    — items are bundled correctly
// 3. test_hierarchical_query_recent_first — recent items match exactly
// 4. test_hierarchical_query_folded — folded items match approximately
// 5. test_hierarchical_eviction_folds — evicted items go to fold
// 6. test_hierarchical_recall_by_label — recall a specific labeled item
// 7. test_empty_query             — query on empty memory returns None
// 8. test_single_item             — single item in recent buffer

use crate::Hypervector;
use std::collections::VecDeque;

// ═══════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════

/// Number of recent items kept at full resolution.
/// 32 items × 160 u64 blocks × 8 bytes ≈ 41 KB per buffer instance.
pub const RECENT_WINDOW_SIZE: usize = 32;

/// Number of items to fold into a single bundle before creating a new chunk.
/// 64 items × bundle_weighted majority vote ≈ 3.2 ops per bit per fold.
pub const FOLD_CHUNK_SIZE: usize = 64;

/// Minimum similarity for a recent-buffer hit.
/// Random similarity for D=10240 is ~0.50, so 0.65 ensures genuine matches.
pub const RECENT_HIT_THRESHOLD: f64 = 0.65;

/// Minimum similarity for a folded-buffer hit.
/// Folded bundles are approximate, so threshold is lower.
pub const FOLD_HIT_THRESHOLD: f64 = 0.55;

// ═══════════════════════════════════════════════════════════════════════════
// RECENT ENTRY
// ═══════════════════════════════════════════════════════════════════════════

/// A single entry in the recent buffer at full resolution.
#[derive(Clone, Debug)]
pub struct RecentEntry {
    /// The hypervector at full 10240-bit resolution.
    pub hv: Hypervector,
    /// Human-readable label (e.g., "state_at_tick_142").
    pub label: String,
    /// System tick when this entry was added.
    pub tick: usize,
}

// ═══════════════════════════════════════════════════════════════════════════
// FOLDED CHUNK
// ═══════════════════════════════════════════════════════════════════════════

/// A folded/bundled representation of a chunk of older items.
#[derive(Clone, Debug)]
pub struct FoldedChunk {
    /// Bundled hypervector (weighted majority of all items in chunk).
    pub bundle: Hypervector,
    /// Tick range this chunk covers.
    pub tick_start: usize,
    pub tick_end: usize,
    /// Number of items folded into this chunk.
    pub count: usize,
    /// Labels for partial reconstruction (stored for debugging).
    pub labels: Vec<String>,
}

// ═══════════════════════════════════════════════════════════════════════════
// HIERARCHICAL MEMORY
// ═══════════════════════════════════════════════════════════════════════════

/// The combined query result from either tier.
#[derive(Clone, Debug)]
pub struct ContextQueryResult {
    /// The matched hypervector.
    pub hv: Hypervector,
    /// Similarity score (1 - NHD for recent, bundle similarity for folded).
    pub similarity: f64,
    /// Which tier produced the match.
    pub tier: ContextTier,
    /// Label of the matched entry or chunk.
    pub label: String,
    /// Tick of the matched entry (for recent) or midpoint (for folded).
    pub tick: usize,
}

/// Which tier of the hierarchy produced a match.
#[derive(Clone, Debug, PartialEq)]
pub enum ContextTier {
    /// Matched in the recent buffer (exact item).
    Recent,
    /// Matched in a folded chunk (approximate bundle).
    Folded,
}

/// Hierarchical context memory implementing Keep-Recent + Fold.
///
/// ## Architecture
///
/// ```text
///         query(hv)
///            │
///     ┌──────▼──────┐
///     │ RecentBuffer │  ← sliding window of N items, full resolution
///     │  (exact)     │
///     └──────┬───────┘
///            │ no hit?
///     ┌──────▼──────┐
///     │ FoldedMemory │  ← bundled chunks of older items
///     │ (approx.)    │
///     └──────┬───────┘
///            │ no hit?
///          return None
/// ```
#[derive(Clone, Debug)]
pub struct HierarchicalContextMemory {
    /// Recent buffer (full-resolution, sliding window).
    recent: VecDeque<RecentEntry>,
    /// Capacity of the recent buffer.
    recent_capacity: usize,
    /// Folded memory (bundled chunks of older items).
    folded: Vec<FoldedChunk>,
    /// Items accumulated since last fold flush.
    pending_fold: Vec<(Hypervector, String, usize)>,
    /// Maximum pending items before folding.
    fold_chunk_size: usize,
    /// Current system tick (incremented by caller or internally).
    tick: usize,
    /// Query hit statistics.
    pub recent_hits: usize,
    pub folded_hits: usize,
    pub misses: usize,
}

impl Default for HierarchicalContextMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl HierarchicalContextMemory {
    /// Create a new empty hierarchical context memory.
    pub fn new() -> Self {
        HierarchicalContextMemory {
            recent: VecDeque::with_capacity(RECENT_WINDOW_SIZE),
            recent_capacity: RECENT_WINDOW_SIZE,
            folded: Vec::new(),
            pending_fold: Vec::with_capacity(FOLD_CHUNK_SIZE),
            fold_chunk_size: FOLD_CHUNK_SIZE,
            tick: 0,
            recent_hits: 0,
            folded_hits: 0,
            misses: 0,
        }
    }

    /// Create with custom capacities.
    pub fn with_capacities(recent_capacity: usize, fold_chunk_size: usize) -> Self {
        HierarchicalContextMemory {
            recent: VecDeque::with_capacity(recent_capacity),
            recent_capacity,
            folded: Vec::new(),
            pending_fold: Vec::with_capacity(fold_chunk_size),
            fold_chunk_size,
            tick: 0,
            recent_hits: 0,
            folded_hits: 0,
            misses: 0,
        }
    }

    /// Increment the internal tick counter.
    pub fn tick(&mut self) {
        self.tick += 1;
    }

    /// Set the tick explicitly (for external tick synchronization).
    pub fn set_tick(&mut self, tick: usize) {
        self.tick = tick;
    }

    /// Push a new item into the hierarchical memory.
    ///
    /// If the recent buffer is full, the oldest item is evicted to the
    /// pending-fold buffer.  When the pending-fold buffer reaches
    /// `fold_chunk_size`, it is committed as a folded chunk.
    pub fn push(&mut self, hv: Hypervector, label: &str) {
        let entry = RecentEntry {
            hv,
            label: label.to_string(),
            tick: self.tick,
        };

        // If recent buffer is full, evict oldest to pending fold.
        if self.recent.len() >= self.recent_capacity {
            if let Some(evicted) = self.recent.pop_front() {
                self.pending_fold
                    .push((evicted.hv, evicted.label, evicted.tick));
            }
        }

        self.recent.push_back(entry);

        // If pending fold is full, commit it.
        if self.pending_fold.len() >= self.fold_chunk_size {
            self.commit_fold();
        }
    }

    /// Commit the pending fold buffer as a new folded chunk.
    fn commit_fold(&mut self) {
        if self.pending_fold.is_empty() {
            return;
        }

        let tick_start = self.pending_fold.first().map(|(_, _, t)| *t).unwrap_or(0);
        let tick_end = self.pending_fold.last().map(|(_, _, t)| *t).unwrap_or(0);
        let count = self.pending_fold.len();

        // Collect labels
        let labels: Vec<String> = self.pending_fold.iter().map(|(_, l, _)| l.clone()).collect();

        // Bundle via weighted majority (all weights = 1.0).
        // `bundle_weighted` does per-bit majority voting — no rotation
        // needed.  The resulting bundle is the centroid of the items,
        // so it maintains high similarity to each constituent.
        let refs: Vec<&Hypervector> = self
            .pending_fold
            .iter()
            .map(|(hv, _, _)| hv)
            .collect();
        let weights: Vec<f64> = vec![1.0; refs.len()];
        let bundle = if refs.is_empty() {
            Hypervector::new_zero()
        } else {
            Hypervector::bundle_weighted(&refs, &weights)
        };

        self.folded.push(FoldedChunk {
            bundle,
            tick_start,
            tick_end,
            count,
            labels,
        });

        self.pending_fold.clear();
    }

    /// Manually force a fold of the pending buffer.
    pub fn flush_fold(&mut self) {
        self.commit_fold();
    }

    /// Query the hierarchical memory for the closest match to `query`.
    ///
    /// 1. Scan recent buffer (exact NHD, full resolution).
    /// 2. If no recent hit above threshold, scan folded chunks (bundle similarity).
    /// 3. Return the best match with tier indicator, or None.
    pub fn query(&mut self, query: &Hypervector) -> Option<ContextQueryResult> {
        // Tier 1: Recent buffer (exact full-resolution scan)
        let mut best_recent: Option<ContextQueryResult> = None;
        let mut best_recent_sim = 0.0;

        for entry in &self.recent {
            let sim = 1.0 - query.normalized_hamming_distance(&entry.hv);
            if sim > best_recent_sim && sim >= RECENT_HIT_THRESHOLD {
                best_recent_sim = sim;
                best_recent = Some(ContextQueryResult {
                    hv: entry.hv.clone(),
                    similarity: sim,
                    tier: ContextTier::Recent,
                    label: entry.label.clone(),
                    tick: entry.tick,
                });
            }
        }

        if let Some(result) = best_recent {
            self.recent_hits += 1;
            return Some(result);
        }

        // Tier 2: Folded memory (bundle similarity, approximate)
        let mut best_folded: Option<ContextQueryResult> = None;
        let mut best_folded_sim = 0.0;

        for chunk in &self.folded {
            // For folded chunks, use normalized Hamming distance to the bundle
            let sim = 1.0 - query.normalized_hamming_distance(&chunk.bundle);
            if sim > best_folded_sim && sim >= FOLD_HIT_THRESHOLD {
                best_folded_sim = sim;
                best_folded = Some(ContextQueryResult {
                    hv: chunk.bundle.clone(),
                    similarity: sim,
                    tier: ContextTier::Folded,
                    label: format!(
                        "fold_[{}-{}]_n{}",
                        chunk.tick_start, chunk.tick_end, chunk.count
                    ),
                    tick: (chunk.tick_start + chunk.tick_end) / 2,
                });
            }
        }

        if let Some(result) = best_folded {
            self.folded_hits += 1;
            return Some(result);
        }

        self.misses += 1;
        None
    }

    /// Recall the most recent entry with a label containing `substring`.
    /// This is a convenience method for labeled retrieval.
    pub fn recall_by_label(&self, substring: &str) -> Option<&RecentEntry> {
        self.recent.iter().rev().find(|e| e.label.contains(substring))
    }

    /// Number of items in the recent buffer.
    pub fn recent_len(&self) -> usize {
        self.recent.len()
    }

    /// Number of folded chunks.
    pub fn folded_len(&self) -> usize {
        self.folded.len()
    }

    /// Number of pending items waiting to be folded.
    pub fn pending_len(&self) -> usize {
        self.pending_fold.len()
    }

    /// Total items tracked (recent + pending + folded_items).
    pub fn total_items(&self) -> usize {
        self.recent.len()
            + self.pending_fold.len()
            + self.folded.iter().map(|c| c.count).sum::<usize>()
    }

    /// Clear all memory.
    pub fn clear(&mut self) {
        self.recent.clear();
        self.folded.clear();
        self.pending_fold.clear();
        self.recent_hits = 0;
        self.folded_hits = 0;
        self.misses = 0;
    }

    /// Total queries processed.
    pub fn total_queries(&self) -> usize {
        self.recent_hits + self.folded_hits + self.misses
    }

    /// Hit rate across both tiers.
    pub fn hit_rate(&self) -> f64 {
        let total = self.total_queries();
        if total == 0 {
            return 0.0;
        }
        (self.recent_hits + self.folded_hits) as f64 / total as f64
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recent_buffer_push_pop() {
        let mut mem = HierarchicalContextMemory::with_capacities(10, 64);
        assert_eq!(mem.recent_len(), 0);

        for i in 0..10 {
            mem.push(Hypervector::new_random(), &format!("item_{}", i));
            mem.tick();
        }
        assert_eq!(mem.recent_len(), 10);

        // Push one more — oldest should be evicted to pending
        mem.push(Hypervector::new_random(), "item_10");
        assert_eq!(mem.recent_len(), 10);
        assert_eq!(mem.pending_len(), 1);
    }

    #[test]
    fn test_folded_memory_bundle() {
        let mut mem = HierarchicalContextMemory::with_capacities(5, 10);
        for i in 0..15 {
            mem.push(Hypervector::new_random(), &format!("item_{}", i));
            mem.tick();
        }
        // 5 in recent, 10 pending should trigger one fold
        assert_eq!(mem.recent_len(), 5);
        // 15 items total - 5 recent = 10 pending, but fold_chunk_size=10
        // so one fold should be committed
        assert!(mem.folded_len() >= 1);
    }

    #[test]
    fn test_hierarchical_query_recent() {
        let mut mem = HierarchicalContextMemory::with_capacities(10, 64);

        // Push some random items
        for i in 0..8 {
            mem.push(Hypervector::new_random(), &format!("distractor_{}", i));
            mem.tick();
        }

        // Push a target item
        let target = Hypervector::new_random();
        mem.push(target, "target");
        mem.tick();

        // Query with the target — should find it in recent
        let result = mem.query(&target);
        assert!(result.is_some(), "Target should be found in recent buffer");
        if let Some(r) = result {
            assert_eq!(r.tier, ContextTier::Recent, "Should hit recent tier");
            assert!(r.similarity > 0.99, "Exact match should have near-1 sim");
        }
    }

    #[test]
    fn test_hierarchical_query_folded() {
        // Folded memory needs CORRELATED vectors for the bundle to preserve
        // similarity.  Use a base vector and slightly perturbed copies.
        let mut mem = HierarchicalContextMemory::with_capacities(3, 4);
        let base = Hypervector::new_random();

        // Push 3 correlated items (base rotated by +1 bit each)
        for i in 0..3 {
            let rotated = base.rotate_left(i);
            mem.push(rotated, &format!("fill_{}", i));
            mem.tick();
        }

        // Target is the un-rotated base
        mem.push(base.clone(), "target");
        mem.tick();

        // Push 3 more correlated items to evict target into pending
        for i in 0..3 {
            let rotated = base.rotate_left(10 + i);
            mem.push(rotated, &format!("push_{}", i));
            mem.tick();
        }
        // pending should have 4 items now → auto-fold (fold_chunk_size=4)
        // target is in the fold alongside 3 correlated items

        // Push distractors (uncorrelated) to recent
        for _ in 0..3 {
            mem.push(Hypervector::new_random(), "distractor");
            mem.tick();
        }

        // Query with base — should find it in folded (bundle preserves cluster)
        let result = mem.query(&base);
        assert!(
            result.is_some(),
            "Target should be found in folded memory"
        );
        if let Some(r) = result {
            assert_eq!(r.tier, ContextTier::Folded);
            assert!(
                r.similarity > 0.50,
                "Folded similarity should be >0.50 for correlated bundle (got {:.4})",
                r.similarity
            );
        }
    }

    #[test]
    fn test_hierarchical_recall_by_label() {
        let mut mem = HierarchicalContextMemory::with_capacities(10, 64);
        for i in 0..5 {
            mem.push(Hypervector::new_random(), &format!("item_{}", i));
            mem.tick();
        }
        let recalled = mem.recall_by_label("item_3");
        assert!(recalled.is_some(), "Should find item_3 by label");
        assert_eq!(recalled.unwrap().label, "item_3");
    }

    #[test]
    fn test_empty_query() {
        let mut mem = HierarchicalContextMemory::new();
        let query = Hypervector::new_random();
        let result = mem.query(&query);
        assert!(result.is_none(), "Empty memory should return None");
        assert_eq!(mem.misses, 1);
    }

    #[test]
    fn test_single_item() {
        let mut mem = HierarchicalContextMemory::with_capacities(5, 64);
        let hv = Hypervector::new_random();
        mem.push(hv.clone(), "only");
        assert_eq!(mem.recent_len(), 1);
        let result = mem.query(&hv);
        assert!(result.is_some());
        assert_eq!(result.unwrap().label, "only");
    }

    #[test]
    fn test_clear() {
        let mut mem = HierarchicalContextMemory::with_capacities(5, 5);
        for i in 0..20 {
            mem.push(Hypervector::new_random(), &format!("i_{}", i));
            mem.tick();
        }
        assert!(mem.total_items() > 0);
        mem.clear();
        assert_eq!(mem.total_items(), 0);
        assert_eq!(mem.recent_len(), 0);
        assert_eq!(mem.folded_len(), 0);
    }

    #[test]
    fn test_hit_rate_tracking() {
        let mut mem = HierarchicalContextMemory::with_capacities(5, 64);
        let hv = Hypervector::new_random();
        mem.push(hv.clone(), "test");
        mem.query(&hv); // hit
        mem.query(&Hypervector::new_random()); // miss
        assert_eq!(mem.total_queries(), 2);
        assert!(mem.hit_rate() > 0.0);
        assert!(mem.hit_rate() < 1.0);
    }

    #[test]
    fn test_eviction_chain_preserves_order() {
        // Verify that FIFO eviction order is preserved when pushing many items.
        let mut mem = HierarchicalContextMemory::with_capacities(5, 64);
        let mut labels = Vec::new();

        for i in 0..10 {
            let hv = Hypervector::new_random();
            let label = format!("evict_test_{}", i);
            mem.push(hv, &label);
            labels.push(label);
            mem.tick();
        }

        // Recent should have items 5-9 (the last 5)
        assert_eq!(mem.recent_len(), 5);

        // The first 5 items should be in pending fold
        assert_eq!(mem.pending_len(), 5);

        // Verify recent items are the newest ones
        let recent_labels: Vec<String> = mem.recent.iter().map(|e| e.label.clone()).collect();
        for (i, label) in recent_labels.iter().enumerate() {
            assert_eq!(label, &format!("evict_test_{}", i + 5));
        }

        // Force fold and verify
        mem.flush_fold();
        assert_eq!(mem.pending_len(), 0);
        assert_eq!(mem.folded_len(), 1);
        assert_eq!(mem.folded[0].count, 5);
    }

    #[test]
    fn test_auto_fold_on_chunk_full() {
        // With recents=3 and fold_chunk=4, pushing 7 items should produce
        // 1 fold automatically.
        let mut mem = HierarchicalContextMemory::with_capacities(3, 4);
        for i in 0..7 {
            mem.push(Hypervector::new_random(), &format!("a_{}", i));
            mem.tick();
        }
        // 3 recent + 0 pending (auto-folded at 4)
        assert_eq!(mem.recent_len(), 3);
        assert_eq!(mem.pending_len(), 0);
        assert_eq!(mem.folded_len(), 1);
        assert_eq!(mem.folded[0].count, 4);
    }

    #[test]
    fn test_query_with_fold_hit_similarity() {
        // Two correlated items bundled should produce a centroid with
        // high similarity to both constituents.
        let mut mem = HierarchicalContextMemory::with_capacities(2, 3);
        let base = Hypervector::new_random();

        // Fill recent with correlated items
        mem.push(base.rotate_left(0), "fill_0");
        mem.tick();
        mem.push(base.rotate_left(1), "fill_1");
        mem.tick();

        // Push target (the base itself)
        mem.push(base.clone(), "target");
        mem.tick();

        // Push 2 more to evict target into pending
        mem.push(base.rotate_left(2), "push_0");
        mem.tick();
        mem.push(base.rotate_left(3), "push_1");
        mem.tick();

        // Force fold (3 correlated items + target in bundle)
        mem.flush_fold();

        // Push uncorrelated distractors to recent
        for _ in 0..3 {
            mem.push(Hypervector::new_random(), "distractor");
            mem.tick();
        }

        let result = mem.query(&base);
        assert!(result.is_some(), "Target should be found in folded");
        if let Some(r) = result {
            assert_eq!(r.tier, ContextTier::Folded);
            // Bundle of 4 correlated items should have decent similarity to base
            assert!(
                r.similarity > 0.45,
                "Folded similarity should be >0.45 for correlated bundle (got {:.4})",
                r.similarity
            );
        }
    }
}
