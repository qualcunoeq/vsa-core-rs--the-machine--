# Stage 281 — unit-conversion promotion and rollback

Clone-only lifecycle gate for the source-derived bounded unit-conversion candidate.

* decisions: 240/240 exact
* promotions / blocked: 100 / 100
* regressions / rollbacks: 40 / 40
* replay / tamper: 240 / 240
* historical replay / parent preserved: 240 / 240
* false authorizations / denials: 0 / 0
* live manifest / registry mutations: 0 / 0

Production routing remains unchanged.

Reproduce with `cargo run --quiet --bin stage281_unit_conversion_promotion_rollback`.
