# Stage 287 — expanded curriculum memory scale

The 38-pack shadow portfolio was materialized into append-only typed memory and queried under exact domain/artifact/version constraints.

* shadow packs / descriptors: 38 / 131
* records / segments: 60000 / 235
* exact queries / complete: 1200 / 1200
* ambiguous queries / detected: 300 / 300
* stale refused: 200 / 200
* unknown refused: 200 / 200
* provenance refused: 100 / 100
* prerequisite queries / complete: 1200 / 1200
* contamination: 0
* replay / tamper: 60000 / 1000
* reconstruction records / equal: 60000 / true
* parent memory unchanged / manifest unchanged: true / true
* false authorizations / denials: 0 / 0

Reproduce with `cargo run --quiet --bin stage287_expanded_curriculum_memory_scale`.
