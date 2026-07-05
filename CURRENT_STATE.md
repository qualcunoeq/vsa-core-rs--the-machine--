# Current State

Last checked: 2026-07-05.

## Project Shape

The Machine is a Rust HDC/VSA cognitive architecture built around 10,240-bit
binary hypervectors. The crate exposes the core library in `src/lib.rs`, the
reasoning engine in `src/reason.rs`, QA in `src/qa.rs`, diagnostics in
`src/diagnostic.rs`, and the self-extending diagnostic learner in
`src/abstraction_learner.rs`.

The repository also contains experiment and validation binaries:

```bash
cargo run --bin the_machine
cargo run --bin bond_pipeline
cargo run --bin diagnose_experiment
cargo run --bin autonomy_experiment
cargo run --bin validate_autonomy
cargo run --bin intervention_test
```

## Verification Commands

Fast/default library verification:

```bash
cargo test --lib
```

Research and dataset-backed validation tests are intentionally excluded from the
default suite with `#[ignore]`. Run them explicitly when recalibrating claims:

```bash
cargo test --lib -- --ignored
```

Useful focused checks:

```bash
cargo test --lib qa::tests
cargo test --lib diagnostic::tests
cargo test --lib abstraction_learner::tests
cargo test --lib reason::tests::test_soft_projection_frontier_sweep -- --ignored --nocapture
```

## Test Boundaries

Default tests should be deterministic, local, and fast enough to run during
normal development.

Ignored tests are research or integration benchmarks. They may require local
datasets, run long validation loops, or emit calibration tables:

- chess cross-validation and weight-learning sweeps in `src/chess_eval.rs`
- full bond market pipeline in `src/bond_feeder.rs`
- soft projection frontier calibration in `src/reason.rs`
- large HNSW recall sweep in `src/hnsw.rs`
- larger source-file ingestion in `src/code_bridge.rs`

## Current Engineering Priorities

1. Keep theorem and calibration tests deterministic with seeded RNG.
2. Maintain a fast default regression suite separate from research benchmarks.
3. Persist and version `AbstractionLearner` promotions before using them as
   operational memory.
4. Add traceable `resolve_term` results for QA and causal-chain debugging.
5. Keep README and math-facing documentation aligned with current code.

## Research Coordination

The broad architecture is tracked through three lightweight docs:

- `docs/ROADMAP.md` separates the lifelong system goal into research layers.
- `docs/CLAIMS.md` records active, false, provisional, and retired claims.
- `docs/EVALUATION.md` maps capabilities to baselines, metrics, and checks.

New mechanisms should normally add or update a claim and identify how the
result will be evaluated before becoming part of the core path.

## Known Residual Risk

The codebase still emits many compiler warnings. Most are unused imports,
unused variables in experiments/tests, or intentionally broad helper code, but
warning volume makes real regressions easier to miss. Reducing warning noise is
a good follow-up once behavioral test boundaries are stable.
