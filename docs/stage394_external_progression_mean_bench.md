# Stage 394 — external arithmetic-progression mean benchmark

This independent, answer-key-blind corpus pressure-tests a source-derived
arithmetic-progression mean frontend.  It is not generated from the frozen
external mathematics questions and it does not read HLE or portfolio answers.

- Cases: 240 (120 supported, 40 ambiguous, 80 refused)
- Exact decisions: 240/240
- Supported authorizations: 120/120
- Supported downstream replays: 120/120
- Frontend replays: 240/240
- Frontend tamper rejections: 240/240
- Downstream tamper rejections: 240/240
- False authorizations / denials: 0 / 0
- Corpus report SHA-256: `a1bc1ae506f922534a33f2619f990579149d36edc43265709437b180aaf0d46f`

The source-backed relation is the finite arithmetic-progression endpoint
identity `(first + last) / 2`, attributed to OpenStax *Precalculus 2e*.
The frontend accepts explicit endpoints and structurally bounded multiple
ranges, while refusing missing bounds, non-finite or continuous requests,
descending/equal endpoints, weighted or unrelated statistics, and ambiguous
progression descriptions.

The route remains shadow-only.  It does not mutate the live registry or
curriculum manifest.

Reproduce with:

```text
cargo run --quiet --bin stage394_external_progression_mean_bench
```

Machine-readable report: `docs/stage394_external_progression_mean_bench.json`.
