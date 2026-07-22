# Current State

**Last Updated:** 2026-07-22

## Project Identity

The Machine is a **provably stable, mathematically verified autonomous cognitive architecture** built on hyperdimensional computing (HDC/VSA). It uses 10,240-bit binary hypervectors with XOR, rotation, and majority bundling as its only primitives. **No neural networks. No gradients. No LLM inference.**

The crate exposes:
- **Core VSA engine** (`src/lib.rs`): Hypervector, MemoryCluster, accumulator dynamics, ContractionTelemetry, hot/cold memory management
- **Reasoning engine** (`src/reason.rs`): Forward chaining, soft projection, anchored composition
- **QA engine** (`src/qa.rs`): Question answering, causal-chain reasoning, fact verification
- **Narrative generator** (`src/narrative.rs`): Pure rule-based NLG (no ML, no LLMs)
- **Diagnostic system** (`src/diagnostic.rs`, `src/abstraction_learner.rs`): Self-extending failure classification
- **Formalization stack** (`src/formalization.rs`, `src/proposition.rs`, `src/recurrence.rs`): Typed direct instantiation, proposition proofs, recurrence solving
- **Governed reasoning** (`src/governed_benchmark.rs`, `src/strategic_route_benchmark.rs`, `src/capability_planner.rs`): Seven-tier evaluation with receipt verification
- **Algebraic/equation solving** (`src/linear_equation.rs`, `src/quadratic_equation.rs`, `src/linear_system.rs`, `src/expression_evaluation.rs`): Executor-based equation solving
- **Drift cognitive subsystems** (`src/drift.rs`): Port of 10 subsystems — DMU scoring, CognitiveMode, DCP consensus, Homeostasis, PSC Predictor, Global Workspace, Emotional Field, Context Engine, Implicit Intuition, Shadow/Enantiodromia
- **Memory compression** (`src/compression.rs`): Counting Bloom filter, sparse accumulator, entry merging, cold storage serialization
- **Multi-agent simulation** (`src/main.rs`): Broker + agents with self-narrative, n-gram prediction, sleep cycles
- **17 binary targets** for experiments, benchmarks, and validation

### Key Version History

| Version | Date | Key Changes |
|---------|------|-------------|
| v3.0 | Apr 2026 | Soft projection, Theorem XXV.4 spectral gap, L_F correction |
| v3.1 | Jun 2026 | Numerical stability fix (soft projection formula), ρ-admissible invariant, Sub-Lemma S closure, tracking error theorems, coreference chain |
| v3.2 | Jun 2026 | Intervention test: A21 resolved via structural SVO centroids (3/3 correct vs 1/3 with hand-coded tables) |
| v3.3 | Jul 2026 | Memory compression (L0–L3), sparse accumulator, cold storage, Bloom filter |
| v3.4 | Jul 2026 | Governed reasoning verticals (recurrence, proposition, algebra benchmarks), formalization audit, 7-tier unified evaluation |

## Verification Commands

### Fast Library Tests
```bash
cargo test --lib              # All default tests (~1980 #[test] items)
cargo test --lib qa::tests    # QA engine tests (44+)
cargo test --lib reason::tests  # Reasoning engine tests
```

### Ignored / Research Tests
```bash
cargo test --lib -- --ignored              # All research benchmarks
cargo test --lib reason::tests::test_soft_projection_frontier_sweep -- --ignored --nocapture
```

### Binary Targets
```bash
cargo run --release --bin governed_bench -- 500 500 42 results/governed_bench/large.jsonl
cargo run --release --bin algebra_bench -- data/algebra_seed_v1.json results/algebra_bench/seed.jsonl
cargo run --release --bin recurrence_bench -- 500 42 results/recurrence_bench/large.jsonl
cargo run --release --bin proposition_bench -- 500 42 results/proposition_bench/large.jsonl
cargo run --release --bin strategic_route_bench -- --scale medium --seed 42
cargo run --release --bin cognition_bench -- --case all --scale small --seed 42
cargo run --release --bin formalization_baseline -- data/formalization_seed_v1.json results/formalization_seed_report.json
cargo run --release --bin intervention_test     # Zero-overlap analogy test
cargo run --release --bin concept_composition_bench -- 5 results/concept_composition.json
```

### Python Verification Scripts
```bash
python3 prove_decay_plasticity.py    # Theorem I.2-R flip bounds
python3 prove_adversarial_Lf.py      # L_F = 1.0 worst-case construction
python3 verify_dynamics.py           # Dynamical systems verification
```

## Test Boundaries

| Category | Count | Type | Deterministic |
|----------|-------|------|---------------|
| Default library tests | ~1980 #[test] items | Fast invariant checks | ✅ (seeded RNG) |
| Ignored benchmarks | ~50+ | Long-running calibration | ✅ (seeded RNG) |
| Flaky tests (pre-existing) | 1 | reason.rs `thread_rng()` | ❌ (~18% failure) |

**Key principle:** Default tests are deterministic, local, and fast. Ignored tests are research or integration benchmarks requiring datasets or long validation loops.

## Mathematical Verification Status

The formal specification (`MATH.md`, 3,353 lines) contains **proven theorems** with three levels of rigor:

| Status | Count | Meaning |
|--------|-------|---------|
| **PROVEN** | 25+ | Algebraic identity or Banach fixed point (no assumptions beyond GF(2)) |
| **EMPIRICALLY CONSISTENT** | 15+ | Observed across test suites, not formally proven |
| **DEPENDENT** | 4 | Proven under stated assumptions (A1–A31 contracts) |
| **CALIBRATED/MEASURED** | 6 | Empirically measured with documented parameters |

### Key Proven Guarantees

| Guarantee | Theorem | Value |
|-----------|---------|-------|
| Memory is bounded w.r.t. time | III.1 | ~10.6 MB maximum at K=5120 |
| Cluster count is bounded | II.1 | K ≤ 5120 |
| Unique invariant measure exists | XXI.1 | Banach fixed point from κ < 1 |
| System mixes exponentially | XXVI.2 | d_TV ≤ 0.01 within 77 cycles |
| Adversary cannot break contraction | XXII.1-R | L_F ≤ 1.0 (tight), margin = 0.010 |
| Tracking error never exceeds 0.70 | XXIII.1 | min_c δ(v_t, c) ≤ 0.70 always |
| Spectral gap < 1 (A3-Q manifolds) | XXV.4 | λ₂(P)·κ_F < 1 for admissible manifolds |
| Sub-Lemma S (surjectivity) | XXV.5 | g = nearest∘P_τ surjects from ρ²⁶(W_i) |

## What's Running in a Typical Agent Loop

The main agent loop (`main.rs`) orchestrates:

```
Every tick (~2s):
  → Forager: crawl financial Wikipedia pages
  → Sensory intake → projection through clusters
  → DeepThought reasoning (forward chaining)
  → Intent selection → action dispatch
  → Epistemic update (broadcast to all agents)
  → Broker consensus (quorum/executor selection)

Every 50 ticks:
  → Accumulator decay (γ = 0.975)
  → κ_P measurement (20 random pairs)
  → Tripwire check (κ = κ_P · κ_F < 0.995)
  → Entry merging (age-weighted centroid collapse)
  → Episode store persistence
  → Contraction telemetry report

Every 100 ticks:
  → Hot/cold memory sweep
  → Cluster compaction (merge/fission)

Every 250 ticks:
  → Memory profiler snapshot
```

## N-gram Chain Prediction

Wired into the agent loop: `NgramChain` observes `CognitiveMode` transitions and predicts the next mode every 25 ticks.

## Admin Socket Commands

| Command | Example | Description |
|---------|---------|-------------|
| `ASK <question>` | `ASK Who raised rates?` | Answers question from stored facts |
| `STORE <sentence>` | `STORE The Fed raised rates.` | Stores fact from natural language |
| `FACTS` | `FACTS` | Shows fact + rule counts |
| `CHAIN <question>` | `CHAIN What happened after the Fed raised rates?` | Multi-hop chain reasoning |
| `STORE_RULE <rule>` | `STORE_RULE IF the_fed raise rates THEN yields rise` | Store causal rule |
| `SAVE` | `SAVE` | Persist QA memory to JSON file |
| `LOAD` | `LOAD` | Load QA memory from JSON file |

## Current Engineering Priorities

1. **Reduce compiler warnings** — ~80+ warnings from unused imports/variables in experimental modules; warning volume makes real regressions harder to spot
2. **Wire persistence** for `SelfModel` state, `ConfidenceCalibration` store, and `ConceptJournal` cold-storage events
3. **Run pre/post feedback integration test** using `TaskFamily` / `PrePostComparison`
4. **Extend governed evaluation** to method-acquisition layers when a `method_not_found` gap is demonstrated
5. **Keep documentation aligned with code** — README, geometry guides, and MATH.md should all reference same version and test counts

## Risks and Known Issues

| Risk | Severity | Status |
|------|----------|--------|
| Compiler warnings mask regressions | Medium | ~80+ warnings, gradual cleanup in progress |
| Flaky test (reason.rs, thread_rng) | Low | 1 test, ~18% failure rate |
| Zero-overlap analogy (A21) | Resolved v3.2 | SVO centroids fix: 3/3 correct |
| Joint contraction margin | Thin (0.010) | Monitored continuously via telemetry; tripwire at 0.995 |
| LSH collision at high K | Soft limit ~200 clusters | Phase 2 full-scan fallback handles overflow |
| MATH.md vs implementation drift | Low | Test coverage catches inconsistencies |

## Key Modules Reference

| Module | Lines | Purpose |
|--------|-------|---------|
| `lib.rs` | 7123 | Core VSA types, Hypervector, MemoryCluster, ContractionTelemetry, memory management |
| `reason.rs` | ~5000+ | DeepThought reasoning engine, soft_project, theorem tests |
| `drift.rs` | 2291 | 10 DRIFT cognitive subsystems (DMU, DCP, Homeostasis, etc.) |
| `compression.rs` | 1311 | Bloom filter, sparse accumulator, entry merging, cold storage |
| `cognition.rs` | ~2000 | Episodes, ConceptJournal, ConfidenceCalibration, AblationConfig |
| `qa.rs` | ~1500 | QA engine, causal-chain reasoning, fact verification |
| `diagnostic.rs` | ~1200 | Failure classification, structural SVO centroids |
| `narrative.rs` | ~1000 | Pure rule-based NLG, morphology, dependency linearization |
| `main.rs` | ~800 | Multi-agent simulation, agent loop, telemetry |
| `broker.rs` | ~700 | NeocortexBroker, DCP consensus, quorum selection |
