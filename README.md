# The Machine

A cognitive architecture built on hyperdimensional computing (HDC/VSA). State is a 10,240-bit binary hypervector; the primitives are XOR, popcount, bit rotation, and majority bundling. There are no neural networks, no gradients, and no LLM inference.

**Version:** v3.4
**Last updated:** 2026-09-28
**Public source:** `github.com/qualcunoeq/vsa-core-rs--the-machine--`
**Formal specification:** [`MATH.md`](MATH.md) (3,452 lines)
**Library:** 260 modules, 572 binary targets
**Library tests:** 2,567 collected (2,511 passing, 41 known pre-existing failures, 15 ignored); see [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md) section 6

## Contents

- [Overview](#overview)
- [Verification](#verification)
- [Project structure](#project-structure)
- [Getting started](#getting-started)
- [Everyday operation](#everyday-operation)
- [Interfaces](#interfaces)
- [Evaluation results](#evaluation-results)
- [Documentation map](#documentation-map)
- [Roadmap](#roadmap)
- [Licensing](#licensing)
- [Citation](#citation)

## Overview

The Machine is a two-timescale stochastic iterated function system over binary hypervectors. Cluster centroids are the fixed-size memory; new observations are absorbed into integer accumulators and projected back onto the nearest centroid.

Fast dynamics run every reasoning cycle (about 10 ticks). The state update is `x_{t+1} = P o A(x_t)`, where `A` is algebraic composition (XOR and rotation) and `P` projects onto the cluster manifold. The composition is conditionally contractive.

Slow dynamics run every 50 to 500 ticks. Integer accumulators integrate evidence per bit, a novelty gate creates clusters when the nearest-centroid distance reaches a threshold, a compactor merges and splits clusters, and a decay step ages out old evidence.

The default projection is a hard nearest-centroid snap. An optional soft projection replaces it with a temperature-weighted majority vote over all centroids. The corrected calibration (v3.1) uses `tau = 0.10` and measures an effective capacity of 2,554 states (about 11.3 bits) at `kappa_P = 0.916`.

Ten cognitive subsystems from the DRIFT project are ported in `src/drift.rs`: DMU scoring, CognitiveMode, DCP consensus, Homeostasis, PSC predictor, Global Workspace, Emotional Field, Context Engine, Implicit Intuition, and Shadow/Enantiodromia. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the module map and [`docs/UPDATES.md`](docs/UPDATES.md) for the subsystem descriptions.

## Verification

The formal specification states each result as an algebraic identity, a Banach fixed point argument, or an empirically measured bound. The stated operating envelope is 31 assumptions (A1 to A31), each labelled as assumed, checked, or enforced in [`MATH.md`](MATH.md) section 0.4.

Key guarantees:

| Guarantee | Theorem | Value |
|---|---|---|
| Memory is bounded with respect to time | III.1 | about 10.6 MB maximum at K=5120 clusters |
| Cluster count is bounded | II.1 | K <= 5120 (structural) |
| A unique attractor exists | XXI.1 | Banach fixed point from kappa < 1 |
| The system mixes exponentially | XXVI.2 | d_TV <= 0.01 within 77 cycles |
| An adversary cannot break contraction | XXII.1-R | L_F <= 1.0, joint margin 0.010 |
| Tracking error stays within its threshold | XXIII.1 | min_c delta(v_t, c) <= 0.70 |
| The capacity gain is real | XXVII.2-R | 128x multiplier (C_eff = 2554) at tau = 0.10 |

`ContractionTelemetry` in `src/lib.rs` monitors the joint product `kappa = kappa_P * kappa_F` every 50 ticks. It raises a warning at 0.995 and a critical alert at 1.001.

Reliability claims are enforced in code, not only declared. Memory budgets, replay strength, server limits, document-derived knowledge, and the release schema are covered by the guarantees in [`docs/CLAIMS.md`](docs/CLAIMS.md) and [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md) sections 12 to 16.

## Project structure

```
Cargo.toml              crate and 572 binary targets
MATH.md                 formal specification (3,452 lines)
GUIDE.md                developer guide with flowcharts and code examples
CURRENT_STATE.md        current state and engineering priorities
src/
  lib.rs                core VSA types, MemoryCluster, ContractionTelemetry
  reason.rs             DeepThought reasoning engine, soft projection
  main.rs               multi-agent simulation, broker, agent loop
  qa.rs                 question answering, causal chains, fact verification
  narrative.rs          rule-based natural language generation
  drift.rs              the ten DRIFT cognitive subsystems
  compression.rs        memory compression (L0 to L3)
  cognition.rs          episodes, ConceptJournal, confidence calibration
  reliability.rs        enforced memory, replay, and guarantee contracts
  autonomy_task.rs      bounded background tasks
  operator.rs           setup, doctor, config, release, upgrade, recovery
  conversation/         conversational runtime and follow-up semantics
  persistence/          SQLite durable state and migrations
  chat_server.rs        HTTP/SSE interface
  bin/                  572 binaries (benchmarks, experiments, tools)
docs/
  ARCHITECTURE.md       architecture overview and module index
  DEVELOPMENT.md        environment, build, test baseline, per-phase reference
  ROADMAP.md            research layers and near-term work
  CLAIMS.md             claim ledger with evidence and failure modes
  EVALUATION.md         capability matrix and experiment taxonomy
integration-tests/      conversational runtime acceptance crate
prove_decay_plasticity.py    Monte Carlo check of Theorem I.2-R
prove_adversarial_Lf.py      construction of the L_F = 1.0 worst case
verify_dynamics.py           dynamical systems verification
derive_optimal_threshold.py  projection threshold derivation
```

## Getting started

Prerequisites:

- Rust 1.97.1 (pinned in `rust-toolchain.toml`), edition 2021
- Python 3 for the verification scripts; `sympy` for the CAS paths
- Full environment inventory and recorded test baseline: [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md)

Build and test:

```bash
cargo build --locked --lib -j 3
cargo test --lib --locked -j 3
cargo test -p conversation_runtime_integration --locked
```

Use `-j 3` or `-j 4`. A single `rustc` unit for this crate peaks near 4.3 GiB resident memory. Do not use bare `cargo test`: it would build all 572 binaries, and two of them do not compile (pre-existing, unrelated to the library).

## Everyday operation

Everyday use goes through one command, `machine`. First run once, then start the interface:

```bash
cargo run --release --bin machine -- setup   # create directories, open the database once
cargo run --release --bin machine -- start   # start the chat interface
```

| Task | Command |
|---|---|
| Health and dependency status | `machine doctor` |
| Validate or show the configuration | `machine config check` / `machine config show` |
| Release, schema, and profile | `machine version` |
| Versioned capability inventory | `machine capabilities` |
| Release notes tied to evaluation results | `machine release notes` |
| Regenerate inventory, notes, and doctor artifacts | `machine release write` |
| Back up or restore the database | `machine backup <file>` / `machine restore <file>` |
| Upgrade, or roll back the last upgrade | `machine upgrade` / `machine recover` |

Configuration comes from `MACHINE_*` environment variables or flags (`--data-dir`, `--db`, `--host`, `--port`, `--token`, ...). `machine config check` reports each problem with a remedy. Profiles are documented configurations, not separate builds: `minimal`, `standard` (default), `gpu` (requires `cargo build --features cuda`), and `semantic_worker`. `machine doctor` reports whether the running binary matches the requested profile. The full operator reference is [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md) section 16.

## Interfaces

- Chat interface: `cargo run --bin machine -- start` serves the web interface on `127.0.0.1:8787` by default. It supports Ask and Teach turns, streamed progress, cancellation, a memory inspector with provenance and correct/retract/forget actions, conversation export, and follow-ups (pronoun resolution, corrections, earlier values, solver continuation). The server binds loopback by default and refuses a remote bind unless `--allow-remote` and a bearer token are set.
- Documents: `cargo run --bin machine_docs -- demo` imports text or a text-based PDF, extracts facts, definitions, and rules with source spans, and commits nothing until the items are explicitly accepted. Imported text is data, never commands. See [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md) section 13.
- Evaluation: `cargo run --bin machine_eval` scores the conversation corpus and gates on regression drift, unsupported assertions, and ablation safety. `cargo run --bin machine_eval task` runs the bounded autonomy suite. See [`docs/EVALUATION.md`](docs/EVALUATION.md).
- Semantic worker: `cargo run --bin semantic_fidelity_eval` regenerates the semantic fidelity report. The worker runs in shadow mode and never authorizes an answer.
- Multi-agent simulation: `cargo run` launches a broker and three agents.
- Verification scripts: `python3 prove_decay_plasticity.py`, `python3 prove_adversarial_Lf.py`, `python3 verify_dynamics.py`.
- Governed benchmark: `cargo run --release --bin governed_bench -- 500 500 42 results/governed_bench/large.jsonl`.

## Evaluation results

Recorded on the development host (Intel i7-1365U, portable release build).

| Vertical | Cases | Positive execution | Replay rate | False auth | False denial |
|---|---|---|---|---|---|
| Algebra (seed) | 60 | 27/27 | 1.000 | 0 | 0 |
| Algebra (generated) | 500 | 260/260 | 1.000 | 0 | 0 |
| Algebra (prose) | 20 | 15/15 | 1.000 | 0 | 0 |
| Strategic route | 500 | 500/500 | 1.000 | 0 | 0 |
| Recurrence | 500 | 251/251 | 1.000 | 0 | 0 |
| Proposition kernel | 500 | 324/324 | 1.000 | 0 | 0 |
| Adversarial (all) | 21+ | safe abstention | | 0 | 0 |

Verification control: 32/32 valid receipts accepted, 32/32 tampered receipts rejected. Unified suite runtime about 1.0 to 1.1 s (release, seed 42, 500/500, seven tiers). Conversation evaluation, semantic fidelity, and autonomy results are in [`docs/EVALUATION.md`](docs/EVALUATION.md) and the phase reports under `docs/`.

## Documentation map

| Document | Contents |
|---|---|
| [`MATH.md`](MATH.md) | Formal specification and theorems |
| [`GUIDE.md`](GUIDE.md) | Developer guide with flowcharts and code examples |
| [`CURRENT_STATE.md`](CURRENT_STATE.md) | Current state, priorities, and known issues |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Architecture overview and module index |
| [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md) | Environment, build, test baseline, per-phase reference |
| [`docs/ROADMAP.md`](docs/ROADMAP.md) | Research layers and near-term work |
| [`docs/CLAIMS.md`](docs/CLAIMS.md) | Claim ledger with evidence and failure modes |
| [`docs/EVALUATION.md`](docs/EVALUATION.md) | Capability matrix and experiment taxonomy |
| [`docs/UPDATES.md`](docs/UPDATES.md) | Memory compression and DRIFT subsystem notes |

## Roadmap

The architecture is organized into six research layers. See [`docs/ROADMAP.md`](docs/ROADMAP.md).

| Layer | Capability | Status |
|---|---|---|
| 0 | Bitwise substrate | Stable: core VSA, projection, telemetry |
| 1 | Memory and concept formation | Active: abstractor, hierarchy, sleep, concept journal |
| 2 | Reasoning and explanation | Active: QA, analogy, narrative, episode provenance |
| 3 | Self-model and adaptation | Active: diagnostic learner, confidence calibration |
| 4 | Tools and world interfaces | Stable: action registry, tool events, reliability |
| 5 | Bounded autonomy | Active: budgets, decision journal, operator boundaries |
| 6 | Governed reasoning evaluation | Stable: seven-tier suite, ablations, receipt verification |

## Licensing

The Machine is source-available under [The Machine Research Source License 1.0](LICENSE). It is not an open source license. Personal, academic, and other noncommercial research use is permitted, with attribution. Commercial use requires a separate commercial license from the author.

| Document | Contents |
|---|---|
| [`LICENSE`](LICENSE) | The Machine Research Source License 1.0 |
| [`COMMERCIAL.md`](COMMERCIAL.md) | What counts as commercial use, and how to obtain a commercial license |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | How to contribute |
| [`CLA.md`](CLA.md) | Contributor License Agreement |
| [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md) | Third-party datasets and source texts, with their licenses |

Contact for commercial licensing: <gliracurcio@gmail.com>.

## Citation

```bibtex
@misc{the-machine,
  title = {The Machine: A Provably Stable Autonomous Cognitive Architecture
           Using Hyperdimensional Computing},
  author = {qualcunoeq},
  year = {2026},
  note = {Formal specification: MATH.md (3,452 lines).
           Library: 260 modules, 572 binary targets.
           Version: v3.4},
  doi = {10.5281/zenodo.XXXXXXX}
}
```
