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
| Concept formation | similarity-only grouping | prediction gain, compression gain, concept churn | `src/abstractor.rs` | abstractor on/off ablation |
| Temporal prediction | last-state or frequency baseline | top-k accuracy, calibration error | `src/temporal.rs`, `src/predictive.rs` | seedable transition benchmark |
| QA recall | direct lookup | answer accuracy, provenance completeness | `src/qa.rs` | traceable `resolve_term` result |
| Analogical transfer | non-VSA parser/classifier | held-out transfer accuracy | `src/analogy.rs` | keep negative A21 result until mechanism changes |
| Diagnostics | static keyword map | held-out diagnosis accuracy, category drift | `src/diagnostic.rs`, `src/abstraction_learner.rs` | persistent learner promotion audit |
| Tool use | direct invocation logs | replayability, reliability estimate, side-effect class | `src/action.rs`, `src/actuator.rs` | `ToolEvent` schema |
| Autonomy | unconstrained loop | success rate under budget, rollback rate, unsafe-action blocks | `src/bin/autonomy_experiment.rs` | explicit autonomy budget model |
| Resilience | no adversary | recovery time, false positive rate, integrity preservation | `src/defense.rs`, `src/monitor.rs` | operator-visible resilience tests |

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

