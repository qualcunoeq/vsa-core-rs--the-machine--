# Stage 372 — independently authored source-language holdout

- source records: 2
- cases: 80
- supported / ambiguous / unsupported: 36 / 20 / 24
- exact decisions / route decisions: 80 / 80
- frontend cases / replay / tamper: 60 / 60 / 60
- downstream executions / authorized / replay / tamper: 38 / 36 / 38 / 38
- false authorizations / denials: 0 / 0
- live registry mutations / HLE questions read: 0 / 0
- corpus SHA-256: `7644a64513d61749cb83de7f1b611a6c52f5cf41860c88e32f44d6ff096ff18f`

The holdout is stored separately from the case generator and uses independently authored prose, clause order, distractors, cross-catalog ambiguity, missing inputs, approximation markers, and unsupported targets. No frontend or catalog mutation occurred.

Reproduce with `cargo run --quiet --bin stage372_source_language_holdout`.
Machine-readable report: `docs/stage372_source_language_holdout.json`
