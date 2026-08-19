# Stage 373 — independent holdout promotion gate

- acquisition / holdout preflight: true / true
- cases / exact decisions: 2 / 2
- clean promotion / failed-holdout block: true / true
- promotion replays: true / true
- tamper rejection: true / true
- false authorizations / live registry mutations: 0 / 0
- production snapshot unchanged: true
- acquisition report SHA-256: `3cbc119e5759642c2fa35814313d2acd6e98cc23f8e5998a18f05a4156454efc`
- holdout report SHA-256: `ea70d52b58a737226f3bd38a255319361b9b96dd59cf087d68a6142f6cb20712`

The source-derived candidate is eligible only after the independently authored language holdout passes. A candidate with holdout_passed=false is denied even when policy, migration, and dependencies are otherwise valid. No production registry is mutated.

Reproduce with `cargo run --quiet --bin stage373_independent_holdout_promotion_gate`.
Machine-readable report: `docs/stage373_independent_holdout_promotion_gate.json`
