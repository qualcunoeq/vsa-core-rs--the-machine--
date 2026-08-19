# Stage 374 — self-directed source education

- acquisition / holdout preflight: true / true
- source candidates / validation receipts / replays: 2 / 2 / 2
- initial / resolved / remaining gaps: 3 / 2 / 1
- campaign rounds / selected rounds: 3 / 2
- campaign replay / manifest unchanged: true / true
- false authorizations / live registry mutations: 0 / 0
- corpus SHA-256: `85b6c600d3e9519848cc3aa7b506dfee85147cc861254f163578340cae96d34a`

The planner receives exact typed residuals and two source-backed candidates whose independent holdout has passed. It selects both candidates by exact coverage, leaves the unavailable specialist gap unresolved, and keeps the curriculum manifest unchanged.

Reproduce with `cargo run --quiet --bin stage374_self_directed_source_education`.
Machine-readable report: `docs/stage374_self_directed_source_education.json`
