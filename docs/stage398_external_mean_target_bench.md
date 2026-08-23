# Stage 398 — stated-mean/one-unknown binding benchmark

Stage 398 adds a source-backed frontend for a bounded transformation already
present in the finite-statistics catalog: a stated finite mean plus exactly
one remaining value.  It accepts explicit set notation and prose with an
explicit finite count and known values.  It refuses multiple unknowns,
median/minimum/difference targets, weighted or graphical claims, and missing
counts or values.

- Cases: **240** (120 supported, 40 ambiguous, 80 refused)
- Exact decisions / authorized answers: **240/240** / **120/120**
- Supported value replay: **120/120**
- Frontend replay / tamper rejection: **240/240** / **240/240**
- Downstream replay / tamper rejection: **120/120** / **240/240**
- False authorizations / denials: **0 / 0**
- Corpus SHA-256: `9f06bd40f53ae0f930b2e391d2055657754e1a784cbff9c03daf3115e95d977a`

The execution path is the existing OpenStax finite-statistics
`mean_equality_unknown` record and generic source-formula runtime.  No new
solver branch is introduced.

Reproduce with:

```text
cargo run --quiet --bin stage398_external_mean_target_bench
```

Machine-readable report: `docs/stage398_external_mean_target_bench.json`.
