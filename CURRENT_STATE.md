# Current State

**Version:** v3.4
**Last updated:** 2026-09-28

## Identity

The Machine is a cognitive architecture built on hyperdimensional computing (HDC/VSA). State is a 10,240-bit binary hypervector; the primitives are XOR, popcount, bit rotation, and majority bundling. There are no neural networks, no gradients, and no LLM inference.

The crate exposes:

- Core VSA engine (`src/lib.rs`): Hypervector, MemoryCluster, accumulator dynamics, ContractionTelemetry, hot and cold memory management.
- Reasoning engine (`src/reason.rs`): forward chaining, soft projection, anchored composition.
- QA engine (`src/qa.rs`): question answering, causal-chain reasoning, fact verification.
- Narrative generator (`src/narrative.rs`): rule-based natural language generation.
- Diagnostic system (`src/diagnostic.rs`, `src/abstraction_learner.rs`): self-extending failure classification.
- Formalization stack (`src/formalization.rs`, `src/proposition.rs`, `src/recurrence.rs`): typed direct instantiation, proposition proofs, recurrence solving.
- Governed reasoning suite (`src/governed_benchmark.rs`, `src/strategic_route_benchmark.rs`, `src/capability_planner.rs`): seven-tier evaluation with receipt verification.
- Algebra and equation solving (`src/linear_equation.rs`, `src/quadratic_equation.rs`, `src/linear_system.rs`, `src/expression_evaluation.rs`): executor-based solving.
- DRIFT cognitive subsystems (`src/drift.rs`): DMU scoring, CognitiveMode, DCP consensus, Homeostasis, PSC predictor, Global Workspace, Emotional Field, Context Engine, Implicit Intuition, Shadow/Enantiodromia.
- Memory compression (`src/compression.rs`): counting Bloom filter, sparse accumulator, entry merging, cold storage serialization.
- Conversational runtime (`src/conversation/`, `src/persistence/`, `src/chat_server.rs`): chat sessions, durable SQLite state, follow-up semantics, and typed capability adapters.
- Reliability and operator surfaces (`src/reliability.rs`, `src/autonomy_task.rs`, `src/operator.rs`): enforced memory and replay contracts, bounded background tasks, and the `machine` operator command.
- Multi-agent simulation (`src/main.rs`): broker and agents with self-narrative, n-gram prediction, and sleep cycles.

The library has 260 modules and the crate declares 572 binary targets for experiments, benchmarks, and tools.

## Version history

| Version | Date | Key changes |
|---|---|---|
| v3.0 | Apr 2026 | Soft projection, Theorem XXV.4 spectral gap, L_F correction |
| v3.1 | Jun 2026 | Soft projection numerical fix, rho-admissible invariant, Sub-Lemma S closure, tracking error theorems, coreference chain |
| v3.2 | Jun 2026 | Intervention test: A21 resolved with structural SVO centroids (3/3 correct against 1/3 for hand-coded tables) |
| v3.3 | Jul 2026 | Memory compression (L0 to L3), sparse accumulator, cold storage, Bloom filter, DRIFT subsystem port |
| v3.4 | 2026-09-28 | Governed reasoning verticals and formalization audit (Jul 2026), then the conversational runtime phases (Sep 2026): chat interface, durable state, follow-up conversation, typed capabilities, semantic fidelity and shadow worker, reliability contracts, document learning, conversation evaluation, controlled autonomy, and the operator release tooling |

## Verification

Everyday operation goes through the Phase 11 operator command:

```bash
cargo run --release --bin machine -- setup   # once
cargo run --release --bin machine -- start   # daily use
cargo run --release --bin machine -- doctor  # health and dependency status
```

Library tests:

```bash
cargo test --lib --locked -j 3                 # full library suite
cargo test --lib qa::tests                     # QA engine
cargo test --lib reason::tests                 # reasoning engine
cargo test -p conversation_runtime_integration --locked   # acceptance crate
```

Research and calibration tests are ignored by default:

```bash
cargo test --lib -- --ignored
cargo test --lib reason::tests::test_soft_projection_frontier_sweep -- --ignored --nocapture
```

Benchmarks and tools:

```bash
cargo run --release --bin governed_bench -- 500 500 42 results/governed_bench/large.jsonl
cargo run --release --bin algebra_bench -- data/algebra_seed_v1.json results/algebra_bench/seed.jsonl
cargo run --release --bin recurrence_bench -- 500 42 results/recurrence_bench/large.jsonl
cargo run --release --bin proposition_bench -- 500 42 results/proposition_bench/large.jsonl
cargo run --release --bin strategic_route_bench -- --scale medium --seed 42
cargo run --release --bin machine_eval
cargo run --release --bin machine_eval task
```

Verification scripts:

```bash
python3 prove_decay_plasticity.py    # Theorem I.2-R flip bounds
python3 prove_adversarial_Lf.py      # L_F = 1.0 worst case
python3 verify_dynamics.py           # dynamical systems verification
```

## Test baseline

Recorded 2026-09-28 with `cargo test --lib --locked -j 3`:

| Result | Count |
|---|---|
| passed | 2,511 |
| failed | 41 (all pre-existing) |
| ignored | 15 |
| wall time | 318 s |

There are 2,567 library tests in total. The 41 failures are pre-existing and unrelated to the library modules that changed in the recent phases. They fall into five groups: missing external artifacts (`chess_learner`, `pdf_reader`), the curated-evidence gate in `qa` and `router`, proof-stack drift in `qa`, stale CAS and typed-math expectations, and formalization or capability drift. A stochastic set (`reason`, `indexer`, `predictive`) swaps which member fails between runs, so a run reports 41 or 42 failures. The full breakdown is in [`docs/DEVELOPMENT.md`](DEVELOPMENT.md) section 6.

The conversational runtime acceptance crate (`integration-tests/conversation_runtime`) has 12 tests.

## Guarantees

The formal specification ([`MATH.md`](MATH.md), 3,456 lines) covers three levels of rigor:

| Status | Meaning |
|---|---|
| Proven | Algebraic identity or Banach fixed point, with no assumptions beyond GF(2) |
| Empirically consistent | Observed across the test suites, not formally proven |
| Dependent | Proven under the stated assumptions (A1 to A31 contracts) |

Recorded guarantees:

| Guarantee | Theorem | Value |
|---|---|---|
| Memory is bounded with respect to time | III.1 | about 10.6 MB maximum at K=5120 |
| Cluster count is bounded | II.1 | K <= 5120 |
| A unique invariant measure exists | XXI.1 | Banach fixed point from kappa < 1 |
| The system mixes exponentially | XXVI.2 | d_TV <= 0.01 within 77 cycles |
| An adversary cannot break contraction | XXII.1-R | L_F <= 1.0, margin 0.010 |
| Tracking error never exceeds 0.70 | XXIII.1 | min_c delta(v_t, c) <= 0.70 |
| Spectral gap below 1 on A3-Q manifolds | XXV.4 | lambda_2(P) * kappa_F < 1 |
| Sub-Lemma S (surjectivity) | XXV.5 | g = nearest o P_tau surjects from rho^26(W_i) |

The reliability registry records 15 enforced guarantees, 2 checked, and 0 assumed, including the memory-boundedness contracts (Phase 7), task authority and observability (`G-TASK-AUTHORITY`, `G-TASK-OBSERVABLE`, Phase 10), and upgrade safety (`G-RELEASE-SCHEMA`, Phase 11). `cargo run --bin reliability_report` regenerates `docs/phase7_reliability_v1.*`.

## Agent loop

The main loop (`src/main.rs`) orchestrates:

```
Every tick (about 2 s): forager intake, projection through clusters,
  DeepThought reasoning, intent selection, action dispatch,
  epistemic update, broker consensus.

Every 50 ticks: accumulator decay (gamma = 0.975), kappa_P measurement,
  tripwire check (kappa = kappa_P * kappa_F < 0.995), entry merging,
  episode store persistence, contraction report.

Every 100 ticks: hot and cold memory sweep, cluster compaction.

Every 250 ticks: memory profiler snapshot.
```

N-gram chain prediction observes `CognitiveMode` transitions and predicts the next mode every 25 ticks.

## Admin socket

| Command | Description |
|---|---|
| `ASK <question>` | answer from stored facts |
| `STORE <sentence>` | store a fact from natural language |
| `FACTS` | show fact and rule counts |
| `CHAIN <question>` | multi-hop chain reasoning |
| `STORE_RULE <rule>` | store a causal rule |
| `SAVE` / `LOAD` | persist or load QA memory |

## Known issues and priorities

| Item | Status |
|---|---|
| 41 pre-existing library test failures | Open. Grouped by cause in `docs/DEVELOPMENT.md` section 6; unrelated to the recent phases |
| Stochastic tests using `rand::thread_rng()` | Open. Three tests (`reason`, `indexer`, `predictive`) swap which member fails between runs |
| Two binary targets fail to compile | Open. `stage243_second_source_corpus` and `stage173_route_blind_technical_language`, pre-existing |
| Compiler warnings in experimental modules | Open. A clean library build records 9 warnings; warning volume makes regressions harder to spot |
| LSH collision at high cluster counts | Bounded. A full-scan fallback handles overflow above roughly 200 clusters |
| Joint contraction margin | Monitored. The margin is 0.010; the telemetry tripwire is at 0.995 |
| Documentation drift | Addressed by this pass: version, dates, and counts are aligned across the core documents |

## Module reference

| Module | Lines | Purpose |
|---|---|---|
| `lib.rs` | 7,439 | Core VSA types, Hypervector, MemoryCluster, ContractionTelemetry, memory management |
| `reason.rs` | 7,190 | DeepThought reasoning engine, soft projection, theorem tests |
| `qa.rs` | 10,989 | QA engine, causal-chain reasoning, fact verification |
| `drift.rs` | 2,622 | Ten DRIFT cognitive subsystems |
| `cognition.rs` | 2,630 | Episodes, ConceptJournal, confidence calibration, ablations |
| `compression.rs` | 1,330 | Bloom filter, sparse accumulator, entry merging, cold storage |
| `diagnostic.rs` | 2,338 | Failure classification, structural SVO centroids |
| `narrative.rs` | 2,365 | Rule-based natural language generation |
| `main.rs` | 3,168 | Multi-agent simulation, agent loop, telemetry |
| `broker.rs` | 2,293 | Neocortex broker, DCP consensus, quorum selection |
| `conversation/service.rs` | 4,582 | Conversational runtime |
| `chat_server.rs` | 2,269 | HTTP and SSE interface |
| `reliability.rs` | 845 | Enforced memory, replay, and guarantee contracts |
| `autonomy_task.rs` | 1,887 | Bounded background tasks |
| `operator.rs` | 1,954 | Operator command: setup, doctor, config, release, upgrade, recovery |
