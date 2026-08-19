# Stage 371 — source-derived technical-language ingestion

- source records: 2
- cases: 240
- supported / ambiguous / unsupported: 120 / 40 / 80
- exact decisions / route decisions: 240 / 240
- frontend replay / tamper rejection: 240 / 240
- downstream authorized / replay / tamper: 120 / 120 / 120
- false authorizations / denials: 0 / 0
- live registry mutations / HLE questions read: 0 / 0
- corpus SHA-256: `78fe24d833dc04a8127115f803be62f10d6f354064b3a1448954ccf63840edab`

The route-blind corpus is generated from two independently cited declarative catalogs. Alias and labeled-input grounding select a unique catalog; explicit alternative, approximation, and missing-input cases remain non-authorized. The generic frontend and expression runtime contain no subject-specific execution branch.

Reproduce with `cargo run --quiet --bin stage371_source_derived_technical_language`.
Machine-readable report: `docs/stage371_source_derived_technical_language.json`
