# Stage 374 — self-directed source education

- acquisition / holdout preflight: true / true
- source candidates / admitted / rejected: 3 / 2 / 1
- validation receipts / replays: 3 / 3
- initial / resolved / remaining gaps: 3 / 2 / 1
- campaign rounds / selected rounds: 3 / 2
- campaign replay / manifest unchanged: true / true
- false authorizations / live registry mutations: 0 / 0
- corpus SHA-256: `85b6c600d3e9519848cc3aa7b506dfee85147cc861254f163578340cae96d34a`

The planner receives exact typed residuals and admits only replay-valid source validation receipts. It selects the two admitted candidates by exact coverage, rejects one invalid candidate, leaves the unavailable specialist gap unresolved, and keeps the curriculum manifest unchanged.

Reproduce with `cargo run --quiet --bin stage374_self_directed_source_education`.
Machine-readable report: `docs/stage374_self_directed_source_education.json`
