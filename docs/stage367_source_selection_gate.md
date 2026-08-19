# Stage 367 — exact source-selection gate

- cases / replay-valid receipts: 4 / 3
- selected lineages / rejected candidates: 5 / 4
- tamper-rejected cases: 2
- false authorizations / live registry mutations: 0 / 0
- corpus SHA-256: `ed303a4bb25074ac3d39f5051e855ff247ae4c2af250f907114a68982dc4f46d`

Selection requires both exact declared operation scope and membership in the independently observed source lineages. A lexical distractor, mismatched scope, or tampered evidence record is rejected. The receipt remains a source proposal and performs no ingestion or promotion.

Reproduce with `cargo run --quiet --bin stage367_source_selection_gate`.
Machine-readable report: `docs/stage367_source_selection_gate.json`
