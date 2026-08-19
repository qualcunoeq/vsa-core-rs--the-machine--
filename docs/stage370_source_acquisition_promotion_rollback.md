# Stage 370 — source-acquisition promotion and rollback

- source preflight: true
- lifecycle cases / exact decisions: 3 / 3
- promotions / blocked: 2 / 1
- registry replays / tamper rejections: 3 / 3
- rollback / world-state preservation / historical replay: 1 / 1 / 1
- false authorizations / live registry mutations: 0 / 0
- production snapshot unchanged: true
- source report SHA-256: `3cbc119e5759642c2fa35814313d2acd6e98cc23f8e5998a18f05a4156454efc`

The Stage 369 source-derived candidate is promoted only in a cloned versioned registry. A later counterexample is blocked, and an accumulated-world-state clone rolls back to the prior version while preserving historical replay. The production snapshot and live registry remain untouched.

Reproduce with `cargo run --quiet --bin stage370_source_acquisition_promotion_rollback`.
Machine-readable report: `docs/stage370_source_acquisition_promotion_rollback.json`
