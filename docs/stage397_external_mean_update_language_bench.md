# Stage 397 — natural-language mean-update binding repair

Stage 397 extends the generic finite mean-update frontend to equivalent
explicit wording such as `scores of ... on ...` and `receives a score of ...`.
The semantic boundary is unchanged: one finite old list, exactly one added
value, and an explicitly requested mean increase.

- Cases: **240** (120 supported, 40 ambiguous, 80 refused)
- Exact decisions / authorized answers: **240/240** / **120/120**
- Supported value replay: **120/120**
- Frontend replay / tamper rejection: **240/240** / **240/240**
- Downstream replay / tamper rejection: **120/120** / **240/240**
- False authorizations / denials: **0 / 0**
- Corpus SHA-256: `46c593b545d990ff841699192ffd40827c3a63e2827f5c94979e43cfd5e9c61c`

The corpus is independently authored and does not use external benchmark
answers.  Execution remains the generic source-formula runtime and the route
remains shadow-only.

Reproduce with:

```text
cargo run --quiet --bin stage397_external_mean_update_language_bench
```

Machine-readable report: `docs/stage397_external_mean_update_language_bench.json`.
