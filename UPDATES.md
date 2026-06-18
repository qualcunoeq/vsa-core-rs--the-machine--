# Memory Allocation Updates — v2.6 Layer 0–3 Compression

This document describes the memory-handling enhancements merged from
`the-machine-enhanced-memory-handling` into the main VSA core.  These
changes address **unbounded RAM growth** across the four memory layers
of The Machine.

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Layer 0 — Online Caches (Forager)](#2-layer-0--online-caches-forager)
3. [Layer 1 — Sparse Accumulator](#3-layer-1--sparse-accumulator)
4. [Layer 2 — Entry Merging](#4-layer-2--entry-merging)
5. [Layer 3 — Cold Storage Serialization](#5-layer-3--cold-storage-serialization)
6. [Transient Cluster Freeze/Thaw](#6-transient-cluster-freezethaw)
7. [Memory Profiler](#7-memory-profiler)
8. [Configuration Reference](#8-configuration-reference)

---

## 1. Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                  THE MACHINE MEMORY STACK                │
├──────────┬──────────────────────────┬────────────────────┤
│ Layer    │ What grows               │ Solution           │
├──────────┼──────────────────────────┼────────────────────┤
│ L0 (src) │ visited: HashSet<String> │ CountingBloomFilter│
│          │ seed_urls: Vec<String>   │ CappedVecDeque     │
│          │ doc_frequency: HashMap   │ Exp. decay+evict   │
│          │ transient_clusters       │ Hot/cold freeze    │
├──────────┼──────────────────────────┼────────────────────┤
│ L1 (lib) │ accumulator: Vec<u32>    │ SparseAccumulator  │
│          │ (40 KB per hot cluster)  │ (~4 KB avg)        │
├──────────┼──────────────────────────┼────────────────────┤
│ L2 (lib) │ entries per cluster      │ Age-weighted       │
│          │ (unbounded growth)       │ centroid collapse  │
├──────────┼──────────────────────────┼────────────────────┤
│ L3 (lib) │ frozen cluster data      │ Centroid-delta +   │
│          │ (RAM for cold clusters)  │ Golomb-Rice coding │
├──────────┼──────────────────────────┼────────────────────┤
│ Monitor  │ -                        │ Memory profiler    │
│          │                          │ tick (every 250)   │
└──────────┴──────────────────────────┴────────────────────┘
```

---

## 2. Layer 0 — Online Caches (Forager)

### 2.1 `visited` — Counting Bloom Filter

**File:** `src/compression.rs` → `CountingBloomFilter`

**Before:** `HashSet<String>` — grows linearly with every URL visited.  After crawling 1M pages, the set alone uses ~100 MB.

**After:** A **Counting Bloom filter** with 32 million bits (~4 MB) and 6 hash functions.  Memory is **fixed** regardless of how many URLs are visited.

```
BloomFilter(32M bits, 6 hashes)
  ↓
insert("https://...")   → sets 6 bits
maybe_contains("...")   → checks 6 bits (false positives < 0.1% at 1M items)
clear()                 → zeroes all bits (reuses allocation)
```

**False positive rate:** ~1% for up to 10M URLs, ~0.1% for 1M URLs.  A false positive means we *skip* a page we haven't actually visited — harmless for a web crawler.

**Persistence:** The Bloom filter is cleared when the forager resets its crawl history.  It's always rebuilt from scratch on restart, which matches the original `HashSet` behaviour.

### 2.2 `seed_urls` — Capped VecDeque

**File:** `src/compression.rs` → `CappedVecDeque<T>`

**Before:** `Vec<String>` — grows unbounded as curiosity targets generate search URLs.

**After:** A `CappedVecDeque<String>` with a hard cap of **50,000 entries** (~4 MB worst case).  When full, the oldest entry is evicted on each `push_back()`.

```rust
let queue = CappedVecDeque::new(50_000);
queue.push_back("https://...");  // evicts oldest if at capacity
queue.pop_front();               // removes oldest
```

### 2.3 `doc_frequency` — Exponential Decay

**File:** `src/forager.rs` → `VSAForager::step()`

**Before:** `HashMap<String, usize>` grows monotonically — every new word in every page adds an entry.  Over a long crawl this reaches millions of entries.

**After:** Every 200 documents, all entries are multiplied by a decay factor (0.85) and entries below a retain threshold (2) are evicted.  This bounds the HashMap size to approximately the unique vocabulary of the *most recent* 200–400 pages.

```rust
if total_documents % 200 == 0 {
    doc_frequency.retain(|_, count| {
        *count = (*count as f64 * 0.85).round() as usize;
        *count >= 2
    });
}
```

---

## 3. Layer 1 — Sparse Accumulator

**File:** `src/compression.rs` → `SparseAccumulator`

**Target:** `MemoryCluster.accumulator` — a `Vec<u32>` with 10,240 entries (40,960 bytes per hot cluster).

**Problem:** With 100 hot clusters, accumulators consume ~4 MB.  Most entries are 0 (bits never observed) or close to their default value.

**Solution:** Store only the indices where the accumulator value *differs from the default*.  For a typical cluster where ~10% of bits are non-zero:

| Storage | Size |
|---------|------|
| Dense `Vec<u32>` | 40,960 bytes |
| `SparseAccumulator` | ~4,096 bytes |

**10× reduction.**

### Key operations

```rust
let mut sa = SparseAccumulator::new(total_weight);

// Add an observation at dimension 42
sa.add(42, 1);           // O(log K) binary search

// Get current value
let val = sa.get(42);    // O(log K)

// Decay all deltas (age out old evidence)
sa.decay(0.975);         // prunes entries that decay to zero

// Reconstruct for centroid recomputation
let dense = sa.to_dense();  // O(D) — called only during merge/decay
```

### Integration in MemoryCluster

The `SparseAccumulator` is defined as a new type in `compression.rs` but the dense `Vec<u32>` in `MemoryCluster` is kept unchanged for backward compatibility.  The sparse encoding is applied during **cold serialization** (Layer 3) and the dense form is used for hot in-memory clusters.  A future upgrade can wire `SparseAccumulator` directly into `MemoryCluster` for additional in-memory savings.

---

## 4. Layer 2 — Entry Merging

**File:** `src/compression.rs` → `merge_entries()`, `vsa_bisect()`, `bundle_and_threshold()`

**Target:** `MemoryCluster.entries` — grows unbounded as observations accumulate.

**Problem:** Entries accumulate from every novelty-gate pass.  Without merging, a cluster that absorbs 100K observations holds 100K `DejavuEntry` objects (~200 MB).

**Solution:** Age-weighted centroid collapse triggered when entry count exceeds `MergeConfig.trigger_count` (default: 600).

### Algorithm

```
merge_entries(cluster, config, current_tick):
  1. Partition entries into three age cohorts:
     - Young  (age <  50 ticks)  → preserved verbatim
     - Middle (age 50–500 ticks) → coherence guard
     - Old    (age > 500 ticks)  → merge unconditionally

  2. OLD cohort → merge into ONE summary entry via majority-rule bundling
       bundle_and_threshold(entries, anchor) → (summary_hv, total_weight)

  3. MIDDLE cohort → check mean pairwise Hamming distance
       if mean_hamming < max_hamming_ratio (0.35):
         → merge into ONE summary entry (coherent cluster)
       else:
         → bisect via VSA k-means (two farthest seeds)
         → merge each sub-group separately

  4. Rebuild accumulator from merged entries:
       rebuild_accumulator_from_entries()
```

### Coherence Guard (`mean_hamming_within_cohort`)

Prevents merging dissimilar vectors into a single centroid.  If the mean pairwise NHD within a cohort exceeds 0.35, the cohort is **bisected** using VSA k-means:

1. Pick the **two most distant** vectors as initial centroids
2. Assign each vector to the nearer centroid
3. Merge each resulting group separately

This preserves semantic diversity while still compressing old observations.

### `rebuild_accumulator_from_entries()`

After merging entries, the accumulator must be rebuilt from scratch to correctly reflect the merged weights:

```rust
for entry in &cluster.entries {
    let vec = entry.reconstruct(&cluster.anchor);
    for (i, acc) in accumulator.iter_mut().enumerate() {
        let bit = (vec.bits[i / 64] >> (i % 64)) & 1;
        *acc += bit * entry.weight;  // weighted by merge count
    }
}
```

### Integration in Agent Loop

```rust
// In main.rs agent subconscious loop — every 50 ticks:
if ticker % 50 == 0 {
    let config = MergeConfig::default();
    for cluster in &mut brain.dejavu_clusters {
        let removed = merge_entries(cluster, &config, ticker as u64);
        if removed > 0 { log!("merged {} entries", removed); }
    }
}
```

---

## 5. Layer 3 — Cold Storage Serialization

**File:** `src/compression.rs` → `ColdStorageManager`, `serialize_cold_cluster()`, `deserialize_cold_cluster()`, `encode_entry()`, `decode_entry()`

### 5.1 Adaptive Entry Encoding

Each entry is adaptively encoded as either **Raw** or **Delta+GolombRice**:

```
encode_entry(entry, centroid):
  δ = entry_vector ⊕ centroid         // differences from centroid
  set_bits = popcount(δ)

  if set_bits > 820 (8% of 10240):
    → Raw: store full 1280-byte hypervector
  else:
    → Delta+GolombRice:
      1. Collect set-bit indices: [i₁, i₂, ..., iₙ]
      2. Choose optimal Rice parameter k ≈ log₂(ln(2)·mean_gap)
      3. Encode gaps via Golomb-Rice:
           gap = idxₙ - idxₙ₋₁
           quotient  → unary code
           remainder → k-bit binary
      4. Typical size: ~200 bytes for 640 set bits
```

**Saving:** For entries near the centroid (the common case), ~200 bytes instead of 1,280 → **~6× reduction**.

### 5.2 Cold Cluster Serialization Format

```
┌─────────────────────────────────────────┐
│ u32: magic (0x4D414348 "MACH")          │
│ u32: version (1)                        │
│ u16: num_entries                        │
├─────────────────────────────────────────┤
│ [u8; 1280]: centroid (raw)              │
├─────────────────────────────────────────┤
│ u32: total_weight                       │
│ u32: num_nonzero_accumulator_entries    │
│ for each: u16 index + u32 value         │
├─────────────────────────────────────────┤
│ for each entry:                         │
│   u32: weight                           │
│   u64: creation_tick                    │
│   u8:  tag (0=Raw, 1=Delta+GR)         │
│   if Raw:   [u8; 1280] hypervector      │
│   if Delta: u8(k) + u32(count) +        │
│             u16(data_len) + [u8; data]  │
└─────────────────────────────────────────┘
```

### 5.3 ColdStorageManager

Stores serialized cluster data in a `HashMap<usize, Vec<u8>>` keyed by cluster index:

```rust
pub struct ColdStorageManager {
    storage: HashMap<usize, Vec<u8>>,
}

// Freeze: serialize cluster, clear entries & accumulator
let serialized = serialize_cold_cluster(cluster);
cold_storage.store(idx, serialized);
cluster.entries.clear();
cluster.accumulator.clear();

// Thaw: deserialize on write access
if cold_storage.contains(idx) {
    if let Some(data) = cold_storage.take(idx) {
        if let Some(thawed) = deserialize_cold_cluster(&data) {
            cluster.entries = thawed.entries;
            cluster.accumulator = thawed.accumulator;
        }
    }
}
```

### 5.4 Integration in `freeze_cold_clusters()`

The existing hot/cold memory management (`VSABrain::freeze_cold_clusters()`) is upgraded to **serialize to cold storage** instead of just clearing the accumulator:

```rust
// Before (v2.5): just clear accumulator
cluster.freeze();

// After (v2.6): serialize all data first
let serialized = serialize_cold_cluster(cluster);
cluster.entries.clear();
cluster.entries.shrink_to_fit();
cluster.accumulator.clear();
self.cold_storage.store(idx, serialized);
```

On any write access (`add_to_dejavu_db`, `absorb_epistemic_update`), the cluster is automatically thawed from cold storage.

---

## 6. Transient Cluster Freeze/Thaw

**File:** `src/lib.rs` → `TransientCluster.{frozen, last_access_tick}`, `VSABrain::freeze_cold_transient_clusters()`

**Problem:** Transient clusters accumulate entries during web crawling.  After thousands of crawl steps, inactive transient clusters hold 100s of entry vectors in memory.

**Solution:** Add `frozen: bool` and `last_access_tick: u64` to `TransientCluster`.  A periodic sweep freezes clusters that haven't been accessed in `staleness_threshold` ticks by dropping their entry vectors (preserving the centroid for matching).

```rust
// Freeze cold transient clusters
for cluster in &mut transient_clusters {
    if !cluster.frozen
        && current_tick - cluster.last_access_tick > staleness_threshold
    {
        cluster.frozen = true;
        cluster.entries.clear();
        cluster.entries.shrink_to_fit();
    }
}
```

On the next `add_transient_fact()` that matches a frozen cluster, it's automatically thawed:

```rust
if best_sim >= cluster_threshold {
    let cluster = &mut transient_clusters[idx];
    cluster.frozen = false;          // thaw
    cluster.last_access_tick = tick;
    cluster.entries.push(entry);      // resume normal operation
    // ...
}
```

A combined convenience method `freeze_and_decay_transients()` handles freeze, then decay in one call.

---

## 7. Memory Profiler

**File:** `src/compression.rs` → `MemorySnapshot`, `log_memory_snapshot()`

**Integration:** In `main.rs`, every 250 agent loop iterations (~500 seconds at 2s/tick):

```rust
if ticker % 250 == 0 {
    log_memory_snapshot(&MemorySnapshot {
        dejavu_clusters:    brain.dejavu_clusters.len(),
        hot_clusters:       brain.clusters.iter().filter(|c| c.is_hot()).count(),
        cold_clusters:      brain.clusters.len() - hot,
        transient_clusters: brain.transient_clusters.len(),
        total_entries:      brain.entries().sum(),
        total_accumulator_kb: hot * 40.96,  // 40 KB per dense accumulator
        visited_urls_approx: forager_visited.approx_count(),
        seed_queue_len:     seed_urls.len(),
        doc_frequency_entries: doc_freq.len(),
        experiences_len:    brain.experiences.len(),
        broker_clusters:    broker.dejavu_clusters.len(),
    });
}
```

Produces output like:
```
[MEMORY] clusters: 45 (hot: 12, cold: 33) | transient: 8 |
entries: 2847 | accumulators: 491.5 KB | visited: ~234,101 |
seeds: 12 | doc_freq: 3,401 | experiences: 129
```

---

## 8. Configuration Reference

| Parameter | Default | Location | Description |
|-----------|---------|----------|-------------|
| `BloomFilter.num_bits` | 32,000,000 | `compression.rs` | Total bits in Bloom filter |
| `BloomFilter.num_hashes` | 6 | `compression.rs` | Hash functions per lookup |
| `MAX_SEED_URLS` | 50,000 | `forager.rs` | Capped seed queue limit |
| `DOC_FREQ_DECAY_INTERVAL` | 200 | `forager.rs` | Docs between doc_frequency decay |
| `DOC_FREQ_MIN_RETAIN` | 2 | `forager.rs` | Min doc_freq to keep after decay |
| `DELTA_CROSSOVER_BITS` | 820 (8%) | `compression.rs` | Raw vs Delta encoding crossover |
| `GOLOMB_RICE_DEFAULT_K` | 3 | `compression.rs` | Default Rice parameter |
| `MergeConfig.trigger_count` | 600 | `compression.rs` | Entry count that triggers merge |
| `MergeConfig.young_tick_threshold` | 50 | `compression.rs` | Young cohort age boundary |
| `MergeConfig.old_tick_threshold` | 500 | `compression.rs` | Old cohort age boundary |
| `MergeConfig.max_hamming_ratio` | 0.35 | `compression.rs` | Coherence guard threshold |
| `MergeConfig.min_cohort_size` | 3 | `compression.rs` | Minimum size to merge |
| `staleness_threshold` (transient) | 500 | `main.rs` | Ticks before transient freeze |
| `staleness_threshold` (dejavu) | 500 | `main.rs` | Ticks before cold storage freeze |
| `max_hot` | 100 | `main.rs` | Max hot clusters kept in RAM |
| profiler interval | 250 | `main.rs` | Ticks between memory snapshots |
| merge interval | 50 | `main.rs` | Ticks between entry merging |

---

# DRIFT Cognitive Architecture Port — 10 Subsystems

This section documents the port of 10 cognitive subsystems from the **DRIFT**
(formerly infj-bot) project by **timeless-hayoka**
([git@github.com:timeless-hayoka/infj-bot.git](git@github.com:timeless-hayoka/infj-bot.git))
into The Machine's binary hypervector framework.

All subsystems are in `src/drift.rs`.

---

## 1. DMU Scoring — Decision Making Utility

**Source:** `core/unified_memory.py`, `memory/dmu.py`

Ebbinghaus-decay × reinforcement × contextual salience scoring for memory
retrieval.  Three presets calibrated for different memory types.

```
DMU = exp(-t / τ) × R × S × (1 - d)

τ     = tau_base × (1 + κ × log(1 + reps + salience × 10))
R     = 1 + α × log(1 + β × salience × reps)
```

| Preset | `tau_base` | Best for |
|--------|------------|----------|
| `dmu_params_episodic()` | 50 | Recent events, fast decay |
| `dmu_params_semantic()` | 150 | Facts, slow decay |
| `dmu_params_bond()` | 250 | Emotional bonds, very slow decay |

**Integration:** Used by `HnswIndex::search_with_dmu()` in `src/hnsw.rs`.

---

## 2. CognitiveMode — 3-Bit Continuity Vector

**Source:** `core/continuity_vector.py`

A compact [Memory, State, Novelty] tag with 8 named patterns.  Each mode
modulates HNSW search breadth and resonator depth.

| Mode | Bits | HNSW ef mult. | Resonator depth |
|------|------|---------------|-----------------|
| QUIET | [0,0,0] | 0.8× | 15 |
| COMPANION | [1,0,0] | 1.0× | 25 |
| REGULATED | [0,1,0] | 0.7× | 20 |
| EXPLORER | [0,0,1] | 1.5× | 30 |
| TASK | [1,1,0] | 1.0× | 25 |
| RESONANT | [1,0,1] | 1.3× | 30 |
| FRONTIER | [0,1,1] | 1.2× | 28 |
| FULL_COUNCIL | [1,1,1] | 1.4× | 35 |

Each mode has a deterministic hypervector via `to_hypervector()` / `from_hypervector()`.

---

## 3. DCP Consensus — Distributed Cognition Protocol

**Source:** `hive_mind/`

Propose → vote → resolve protocol for multi-agent sectors.

| Role | Weight | Function |
|------|--------|----------|
| Primary | 4 | Runs factorization, proposes results |
| Critic | 3 | Adversarial checker, votes against hallucinations |
| Backup | 2 | Fault-tolerant shadow |
| Observer | 0 | Telemetry only, no vote |

Resolution uses weighted-majority bundling across all votes.

**Integration:** `NeocortexBroker` in `src/broker.rs` has a `dcp_consensus` field.

---

## 4. Homeostasis — 7-Need Cybernetic Regulation

**Source:** `core/homeostasis.py`

Tracks seven cognitive needs with setpoints, allostatic prediction, crisis
detection, and regulation strategies.

| Need | Setpoint | Critical | Drift (idle) | Drift (active) |
|------|----------|----------|--------------|----------------|
| ENERGY | 0.80 | < 0.15 | +0.005 | -0.020 |
| COHERENCE | 0.75 | < 0.20 | +0.002 | -0.010 |
| INTEGRATION | 0.70 | < 0.15 | +0.001 | +0.005 |
| CONNECTION | 0.60 | < 0.10 | -0.005 | +0.015 |
| GROWTH | 0.50 | < 0.05 | -0.003 | +0.010 |
| AUTONOMY | 0.65 | < 0.10 | +0.001 | -0.008 |
| INTEGRITY | 0.85 | < 0.20 | -0.001 | -0.005 |

**Regulation strategies:**
- `ConserveEnergy` → depth=10, ef=20, no curiosity, skip non-essential
- `SeekCoherence` → depth=30, ef=50, full processing
- `PromoteGrowth` → depth=35, ef=60, max curiosity=3
- `Rest` → depth=5, ef=10, skip everything non-essential

**Integration:** Wired into the agent subconscious loop in `main.rs`.

---

## 5. PSC Predictor — Predictive State Characterization

**Source:** `core/psc_scaled.py`

Adaptive-horizon trend prediction using HD similarity between successive
state vectors.  Chaos score dynamically shortens the horizon when the
signal is erratic.

```
chaos = mean(Hamming(s[t], s[t-1])) over rolling buffer
horizon = horizon_base × (1 - chaos × 2)
```

**Key insight:** In HDC terms, chaos = 1 - cosine similarity between
adjacent state hypervectors.  This replaces the DRIFT original's per-dimension
OLS regression with a single HD operation that naturally captures the
system's global stability.

**Integration:** Can slot into the agent loop alongside the existing
resonator network for adaptive prediction.

---

## 6. Global Workspace — Competitive Salience Ranking

**Source:** `core/global_workspace.py`

A Global Workspace Theory (GWT) attention mechanism using HD similarity:

1. Contents are submitted with a source label (deduplication by source)
2. Each cycle, all contents are scored against the current context query
3. Age decay is applied (`decay_factor^age`)
4. Contents are assigned to tiers: **spotlight** (top 1), **active** (next 3),
   **preconscious** (remaining above threshold), **archived** (below threshold)

This replaces the DRIFT original's SQLite-backed salience tracking with
pure HD vector operations — no database needed.

---

## 7. Emotional Field — Emotion⊗Stance → Mood Binding

**Source:** `core/emotional_field.py`

A 28-entry associative memory mapping 7 emotions × 4 stances to 8 moods
using HD binding.  Each rule is stored as a key-value pair:

```
key   = emotion_HV ⊗ stance_HV
value = mood_HV
```

Querying unbinds the query emotion⊗stance and finds the nearest mood by
HD similarity.  This replaces the DRIFT original's Python dict lookup
with a proper HD associative memory that supports approximate matching.

---

## 8. Context Engine — Fork/Merge Superposition

**Source:** `core/context_engine.py`

Forking creates N hypothesis contexts by perturbing the current context
with noise vectors.  Merging selects the hypothesis closest to a cue
vector via Hamming similarity.

```rust
let branches = fork_context(&current, 3);
let best = merge_contexts(&branches, &cue_hv);
```

This is the HDC equivalent of the DRIFT original's comonadic fork/merge
pattern, implemented as pure hypervector operations.

---

## 9. Implicit Intuition — Pattern Recognition via Bundled HVs

**Source:** `core/intuition.py`

Learns implicit patterns by bundling domain tag hypervectors.  Each
observation reinforces (or creates) a pattern signature.  Recognition
fires when input similarity exceeds a threshold (`min_examples` required).

```rust
engine.observe("pattern", &["tag1", "tag2", "tag3"]);
let matches = engine.recognize(&input_hv);
engine.prune(2); // remove patterns below strength 2
```

Replaces the DRIFT original's SQLite-backed `implicit_patterns` table
with an HD associative memory.

---

## 10. Shadow / Enantiodromia — Bipolar Archetype Oscillation

**Source:** `core/shadow.py`

Six archetypes (Hero, Shadow, Sage, Trickster, Caregiver, Orphan) with
enantiodromia: when one archetype dominates, charge accumulates in its
opposite, eventually causing a reversal.

```
if dominant.intensity > threshold:
    opposite.charge += rate
if opposite.charge > reversal_threshold:
    swap 50% intensity from dominant to opposite
```

Each archetype has a deterministic hypervector.  The shadow state can
be encoded into a bundle for binding into the cognitive state.

---

## Credits

| Enhancement | Source | Author |
|------------|--------|--------|
| Memory compression (L0–L3) | `the-machine-enhanced-memory-handling` | **qualcunoeq** |
| DMU scoring, CognitiveMode, DCP, Homeostasis | `infj-bot` / `DRIFT` | **timeless-hayoka** |
| PSC Predictor, Global Workspace, Emotional Field | `infj-bot` / `DRIFT` | **timeless-hayoka** |
| Context Engine, Implicit Intuition, Shadow | `infj-bot` / `DRIFT` | **timeless-hayoka** |

Original repos:
- [qualcunoeq/the-machine-enhanced-memory-handling](git@github.com:qualcunoeq/the-machine-enhanced-memory-handling.git)
- [timeless-hayoka/infj-bot](git@github.com:timeless-hayoka/infj-bot.git)

---

## Files Changed

| File | Status | Lines |
|------|--------|-------|
| `src/compression.rs` | **NEW** | 1,311 |
| `src/lib.rs` | Modified | +176 |
| `src/forager.rs` | Modified | +47 |
| `src/main.rs` | Modified | +68 |
| `src/drift.rs` | **NEW** | 2,291 |
| `src/hnsw.rs` | Modified | +73 |
