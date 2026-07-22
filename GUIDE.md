# The Machine — Developer Guide

**Applies to:** v3.4 (July 2026)
**Formal spec:** [`MATH.md`](./MATH.md) (3,353 lines)
**Test count:** ~1,980 `#[test]` items

This guide bridges the formal mathematics (`MATH.md`) and the Rust implementation. It provides:
1. **Visual flowcharts** of the core pipelines
2. **Concrete code examples** showing how the key abstractions work in practice
3. **Optimal τ guide** for soft projection calibration (v3.1 corrected)

---

## Part 1: Visualizing the Architecture

### 1.1 Full System Data Flow

```
                    ┌─────────────────────────────────────────────────────┐
                    │                 WORLD / ENVIRONMENT                 │
                    └──────────────────┬──────────────────────────────────┘
                                       │ sensory data (telemetry, text, network)
                                       ▼
                    ┌─────────────────────────────────────────────────────┐
                    │              SENSORY ENCODERS                       │
                    │  (SystemTelemetryModality, TextSensoryModality,     │
                    │   NetworkTrafficModality)                           │
                    │  Each produces a 10240-bit hypervector             │
                    └──────────────────┬──────────────────────────────────┘
                                       │ bound_role(v_role_market, market_state)
                                       │ bound_role(v_role_news, news_state)
                                       │ bound_role(v_role_infra, infra_state)
                                       ▼
                    ┌─────────────────────────────────────────────────────┐
                    │           WORLD STATE (bundled)                     │
                    │  current_world_state = bundle(market, news, infra)  │
                    └──────────────────┬──────────────────────────────────┘
                                       │
                        ┌──────────────┴──────────────┐
                        ▼                             ▼
           ┌────────────────────────┐     ┌──────────────────────────────┐
           │   Dissonance Check     │     │   DEEPTHOUGHT REASONING      │
           │   δ(world, baseline)   │     │   forward_chain_anchored()   │
            │   > 0.55 → pivot       │     │   τ = 0.10 soft projection   │
           └───────────┬────────────┘     │   (or τ = 0 hard projection) │
                       │                  └──────────────┬───────────────┘
                       │                                 │
                       │         ┌───────────────────────┘
                       │         ▼
           ┌──────────────────────────────────────────────────────────────┐
           │                  INTENT SELECTION                            │
           │  Argmax over: desirability, frequency, crisis override       │
           │  Winner gets dispatched to action pipeline                   │
           └──────────────────────────┬───────────────────────────────────┘
                                      │
                                      ▼
           ┌──────────────────────────────────────────────────────────────┐
           │              ACTION EXECUTION                                │
           │  forager updates crawl target, admin socket sends command    │
           └──────────────────────────┬───────────────────────────────────┘
                                      │ world changes
                                      ▼
                     ┌─────────────────────────────────────────────────────┐
                     │            EPISTEMIC UPDATE (broadcast)             │
                     │  All agents absorb the new world state:             │
                     │  cluster.absorb_entry(&new_world_state)             │
                     │  → returns (centroid_shift, input_distance)         │
                     │  → records κ_F in ContractionTelemetry              │
                     └──────────────────┬──────────────────────────────────┘
                                        │
                                        ▼
                     ┌─────────────────────────────────────────────────────┐
                     │         CLUSTER MAINTENANCE (every N ticks)         │
                     │  • Decay accumulator (γ = 0.975 every 50 ticks)    │
                     │  • Hot/cold memory sweep (every 100 ticks)         │
                     │  • κ_P measurement (20 random pairs, every 50)     │
                     │  • Tripwire check (κ = κ_P · κ_F < 0.995)         │
                     │  • Compaction (merge NHD < 0.30, fission > 0.70)   │
                     │  • Entry merging (age-weighted centroid collapse)  │
                     │  • Memory profiler snapshot (every 250 ticks)      │
                     └──────────────────┬──────────────────────────────────┘
                                        │
                                        ▼
                     ┌─────────────────────────────────────────────────────┐
                     │            NEXT CYCLE (tick + 1)                    │
                     └─────────────────────────────────────────────────────┘
```

### 1.2 The Composition Promotion Pipeline

This pipeline converts raw causal compositions into promoted, trusted rules.

```
  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
  │  Slot 0   │    │  Slot 1   │    │  Slot 2   │    │  Slot 3   │
  │  (seed)   │    │  (hop 1)  │    │  (hop 2)  │    │  (hop 3)  │
  └─────┬─────┘   └─────┬─────┘   └─────┬─────┘   └─────┬─────┘
        │               │               │               │
        └───────────────┴───────────────┴───────────────┘
                        │
                        ▼
        ┌───────────────────────────────────────┐
        │    COMPOSE ALL (algebraic chain)       │
        │    R_chain = R1 ⊕ ρ(R2) ⊕ ρ²(R3) ...  │
        │    ε(n) → 0.5 as n → ∞ (EXPANSIVE)    │
        └──────────────────┬────────────────────┘
                           │
                           ▼
        ┌───────────────────────────────────────┐
        │      DESIRABILITY CHECK                │
        │  1. δ(consequent, baseline) <          │
        │     δ(world_state, baseline)?           │
        │  2. σ(consequent, crisis_j) < 0.65     │
        │     for all crisis concepts?           │
        └──────────────────┬────────────────────┘
                           │
                   ┌────────┴────────┐
                   ▼                 ▼
           (undesirable)      (desirable)
                   │                 │
                   ▼                 ▼
           ┌────────────┐   ┌──────────────────────┐
           │  REJECT    │   │  FREQUENCY CHECK     │
           │  (discard) │   │  f_k ≥ 3 in W_win=5? │
           └────────────┘   └──────────┬───────────┘
                                       │
                              ┌────────┴────────┐
                              ▼                 ▼
                       (below threshold)  (above threshold)
                              │                 │
                              ▼                 ▼
                      ┌────────────┐   ┌──────────────────────┐
                      │  HOLD      │   │  PROMOTE             │
                      │  (counter  │   │  append_composed_rule│
                      │   +1)      │   │  → cluster storage   │
                      └────────────┘   └──────────────────────┘
```

The key insight: **algebraic composition is EXPANSIVE** (ε → 0.5), so promoted rules are immediately anchored through the cluster manifold before storage. The **forward_chain_anchored** path avoids this entirely by projecting intermediate states after EVERY hop.

### 1.3 The Accumulator Lifecycle (Per Cluster Per Bit)

```
  BIT START (a_i = 0, W = 0)
       │
       │ new cluster created with first observation
       ▼
  ┌─────────────────────────────────────────────────┐
  │  ABSORPTION LOOP                                │
  │                                                  │
  │  ┌──────────┐    ┌──────────┐    ┌──────────┐   │
  │  │ τ_i = 1  │    │ τ_i = 0  │    │ Hebbian  │   │
  │  │ a_i += 1 │    │ a_i += 0 │    │ a_i += c_i│   │
  │  │ W += 1   │    │ W += 1   │    │ W += 1   │   │
  │  └────┬─────┘    └────┬─────┘    └────┬─────┘   │
  │       │               │               │         │
  │       └───────────────┴───────────────┘         │
  │                       │                         │
  │                       ▼                         │
  │              ┌──────────────────┐               │
  │              │ Weight cap check │               │
  │              │ W > 500? → scale │               │
  │              │ a *= 500/W       │               │
  │              │ W = 500          │               │
  │              └────────┬─────────┘               │
  │                       │                         │
  │                       ▼                         │
  │              ┌──────────────────┐               │
  │              │ Recompute        │               │
  │              │ centroid:        │               │
  │              │ c_i = (a_i >     │               │
  │              │        floor(W/2))│              │
  │              └────────┬─────────┘               │
  │                       │                         │
  │                       ▼                         │
  │              ┌──────────────────┐               │
  │              │ Check margin     │               │
  │              │ m = a - floor(W/2)│              │
  │              │ m ≥ 1 → bit = 1  │               │
  │              │ m ≤ 0 → bit = 0  │               │
  │              └──────────────────┘               │
  └──────────────────────┬──────────────────────────┘
                         │
                    every 50 ticks
                         │
                         ▼
              ┌──────────────────────┐
              │  DECAY EVENT         │
              │  a = round(0.975 * a)│
              │  W = max(1,          │
              │      round(0.975*W)) │
              │  Recompute centroid  │
              │  Lemma D1:           │
              │  |m' - 0.975m| ≤ 1.5 │
              │  Theorem I.2-R.1:    │
              │  m ≥ 3 → can't flip  │
              └──────────────────────┘
```

### 1.4 Hot/Cold Memory Lifecycle

```
  ┌─────────────────────────────────────────────────────────────┐
  │                    ALL CLUSTERS                              │
  │  K clusters total, each with:                               │
  │    • centroid (1280 bytes) — ALWAYS live                    │
  │    • accumulator (40 KB dense / ~4 KB sparse) — only hot    │
  │    • last_access_tick — tracks recency                      │
  │    • entries (subject to age-weighted merging)              │
  └────────────────────────┬────────────────────────────────────┘
                           │
                           ▼
  ┌─────────────────────────────────────────────────────────────┐
  │              HOT/COLD SWEEP (every 100 ticks)                │
  │                                                              │
  │  For each cluster:                                           │
  │    if tick - last_access_tick > FREEZE_AFTER (default 500): │
  │      → serialize to ColdStorage (centroid-delta + Golomb-Rice)│
  │      → drop entries & accumulator                            │
  │      → cluster.is_hot() → false                              │
  │                                                              │
  │  Keep at most MAX_HOT (default 100) accumulators live.       │
  │  If more than 100 are hot, freeze the coldest.               │
  │                                                              │
  │  Memory (v3.3 with compression):                             │
  │    Hot:    100 × 4 KB  = 0.4 MB  (sparse accumulators)      │
  │    Hot:    100 × 1.3 KB = 0.13 MB (centroids)               │
  │    Cold:   900 × 0.2 KB = 0.18 MB (cold storage, compressed)│
  │    Total:                  0.71 MB  (for K=1000, compressed)│
  │    Bound (Thm III.1):     ~10.6 MB (for K=5120, worst case) │
  └────────────────────────┬────────────────────────────────────┘
                           │
              ┌────────────┴────────────┐
              ▼                         ▼
  ┌─────────────────────┐   ┌──────────────────────────┐
  │  HOT (accumulator   │   │  COLD (serialized)       │
  │  resident)          │   │                          │
  │                     │   │  On next access:         │
  │  • Full evidence    │   │  deserialize from storage│
  │    integration      │   │  reconstruct entries +   │
  │  • Can absorb       │   │  accumulator from        │
  │    new observations │   │  centroid-delta encoding │
  │  • Can flip bits    │   │  (Theorem XIII.1:        │
  │    via decay        │   │  reconstruction is       │
  └─────────────────────┘   │  centroid-preserving)    │
                            └──────────────────────────┘
```

### 1.5 Soft Projection Decision Flow

```
  ┌─────────────────────────────────────────────┐
  │           soft_project(x, clusters, τ)       │
  │                                              │
  │  if τ < 1e-12:                               │
  │    → hard projection (nearest centroid)      │
  │    return clusters[argmin δ(x, c_i)]         │
  │                                              │
  │  Compute d_i = δ(x, c_i) for all K centroids │
  │                                              │
  │  d_min = min(d_i)                            │
  │  for each centroid:                          │
  │    w_i = exp(-(d_i² - d_min²) / τ)           │
  │    NOTE: Corrected formula (v3.1)!           │
  │    Old (buggy): exp(-(d_i - min_d)² / τ)     │
  │    New (correct): exp(-(d_i² - min_d²) / τ)  │
  │                                              │
  │  Normalize: w_i /= Σ w_j                     │
  │                                              │
  │  For each of 10240 bits:                     │
  │    if Σ w_i · centroid_i[bit] > 0.5:         │
  │      output[bit] = 1                         │
  │    else: output[bit] = 0                     │
  │                                              │
  │  return output                               │
  └─────────────────────────────────────────────┘

  TAU GUIDE (v3.1 corrected, frontier sweep with 800 pairs/2000 queries):
    τ = 0.00    → hard projection (4.32 bits, C_eff=20,      κ_P ≈ 0.970)
    τ = 0.08    → CONSERVATIVE (9.58 bits, C_eff=120× gain,  κ_P ≈ 0.932)
    τ = 0.10    → OPTIMUM (10.58 bits, C_eff=2554=128×,     κ_P ≈ 0.916)
    τ = 0.12    → HIGH CAPACITY (11.32 bits, C_eff=128×,     κ_P ≈ 0.898)
    τ > 0.50    → MUSH (outputs blend to centroid mean, κ_P < 0.19)
    
  NOTE (v3.1): The numerical stability transform was corrected from
  exp(-(d - min_d)²/τ) to exp(-(d² - min_d²)/τ). The old formula
  over-weighted distant centroids by exp(2·min_d·(d - min_d)/τ),
  making τ=0.030 appear optimal. The true optimum is τ=0.10.
```

### 1.6 LSH Routing + Anchor-Through-Clusters

```
  ┌──────────────────────────────────────────────────────────┐
  │  anchor_through_clusters_with_threshold(x, clusters, θ)  │
  │                                                          │
  │  sector_q = lsh_sector(x)   ← 10-bit LSH (1024 sectors) │
  │                                                          │
  │  ┌─────────────────────────────────────────────┐         │
  │  │ PHASE 1: Sector prefilter                   │         │
  │  │ For each cluster:                           │         │
  │  │   if lsh_sector(cluster.anchor) == sector_q:│         │
  │  │     check δ(x, cluster.centroid)            │         │
  │  │ O(K/1024) expected comparisons              │         │
  │  └─────────────────────┬───────────────────────┘         │
  │                        │                                 │
  │                  ┌─────┴─────┐                           │
  │                  ▼           ▼                           │
  │           (found match)  (no good match)                 │
  │                  │           │                            │
  │                  │           ▼                            │
  │                  │   ┌───────────────────────────┐        │
  │                  │   │ PHASE 2: Full scan        │        │
  │                  │   │ Check ALL remaining       │        │
  │                  │   │ clusters (O(K) worst case)│        │
  │                  │   └───────────┬───────────────┘        │
  │                  │               │                        │
  │                  └───────┬───────┘                        │
  │                          ▼                                │
  │              ┌──────────────────────┐                     │
  │              │ best_sim ≥ threshold? │                    │
  │              └──────┬───────────────┘                     │
  │                  ┌──┴──┐                                  │
  │                  ▼     ▼                                  │
  │           (snap to    (return input                      │
  │           centroid)   unchanged)                          │
  └──────────────────────────────────────────────────────────┘
```

### 1.7 Compaction (Merge / Fission)

```
  ┌──────────────────────────────────────────────────────────┐
  │              COMPACTOR (runs every T_comp = 50 ticks)    │
  │                                                          │
  │  For each pair of clusters (i, j):                       │
  │                                                          │
  │    if δ(c_i, c_j) ≤ 0.30:                                │
  │      ┌──────────────────────────────────────┐            │
  │      │  MERGE                                │            │
  │      │  c_new = bundle(c_i, c_j)            │            │
  │      │  W_new = W_i + W_j                   │            │
  │      │  ΔΦ = -0.30 / W_total (W₁ contracts) │            │
  │      └──────────────────────────────────────┘            │
  │                                                          │
  │    if max_pairwise_NHD(cluster) > 0.70:                  │
  │      ┌──────────────────────────────────────┐            │
  │      │  FISSION                             │            │
  │      │  Split entries into two sub-clusters │            │
  │      │  Recompute centroids                 │            │
  │      │  ΔΦ = +0.35 / W_total (W₁ expands)   │            │
  │      └──────────────────────────────────────┘            │
  │                                                          │
  │  Net: ΔW₁ ≈ (0.1 - 3.0·p_merge + 3.5·p_fission         │
  │            + 1.0·p_novel) / W_total                      │
  │  For stationary inputs: p_merge dominates → κ ≈ 0.925    │
  └──────────────────────────────────────────────────────────┘
```

---

## Part 2: Practical Code Examples

### 2.1 Creating Hypervectors and Basic VSA Operations

```rust
use the_machine::Hypervector;

// Random 10240-bit hypervector (50% density)
let v1 = Hypervector::new_random();

// All-zero and all-ones vectors
let zero = Hypervector::new_zero();
let ones = Hypervector::new_ones();

// Binding: A ⊕ B (XOR) — invertible, used for role-filler pairs
let bound = v1.bitwise_xor(&zero);  // == v1

// Bundling: majority rule — not invertible, used for sets
let bundle = Hypervector::bundle(&[&v1, &ones]);  // mostly ones

// Constitutional bundling: deterministic, order-independent tiebreaking
let constitution = Hypervector::new_random();
let ordered = Hypervector::bundle_with_constitution(&[&v1, &zero], &constitution);
let reversed = Hypervector::bundle_with_constitution(&[&zero, &v1], &constitution);
assert_eq!(ordered, reversed);  // always true

// Rotation: used for sequence encoding and variable binding
let rotated = v1.rotate_left(13);  // cyclic shift by 13 positions

// Distance
let d = v1.normalized_hamming_distance(&ones);  // ≈ 0.5 for random v1
```

### 2.2 Initializing a MemoryCluster with `ensure_accumulator`

```rust
use the_machine::{MemoryCluster, Hypervector, HD_DIMENSION};

// Create a cluster from scratch
let centroid = Hypervector::new_random();
let mut cluster = MemoryCluster {
    centroid,
    anchor: centroid,  // Locked Anchor — set once, never changes
    entries: Vec::new(),
    reverberation: 1.0,
    last_reinforced_tick: 0,
    accumulator: Vec::new(),  // empty = frozen
    total_weight: 10,         // we know this cluster has 10 observations
    last_access_tick: 0,
};

// On first use after deserialization: reconstruct the accumulator
// Theorem XIII.1 guarantees this produces a valid accumulator:
//   A_i > floor(W/2)  iff  c_i = 1
cluster.ensure_accumulator();
assert!(!cluster.accumulator.is_empty());    // now 10240 elements
assert_eq!(cluster.accumulator.len(), HD_DIMENSION);

// After ensure_accumulator, the centroid is a fixed point:
let reconstructed_centroid = cluster.centroid;
for i in 0..HD_DIMENSION {
    let a = cluster.accumulator[i];
    let threshold = cluster.total_weight / 2;
    let bit_from_acc = (a > threshold) as u8;
    let word = cluster.centroid.bits[i / 64];
    let bit_from_centroid = ((word >> (i % 64)) & 1) as u8;
    assert_eq!(bit_from_acc, bit_from_centroid);
}
```

### 2.3 Absorbing an Observation (with Telemetry)

```rust
use the_machine::{MemoryCluster, Hypervector};

let mut cluster = /* ... initialized cluster ... */;
let observation = Hypervector::new_random();

// Before absorption: record the centroid
let centroid_before = cluster.centroid;
let input_dist = centroid_before.normalized_hamming_distance(&observation);

// Absorb: acc += observation, W += 1, recompute centroid
// Returns (centroid_shift, input_distance) for κ_F telemetry
let (centroid_shift, returned_input_dist) = cluster.absorb_entry(&observation);

assert!((returned_input_dist - input_dist).abs() < 1e-10);

// Local κ_F estimate for this absorption:
let kappa_f_sample = if input_dist > 1e-10 {
    1.0 - centroid_shift / input_dist
} else {
    1.0
};
// κ_F ≈ 0.95 for well-entrenched clusters (small centroid shift)
// κ_F ≈ 0.50 for fragile clusters (large centroid shift)
```

### 2.4 Using Soft Projection vs Hard Projection

```rust
use the_machine::{Hypervector, MemoryCluster};
use the_machine::reason::soft_project;

let clusters: Vec<MemoryCluster> = /* ... */;
let query = Hypervector::new_random();

// HARD projection (τ = 0) — nearest centroid
let hard = soft_project(&query, &clusters, 0.0);
// Equivalent to: anchor_through_clusters(&query, &clusters)

// SOFT projection at optimum (τ = 0.10, v3.1 corrected)
// 128× capacity multiplier, κ_P ≈ 0.916
let soft = soft_project(&query, &clusters, 0.10);

// The soft output is a weighted blend of all centroids (no top-3 truncation)
// Near Voronoi boundaries, it can be a stable hybrid state
// that the hard projection would have snapped to a single centroid
```

### 2.5 Running the Contraction Telemetry

```rust
use the_machine::ContractionTelemetry;

let mut telemetry = ContractionTelemetry::new();

// After each absorption:
let (shift, input_dist) = cluster.absorb_entry(&obs);
telemetry.record_kappa_f(shift, input_dist);

// Every 50 ticks, measure κ_P from random pairs:
// (this is done automatically by VSABrain::measure_kappa_p)
// telemetry.record_kappa_p(d_before, d_after);

// Check the tripwire:
if let Some(warning) = telemetry.check_tripwire(current_tick) {
    eprintln!("JOINT CONTRACTION WARNING: {}", warning);
    // Possible outputs:
    //   "WARNING: Joint contraction κ = 0.9973 approaching threshold 0.995..."
    //   "CRITICAL: Joint contraction κ = 1.0023 ≥ 1.001! System may diverge!"
}

// Get status report:
eprintln!("{}", telemetry.report());
// "κ_P=0.9694 (n=120), κ_F=0.9501 (n=500), κ=0.9210, κ_max=0.9502"
```

### 2.6 The Full Reasoning Cycle (Simplified)

```rust
use the_machine::{Hypervector, MemoryCluster, VSABrain, HD_DIMENSION};
use the_machine::reason::DeepThought;

let mut brain = VSABrain::new(0.43);
let mut dt = DeepThought::new(4, vocab, &brain);

// 1. Build world state from telemetry
let mut telemetry = HashMap::new();
telemetry.insert("vix_zscore".to_string(), 0.5);
telemetry.insert("move_zscore".to_string(), 0.2);
let world_state = brain.compile_state_vector(&telemetry);

// 2. Enable soft projection at optimum (v3.1 corrected)
//    NOTE: The numerical stability transform was fixed from
//    exp(-(d-min_d)²/τ) to exp(-(d²-min_d²)/τ). The true optimal is τ=0.10.
brain.soft_projection_tau = 0.10;

// 3. Project world state through clusters
let projected = brain.project_through_clusters(&world_state);

// 4. Run anchored forward chaining
let clusters_snapshot = brain.dejavu_clusters.clone();
let (intent, slot, trace, desirable) = dt.reason(
    &projected,
    &subjects, &verbs, &objects,
    &clusters_snapshot,
    &historical_baseline,
    &crisis_concepts,
).await;

// 5. If desirable and frequent enough, promote
if desirable {
    brain.contraction_telemetry.check_tripwire(tick);
}

// 6. Periodic κ_P measurement (every 50 ticks)
if tick % 50 == 0 {
    brain.measure_kappa_p(20);
    eprintln!("{}", brain.contraction_telemetry.report());
}
```

### 2.7 Decay Mechanics (Verifying the Proof)

```rust
use the_machine::{MemoryCluster, Hypervector, ACCUMULATOR_DECAY_FACTOR};

// Simulate a bit with margin m = 3 (the proven safety threshold)
let mut cluster = /* ... */;
cluster.accumulator[0] = 52;  // a_i
cluster.total_weight = 100;    // W, threshold = floor(100/2) = 50
cluster.recompute_centroid();  // 52 > 50 → centroid bit = 1

// Apply decay (simulating ACCUMULATOR_DECAY_INTERVAL = 50 ticks)
cluster.decay_accumulator(ACCUMULATOR_DECAY_FACTOR);

// After decay: a_i' = round(0.975 × 52) = 51
//              W'    = round(0.975 × 100) = 98
//              threshold' = floor(98/2) = 49
//              51 > 49 → centroid bit STILL 1
// Theorem I.2-R.1: m ≥ 3 before decay → bit cannot flip
assert_eq!(cluster.centroid_bit(0), 1);
```

---

## Part 3: Memory Compression Architecture (v3.3)

### L0 — Online Caches

The forager's `visited: HashSet<String>` is replaced by a **Counting Bloom filter** (32M bits, ~4 MB, 6 hash functions). False positives mean we skip an unvisited page — harmless for a crawler.

`seed_urls` is capped at 50,000 entries via `CappedVecDeque`. `doc_frequency` uses exponential decay (×0.85 every 200 docs) to bound vocabulary size.

### L1 — Sparse Accumulator

`SparseAccumulator` stores only indices where the accumulator value differs from the default. For a typical cluster with ~10% non-zero bits:

| Storage | Size |
|---------|------|
| Dense `Vec<u32>` | 40,960 bytes |
| `SparseAccumulator` | ~4,096 bytes |

**10× reduction.**

### L2 — Entry Merging

Age-weighted centroid collapse is triggered when entry count exceeds `MergeConfig.trigger_count` (default: 600). Entries are partitioned into three cohorts — Young (< 50 ticks, preserved verbatim), Middle (50–500, coherence-guarded), Old (> 500, merged unconditionally). The coherence guard bisects incoherent groups via VSA k-means.

### L3 — Cold Storage Serialization

When a cluster is frozen, it's serialized using centroid-delta + Golomb-Rice encoding:

- If > 8% bits differ from centroid → raw (1,280 bytes)
- If ≤ 8% bits differ → delta-encoded with optimal Rice parameter (~200 bytes typical)

**~6× reduction for cold entries.**

---

## Summary: Key Files Reference

| File | Lines | Purpose |
|------|-------|---------|
| `src/lib.rs` | 7123 | Core VSA types (Hypervector, MemoryCluster, ContractionTelemetry) |
| `src/reason.rs` | ~5000+ | Reasoning engine (DeepThought, forward_chain, soft_project) |
| `src/drift.rs` | 2291 | 10 DRIFT cognitive subsystems |
| `src/compression.rs` | 1311 | Memory compression (Bloom filter, sparse accumulator, entry merging) |
| `src/cognition.rs` | ~2000 | Episodes, ConceptJournal, ConfidenceCalibration, AblationConfig |
| `src/qa.rs` | ~1500 | QA engine, causal-chain reasoning, fact verification |
| `src/diagnostic.rs` | ~1200 | Failure classification, structural SVO centroids |
| `src/narrative.rs` | ~1000 | Pure rule-based NLG, morphology, dependency linearization |
| `src/main.rs` | ~800 | Multi-agent simulation, agent loop, telemetry |
| `src/broker.rs` | ~700 | NeocortexBroker, DCP consensus, quorum selection |
| `MATH.md` | 3353 | Complete formal specification |
| `prove_decay_plasticity.py` | — | Monte Carlo verification of I.2-R flip bounds |
| `prove_adversarial_Lf.py` | — | Construction of L_F = 1.0 worst case |
