# Evaluation Matrix

This file defines how to measure progress without narrowing the project.  The
architecture can stay broad, but each capability should have a local check, a
baseline, and a failure mode.

## Test Classes

### Unit

Fast deterministic checks for invariants.  These run in the default suite.

Command:

```bash
cargo test --lib
```

Expected contents:

- algebraic identities;
- projection and accumulator invariants;
- parser and encoder edge cases;
- small deterministic reasoning examples.

### Calibration

Quantitative checks for approximate claims.  These may emit tables and may be
ignored by default.

Command pattern:

```bash
cargo test --lib reason::tests::test_soft_projection_frontier_sweep -- --ignored --nocapture
```

Expected contents:

- soft projection frontier sweeps;
- contraction and tracking estimates;
- merge/split probability calibration;
- seeded Monte Carlo checks.

### Benchmark

Long-running or dataset-backed measurements.  These are ignored by default and
should record inputs, seed, metric, and output.

Command pattern:

```bash
cargo test --lib -- --ignored
```

Expected contents:

- chess cross-validation;
- bond pipeline validation;
- large HNSW recall sweeps;
- full autonomy validation runs.

## Capability Matrix

| Capability | Baseline | Metric | Current surface | Next improvement |
| --- | --- | --- | --- | --- |
| Noise-stable memory | raw HV stream | NHD to source, retained signal, memory growth | `src/lib.rs`, `src/reason.rs` | structured long-run memory benchmark |
| Concept formation | similarity-only grouping | prediction gain, compression gain, concept churn | `src/abstractor.rs`, `src/cognition.rs` | `test_abstraction_ablation_benchmark` (ignored) emits `ExperimentResult` JSON |
| Temporal prediction | last-state or frequency baseline | top-k accuracy, calibration error | `src/temporal.rs`, `src/predictive.rs` | seedable transition benchmark |
| QA recall | direct lookup | answer accuracy, provenance completeness | `src/qa.rs`, `src/cognition.rs` | persist `CognitiveEpisode` records for QA runs |
| Analogical transfer | non-VSA parser/classifier | held-out transfer accuracy | `src/analogy.rs` | keep negative A21 result until mechanism changes |
| Diagnostics | static keyword map | held-out diagnosis accuracy, category drift | `src/diagnostic.rs`, `src/abstraction_learner.rs` | persistent learner promotion audit |
| Feedback learning | no post-answer update | outcome score, reversible update rate, regression rate | `src/cognition.rs` | append-only feedback store |
| Tool use | direct invocation logs | replayability, reliability estimate, side-effect class | `src/action.rs`, `src/actuator.rs`, `src/cognition.rs` | route actuator calls through `ToolEvent` |
| Autonomy | unconstrained loop | success rate under budget, rollback rate, unsafe-action blocks | `src/bin/autonomy_experiment.rs`, `src/cognition.rs` | require external actions to spend `AutonomyBudget` |
| Resilience | no adversary | recovery time, false positive rate, integrity preservation | `src/defense.rs`, `src/monitor.rs` | operator-visible resilience tests |
| Ablation | all mechanisms enabled | delta vs full model by capability metric | `src/cognition.rs` | run QA and abstraction with explicit `AblationConfig` |

## Result Record

Each recurring experiment should produce enough information to compare runs.
Use this shape in logs or serialized output:

```json
{
  "experiment": "name",
  "claim": "C-000",
  "commit": "git-sha",
  "seed": 0,
  "dataset": "none-or-path",
  "baseline": "description",
  "metrics": {
    "primary": 0.0
  },
  "passed": true,
  "notes": "short human-readable summary"
}
```

The Rust-side carrier for this schema is `cognition::ExperimentResult`.  New
benchmarks should prefer emitting that structure as JSON over printing
human-only tables.

## Cognition Benchmark Runner

The future rented-machine benchmark campaign should use:

```bash
cargo run --release --bin cognition_bench -- --case all --scale large --seed 42 --out results/cognition_bench/run.jsonl
```

The runner defaults to `small` scale and `/tmp/cognition_bench.jsonl`, so local
use stays limited unless a larger scale is explicitly requested.

### Transformer-Cousin Behavioral Suite

Use this as the first behavioral reference point for comparing the bitwise
architecture against transformer-style expectations without calling an LLM:

```bash
cargo run --release --bin cognition_bench -- --case transformer-cousin --scale small --seed 42 --out results/cognition_bench/cousin.jsonl
```

The suite measures:

- grounded QA accuracy over explicit facts;
- multi-hop chain accuracy over causal rules;
- abstention on unknown questions;
- improvement after feedback insertion;
- term-trace coverage for explanation/provenance.

The primary summary metric is `aggregate_score`, with component metrics emitted
alongside it in the standard `ExperimentResult` JSONL schema.

## Promotion Rules

A mechanism can move closer to core architecture when:

- it beats its baseline on a seeded check;
- it has at least one default deterministic invariant test;
- it has a documented failure condition;
- it does not require manual log inspection to know whether it worked.

A mechanism should stay experimental when:

- it only works in a single hand-built scenario;
- its success depends on broad printed output;
- it has no baseline;
- it improves one domain while silently degrading another.

## Immediate Evaluation Debt

- Convert print-only calibration tests into assertions on coarse, defensible
  ranges, or move them into explicit benchmark runners.
- Add structured output to ignored benchmark tests.
- Reduce warning noise in core modules so new warnings are visible.
- Record negative results in `docs/CLAIMS.md` before removing failed mechanisms.
