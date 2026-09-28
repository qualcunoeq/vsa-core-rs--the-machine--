# Architecture Overview

**Version:** v3.4
**Last updated:** 2026-09-28

This document maps how the pieces fit together, what each module does, and where to find the relevant documentation. For the mathematical specification see [`../MATH.md`](../MATH.md); for the environment and per-phase reference see [`DEVELOPMENT.md`](DEVELOPMENT.md).

## The big picture

The Machine is a two-timescale stochastic iterated function system on 10,240-bit binary hypervectors. The core loop:

```
sensory input -> encode -> project through clusters -> reason -> act -> absorb -> maintain
```

The only primitives are XOR, popcount, rotation, and majority bundling. There are no neural networks, no gradients, and no LLM inference.

## Module dependency map

```
                        +------------------+
                        |    main.rs       |  Agent loop, telemetry, scheduling
                        |  (multi-agent)   |
                        +--------+---------+
               +-----------------+--------------------+
               |                 |                    |
               v                 v                    v
     +-----------------+ +--------------+ +------------------+
     |  forager.rs     | |  broker.rs   | |  socket.rs       |
     |  Web crawling   | |  Consensus   | |  Admin interface |
     +--------+--------+ +------+-------+ +------------------+
              |                 |
              v                 v
     +-----------------------------------------------------+
     |  reason.rs (DeepThought)                             |
     |  Forward chaining, soft projection, theorem tests    |
     +----------+--------------------------------+----------+
                |                                |
                v                                v
     +------------------+              +------------------+
     |  qa.rs           |              |  narrative.rs    |
     |  Question        |              |  Rule-based      |
     |  answering       |              |  NLG engine      |
     +--------+---------+              +------------------+
              |
     +--------+--------------------------------------------+
     |  lib.rs                                              |
     |  Hypervector, MemoryCluster, accumulator, telemetry  |
     |  Hot and cold memory management, entry merging       |
     +--------+----------------------------------------+----+
              |                                        |
     +--------+--------+              +----------------+------+
     |  compression.rs |              |  drift.rs             |
     |  Bloom filter   |              |  DMU, DCP, Homeostasis|
     |  SparseAccum    |              |  CognitiveMode, PSC   |
     |  Cold storage   |              |  GlobalWorkspace, etc |
     +-----------------+              +-----------------------+
```

## Layer architecture

The system is organized into six research layers. See [`ROADMAP.md`](ROADMAP.md) for full detail.

| Layer | Focus | Key modules | Status |
|---|---|---|---|
| 0 | Bitwise substrate | `lib.rs`, `reason.rs`, `compression.rs`, `hnsw.rs` | Stable: core VSA, projection, telemetry, memory compression |
| 1 | Memory and concepts | `abstractor.rs`, `hierarchy.rs`, `sleep.rs`, `cognition.rs` | Active: concept journal, quality scoring, abstraction metrics |
| 2 | Reasoning and explanation | `qa.rs`, `analogy.rs`, `narrative.rs`, `reason.rs` | Active: QA traces, episode provenance, NLG |
| 3 | Self-model and adaptation | `self_model.rs`, `drift.rs`, `diagnostic.rs`, `abstraction_learner.rs` | Active: confidence calibration, homeostatic regulation |
| 4 | Tools and world interfaces | `action.rs`, `actuator.rs`, `sensory.rs`, `forager.rs` | Stable: tool registry, reliability tracking, audit events |
| 5 | Bounded autonomy | `monitor.rs`, `defense.rs`, `workspace.rs`, `meta_reasoning.rs` | Active: autonomy budgets, decision journals, sandbox |
| 6 | Governed evaluation | `governed_benchmark.rs`, `strategic_route_benchmark.rs`, `algebra*.rs` | Stable: seven-tier suite, receipt verification, ablations |

## Data flow

Fast path, every about 10 ticks:

```
World state -> dissonance check -> DeepThought forward chaining
  -> intent selection -> action execution -> epistemic update
```

Slow path, every about 50 ticks:

```
Cluster decay -> kappa_P measurement -> tripwire check
  -> entry merging -> episode store persistence -> contraction report
```

Maintenance path, every about 100 ticks:

```
Hot and cold memory sweep -> cluster compaction -> memory profiler
```

## Formal verification structure

Every theorem in [`../MATH.md`](../MATH.md) (3,452 lines) is stated inside an envelope of 31 assumptions (A1 to A31), each labelled as assumed, checked, or enforced. The five load-bearing assumptions:

| Assumption | Statement | Enforcement |
|---|---|---|
| A1 | Bounded drift (r < 0.35) | `test_drift_magnitude_ewma` |
| A2 | Centroid separation (>= 0.30) | Compactor invariant (merge and fission) |
| A3 | Rotation decorrelation | `enforce_a3q_manifold()` admission gate |
| A4 | Cleanup oracle (>= 0.56) | Resonator threshold in the QA engine |
| A5 | Feedback reliability (p > 0.5) | `test_a5_adversarial_reward_noise` |

Key theorems:

| # | Statement | Proof method |
|---|---|---|
| I.1 | Centroid fixed point | Algebraic (GF(2)) |
| I.2-R | Decay-aware plasticity | Lemma D1 and margin argument |
| II.1 | Cluster count bounded | LSH pigeonhole |
| III.1 | O(1) memory | Structural bound |
| XXI.1 | Unique invariant measure | Banach fixed point |
| XXII.1-R | L_F <= 1.0 (tight) | Bit-wise case analysis |
| XXIII.1 | Tracking error <= 0.70 | Novelty gate invariant |
| XXV.4 | Uniform spectral gap | lambda_2(P) * kappa_F < 1 via A3-Q |
| XXV.5 | Sub-Lemma S surjectivity | Constructive witness |

## Memory architecture (v3.3 and later)

```
Layer   What                  Solution
L0      visited URLs          CountingBloomFilter
        seed_urls             CappedVecDeque
        doc_frequency         exponential decay and evict
L1      accumulator Vec<u32>  SparseAccumulator
L2      cluster entries       age-weighted merge
L3      frozen clusters       delta encoding and Golomb-Rice
Monitor all layers            MemorySnapshot every 250 ticks
```

## DRIFT cognitive subsystems

Ten subsystems are ported into `src/drift.rs`.

| Subsystem | Purpose | When it runs |
|---|---|---|
| DMU scoring | memory retrieval salience | HNSW search |
| CognitiveMode | 3-bit [M,S,N] mode tag | every cycle |
| DCP consensus | propose, vote, resolve | broker round |
| Homeostasis | seven-need regulation | subconscious loop |
| PSC predictor | chaos-aware trend prediction | periodic |
| Global Workspace | GWT salience ranking | attention cycle |
| Emotional Field | Emotion x Stance to Mood | narrative generation |
| Context Engine | fork and merge hypotheses | reasoning |
| Implicit Intuition | pattern bundling | recognition |
| Shadow/Enantiodromia | archetype oscillation | long-term dynamics |

## Module index

### Core VSA

| File | Description |
|---|---|
| `src/lib.rs` | Hypervector, MemoryCluster, accumulator, telemetry, constitution |
| `src/reason.rs` | DeepThought, forward chaining, soft_project, theorem tests |
| `src/resonator.rs` | LSH-cached cleanup, vocabulary management |
| `src/hnsw.rs` | Approximate nearest-neighbor search with DMU scoring |

### Memory

| File | Description |
|---|---|
| `src/compression.rs` | Bloom filter, SparseAccumulator, entry merging, cold storage |
| `src/sleep.rs` | Consolidation, pruning, concept freezing |
| `src/hierarchy.rs` | Multi-level projection, L2 abstraction |
| `src/abstractor.rs` | Community detection, concept formation |

### Reasoning

| File | Description |
|---|---|
| `src/qa.rs` | Question answering, causal chains, fact verification |
| `src/narrative.rs` | Rule-based natural language generation |
| `src/analogy.rs` | Role-frame induction, analogical prediction |
| `src/temporal.rs` | Transition statistics, temporal prediction |

### Formalization and benchmarks

| File | Description |
|---|---|
| `src/formalization.rs` | Typed direct instantiation, prose grammar |
| `src/proposition.rs` | Trusted proposition kernel, theorem checking |
| `src/recurrence.rs` | First-order affine recurrence solving |
| `src/algebra*.rs` | Linear, quadratic, and system equation executors |
| `src/expression_*.rs` | Expression evaluation and simplification |
| `src/kernel.rs` | Formal kernel operations |
| `src/governed_benchmark.rs` | Unified seven-tier evaluation suite |

### Autonomy and safety

| File | Description |
|---|---|
| `src/action.rs` | Tool registry, intent decoding |
| `src/actuator.rs` | Action execution, budget enforcement |
| `src/defense.rs` | Threat detection, port rotation |
| `src/monitor.rs` | State monitoring, telemetry |
| `src/reliability.rs` | Enforced memory, replay, and guarantee contracts |
| `src/autonomy_task.rs` | Bounded background tasks |
| `src/operator.rs` | Operator command: setup, doctor, config, release, upgrade, recovery |

### Conversational runtime

| File | Description |
|---|---|
| `src/conversation/` | Service, follow-up resolution, renderer, capability adapter |
| `src/persistence/` | SQLite database, sessions, assertions, documents, migrations |
| `src/chat_server.rs` | HTTP and SSE interface |
| `src/document_learning.rs` | Document extraction and proposal review |
| `src/conversation_eval.rs` | Conversation corpus scoring, traces, ablations |
| `src/semantic_fidelity.rs` | Semantic fidelity evaluation |
| `src/semantic_shadow.rs` | Shadow worker connection |

### DRIFT cognitive subsystems

| File | Description |
|---|---|
| `src/drift.rs` | All ten subsystems (DMU, CognitiveMode, DCP, Homeostasis, and the rest) |
| `src/broker.rs` | Neocortex broker, DCP consensus |
| `src/self_model.rs` | Self-state tracking, mode transitions |
| `src/context.rs` | Context management, state tracking |

### Infrastructure

| File | Description |
|---|---|
| `src/main.rs` | Multi-agent simulation, agent loop |
| `src/forager.rs` | Web crawling, document processing |
| `src/socket.rs` | Admin socket interface |
| `src/cognition.rs` | Episodes, ConceptJournal, confidence calibration |
| `src/diagnostic.rs` | Failure classification, structural SVO centroids |

## Quick command reference

```bash
# default library suite (2,567 tests)
cargo test --lib --locked -j 3

# focused areas
cargo test --lib qa::tests
cargo test --lib reason::tests
cargo test --lib narrative::tests

# research benchmarks (ignored by default)
cargo test --lib -- --ignored

# soft projection calibration
cargo test --lib reason::tests::test_soft_projection_frontier_sweep -- --ignored --nocapture

# verification scripts
python3 prove_decay_plasticity.py
python3 prove_adversarial_Lf.py
python3 verify_dynamics.py

# multi-agent simulation
cargo run

# governed evaluation
cargo run --release --bin governed_bench -- 500 500 42 results/governed_bench/large.jsonl
```

Version history is in [`../CURRENT_STATE.md`](../CURRENT_STATE.md).
