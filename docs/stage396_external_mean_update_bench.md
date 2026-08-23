# Stage 396 — external finite mean-update binding benchmark

This independent corpus validates a narrow source-derived frontend for the
change in a finite mean after exactly one explicitly stated value is added to
an explicitly enumerated old list.  The frontend refuses grouped, weighted,
graph, table, rate, decrease, and otherwise incomplete formulations.

- Cases: **240** (120 supported, 40 ambiguous, 80 refused)
- Exact decisions / authorized answers: **240/240** / **120/120**
- Supported value replay: **120/120**
- Frontend replay / tamper rejection: **240/240** / **240/240**
- Downstream replay / tamper rejection: **120/120** / **240/240**
- False authorizations / denials: **0 / 0**
- Corpus SHA-256: `42e47759c53fbf89d59e3358f8cd7881632da35f8f5550471ef61d23e5422330`

The source record is OpenStax *Introductory Statistics 2e*,
`descriptive-statistics`, and is stored in
`docs/sources/openstax_mean_update_source.txt`.  Execution uses the generic
source-formula runtime; no mean-update-specific executor is added.

Reproduce with:

```text
cargo run --quiet --bin stage396_external_mean_update_bench
```

Machine-readable report: `docs/stage396_external_mean_update_bench.json`.
