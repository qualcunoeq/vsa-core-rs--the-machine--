# Stage 395 — external natural-combination binding benchmark

This independent corpus validates a narrow natural-language bridge to the
externally sourced finite-combination evaluator.  The accepted grammar is
only `choose R out of N`; role selections, probability questions, ordering,
and additional constraints remain closed.

- Cases: 240 (120 supported, 40 ambiguous, 80 refused)
- Exact decisions: 240/240
- Supported authorizations: 120/120
- Supported downstream replays: 120/120
- Frontend replays: 240/240
- Frontend tamper rejections: 240/240
- Downstream tamper rejections: 240/240
- False authorizations / denials: 0 / 0
- Corpus report SHA-256: `9ce067c4a40635f40a6850913ed687e2ee5337c928b5a46e1d360b4bd0aa4d36`

The execution path remains the source-backed bounded combination evaluator;
the new work is only a structural frontend binding.  The route is shadow-only
and does not mutate production routing or the curriculum manifest.

Reproduce with:

```text
cargo run --quiet --bin stage395_external_combination_binding_bench
```

Machine-readable report: `docs/stage395_external_combination_binding_bench.json`.
