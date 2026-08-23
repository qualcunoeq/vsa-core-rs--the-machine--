# Stage 393 — external mean binding benchmark

- cases: **240** (120 supported, 40 ambiguous, 80 refused)
- exact decisions / authorized answers: **240/240** / **120/120**
- supported value replay: **120/120**
- frontend replay / tamper rejection: **240/240** / **240/240**
- downstream tamper rejection: **240/240**
- false authorizations / denials: **0 / 0**
- corpus SHA-256: `4526d2ea6175669b4540f4c89a458786697f2b9efe97347b6e91141d296c4305`

This independent corpus validates the source-backed arithmetic-mean
frontend's explicit `sum of N items is S` binding. It does not read the
external exam oracle and does not mutate the curriculum or production
registry. The frontend emits the existing typed `sum`/`count` request; the
generic source formula runtime performs execution and replay.

Reproduce with:

```text
cargo run --quiet --bin stage393_external_mean_binding_bench
```

Machine-readable report: `docs/stage393_external_mean_binding_bench.json`.
