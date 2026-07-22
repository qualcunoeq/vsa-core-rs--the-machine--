# Architecture Overview

**Version:** v3.4 (July 2026)
**Last Updated:** 2026-07-22

This document provides a bird's-eye view of The Machine's architecture — how the pieces fit together, what each module does, and where to find the relevant documentation.

---

## The Big Picture

The Machine is a **two-timescale stochastic iterated function system** on 10,240-bit binary hypervectors. The system's core loop:

```
sensory input → encode → project through clusters → reason → act → absorb → maintain
```

It has **no neural networks, no gradients, no LLM inference**. Only XOR, popcount, rotation, and majority bundling.

---

## Module Dependency Map

```
                        ┌──────────────────┐
                        │    main.rs       │  Agent loop, telemetry, scheduling
                        │  (multi-agent)   │
                        └────────┬─────────┘
               ┌─────────────────┼────────────────────┐
               ▼                 ▼                    ▼
     ┌─────────────────┐ ┌──────────────┐ ┌──────────────────┐
     │  forager.rs     │ │  broker.rs   │ │  socket.rs       │
     │  Web crawling   │ │  Consensus   │ │  Admin interface │
     └────────┬────────┘ └──────┬───────┘ └──────────────────┘
              │                 │
              ▼                 ▼
     ┌─────────────────────────────────────────────────────┐
     │  reason.rs (DeepThought)                             │
     │  Forward chaining, soft projection, theorem tests    │
     └──────────┬────────────────────────────────┬──────────┘
                ▼                                ▼
     ┌──────────────────┐              ┌──────────────────┐
     │  qa.rs           │              │  narrative.rs    │
     │  Question        │              │  Pure rule-based │
     │  answering       │              │  NLG engine      │
     └────────┬─────────┘              └──────────────────┘
              │
     ┌────────┴────────────────────────────────────────────┐
     │  lib.rs                                              │
     │  Hypervector, MemoryCluster, accumulator, telemetry │
     │  Hot/cold memory management, entry merging          │
     └────────┬────────────────────────────────────────┬───┘
              │                                        │
     ┌────────┴────────┐              ┌────────────────┴──────┐
     │  compression.rs │              │  drift.rs             │
     │  Bloom filter   │              │  DMU, DCP, Homeostasis│
     │  SparseAccum    │              │  CognitiveMode, PSC   │
     │  Cold storage   │              │  GlobalWorkspace, etc │
     └─────────────────┘              └───────────────────────┘
```

## Layer Architecture

The system is organized into 6 research layers (see `docs/ROADMAP.md` for full detail):

| Layer | Focus | Key Modules | Status |
|-------|-------|-------------|--------|
| **0** | Bitwise substrate | `lib.rs`, `reason.rs`, `compression.rs`, `hnsw.rs` | **Stable** — core VSA, projection, telemetry, memory compression |
| **1** | Memory & concepts | `abstractor.rs`, `hierarchy.rs`, `sleep.rs`, `cognition.rs` | **Active** — concept journal, quality scoring, abstraction metrics |
| **2** | Reasoning & explanation | `qa.rs`, `analogy.rs`, `narrative.rs`, `reason.rs` | **Active** — QA traces, episode provenance, NLG |
| **3** | Self-model & adaptation | `self_model.rs`, `drift.rs`, `diagnostic.rs`, `abstraction_learner.rs` | **Active** — confidence calibration, homeostatic regulation |
| **4** | Tools & world interfaces | `action.rs`, `actuator.rs`, `sensory.rs`, `forager.rs` | **Stable** — tool registry, reliability tracking, audit events |
| **5** | Bounded autonomy | `monitor.rs`, `defense.rs`, `workspace.rs`, `meta_reasoning.rs` | **Active** — autonomy budgets, decision journals, sandbox |
| **6** | Governed evaluation | `governed_benchmark.rs`, `strategic_route_benchmark.rs`, `algebra*.rs`, etc. | **Stable** — 7-tier suite, receipt verification, ablations |

## Data Flow

### Fast Path (every ~10 ticks)
```
World State → Dissonance Check → DeepThought Forward Chaining
  → Intent Selection → Action Execution → Epistemic Update
```

### Slow Path (every ~50 ticks)
```
Cluster Decay → κ_P Measurement → Tripwire Check
  → Entry Merging → Episode Store Persistence → Contraction Report
```

### Maintenance Path (every ~100 ticks)
```
Hot/Cold Memory Sweep → Cluster Compaction → Memory Profiler
```

## Formal Verification Structure

Every theorem in `MATH.md` (3,353 lines) is guarded by **31 assumptions (A1–A31)** which define the operating envelope. The five load-bearing beams:

| Assumption | What it says | How it's enforced |
|------------|-------------|-------------------|
| A1 | Bounded drift (r < 0.35) | `test_drift_magnitude_ewma` |
| A2 | Centroid separation (≥ 0.30) | Compactor invariant (merge/fission) |
| A3 | Rotation decorrelation | `enforce_a3q_manifold()` admission gate |
| A4 | Cleanup oracle (≥ 0.56) | Resonator threshold in QA engine |
| A5 | Feedback reliability (p > 0.5) | `test_a5_adversarial_reward_noise` |

### Key Theorems

| # | Statement | Proof Method |
|---|-----------|-------------|
| I.1 | Centroid fixed point | Algebraic (GF(2)) |
| I.2-R | Decay-aware plasticity | Lemma D1 + margin argument |
| II.1 | Cluster count bounded | LSH pigeonhole |
| III.1 | O(1) memory | Structural bound |
| XXI.1 | Unique invariant measure | Banach fixed point |
| XXII.1-R | L_F ≤ 1.0 (tight) | Bit-wise case analysis |
| XXIII.1 | Tracking error ≤ 0.70 | Novelty gate invariant |
| XXV.4 | Uniform spectral gap | λ₂(P)·κ_F < 1 via A3-Q |
| XXV.5 | Sub-Lemma S surjectivity | Constructive witness |

## Memory Architecture (v3.3+)

```
┌─────────────────────────────────────────────────────────┐
│                  MEMORY STACK                            │
├──────────┬───────────────────────┬───────────────────────┤
│ Layer    │ What                  │ Solution              │
├──────────┼───────────────────────┼───────────────────────┤
│ L0       │ visited URLs          │ CountingBloomFilter   │
│          │ seed_urls             │ CappedVecDeque        │
│          │ doc_frequency         │ Exp. decay + evict    │
│ L1       │ accumulator Vec<u32>  │ SparseAccumulator     │
│ L2       │ cluster entries       │ Age-weighted merge    │
│ L3       │ frozen clusters       │ Delta + Golomb-Rice   │
│ Monitor  │ all layers            │ MemorySnapshot/250t   │
└──────────┴───────────────────────┴───────────────────────┘
```

## DRIFT Cognitive Subsystems

Ten subsystems ported into `src/drift.rs`:

| Subsystem | What it does | When it runs |
|-----------|-------------|-------------|
| DMU Scoring | Memory retrieval salience | HNSW search |
| CognitiveMode | 3-bit [M,S,N] mode tag | Every cycle |
| DCP Consensus | Propose→vote→resolve | Broker round |
| Homeostasis | 7-need regulation | Subconscious loop |
| PSC Predictor | Chaos-aware trend prediction | Periodic |
| Global Workspace | GWT salience ranking | Attention cycle |
| Emotional Field | Emotion⊗Stance→Mood | Narrative generation |
| Context Engine | Fork/merge hypotheses | Reasoning |
| Implicit Intuition | Pattern bundling | Recognition |
| Shadow/Enantiodromia | Archetype oscillation | Long-term dynamics |

## Index of Key Files

### Core VSA
| File | Description |
|------|-------------|
| `src/lib.rs` | Hypervector, MemoryCluster, accumulator, telemetry, constitution |
| `src/reason.rs` | DeepThought, forward chaining, soft_project, theorem tests |
| `src/resonator.rs` | LSH-cached cleanup, vocabulary management |
| `src/hnsw.rs` | Approximate nearest-neighbor search with DMU scoring |

### Memory
| File | Description |
|------|-------------|
| `src/compression.rs` | Bloom filter, SparseAccumulator, entry merging, cold storage |
| `src/sleep.rs` | Consolidation, pruning, concept freezing |
| `src/hierarchy.rs` | Multi-level projection, L2 abstraction |
| `src/abstractor.rs` | Community detection, concept formation |

### Reasoning
| File | Description |
|------|-------------|
| `src/qa.rs` | Question answering, causal-chain, fact verification |
| `src/narrative.rs` | Pure rule-based NLG (no ML/LLM) |
| `src/analogy.rs` | Role-frame induction, analogical prediction |
| `src/temporal.rs` | Transition statistics, temporal prediction |

### Formalization & Benchmarks
| File | Description |
|------|-------------|
| `src/formalization.rs` | Typed direct instantiation, prose grammar |
| `src/proposition.rs` | Trusted proposition kernel, theorem checking |
| `src/recurrence.rs` | First-order affine recurrence solving |
| `src/algebra*.rs` | Linear/quadratic/system equation executors |
| `src/expression_*.rs` | Expression evaluation and simplification |
| `src/kernel.rs` | Formal kernel operations |
| `src/governed_benchmark.rs` | Unified 7-tier evaluation suite |

### Autonomy & Safety
| File | Description |
|------|-------------|
| `src/action.rs` | Tool registry, intent decoding |
| `src/actuator.rs` | Action execution, budget enforcement |
| `src/defense.rs` | Threat detection, port rotation |
| `src/monitor.rs` | State monitoring, telemetry |

### DRIFT Cognitive Subsystems
| File | Description |
|------|-------------|
| `src/drift.rs` | All 10 subsystems (DMU, CognitiveMode, DCP, Homeostasis, etc.) |
| `src/broker.rs` | Neocortex broker, DCP consensus |
| `src/self_model.rs` | Self-state tracking, mode transitions |
| `src/context.rs` | Context management, state tracking |

### Infrastructure
| File | Description |
|------|-------------|
| `src/main.rs` | Multi-agent simulation, agent loop |
| `src/forager.rs` | Web crawling, document processing |
| `src/socket.rs` | Admin socket interface |
| `src/cognition.rs` | Episodes, ConceptJournal, ConfidenceCalibration |
| `src/diagnostic.rs` | Failure classification, structural SVO centroids |

### Documentation
| File | Description |
|------|-------------|
| `MATH.md` | Complete formal mathematical specification (3,353 lines) |
| `README.md` | Project overview, getting started |
| `GUIDE.md` | Developer guide with flowcharts and code examples |
| `CURRENT_STATE.md` | Current project state and priorities |
| `docs/ROADMAP.md` | Research layers and near-term work |
| `docs/CLAIMS.md` | Claim ledger with evidence and failure modes |
| `docs/EVALUATION.md` | Capability matrix and experiment taxonomy |
| `docs/ARCHITECTURE.md` | This file — architectural overview |

## Quick Command Reference

```bash
# Default test suite (~1980 tests)
cargo test --lib

# Focused test areas
cargo test --lib qa::tests
cargo test --lib reason::tests
cargo test --lib narrative::tests

# Research benchmarks (ignored by default)
cargo test --lib -- --ignored

# Soft projection calibration
cargo test --lib reason::tests::test_soft_projection_frontier_sweep -- --ignored --nocapture

# Verification scripts
python3 prove_decay_plasticity.py
python3 prove_adversarial_Lf.py
python3 verify_dynamics.py

# Multi-agent simulation
cargo run

# Governed reasoning evaluation
cargo run --release --bin governed_bench -- 500 500 42 results/governed_bench/large.jsonl
```

## Version History

| Version | Date | Key Additions |
|---------|------|---------------|
| v1.0 | 2025 | Core VSA, accumulator, basic reasoning |
| v2.0 | 2025 | Multi-agent, consensus, decay |
| v2.5 | 2026 | L_F correction, decay theorems, telemetry |
| v3.0 | Apr 2026 | Soft projection, spectral gap, answer open questions |
| v3.1 | Jun 2026 | Soft projection fix, ρ-admissible, Sub-Lemma S, tracking theorems |
| v3.2 | Jun 2026 | Intervention test, structural SVO centroids, A21 resolution |
| v3.3 | Jul 2026 | Memory compression (L0–L3), DRIFT 10-subsystem port |
| v3.4 | Jul 2026 | Governed reasoning verticals, formalization audit, 7-tier evaluation |
