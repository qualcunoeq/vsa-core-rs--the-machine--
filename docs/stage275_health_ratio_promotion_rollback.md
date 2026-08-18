# Stage 275 — health-ratio promotion and rollback

Clone-only lifecycle gate for bounded health ratios.

* decisions: 240/240 exact
* promotions / blocked: 100 / 100
* regressions / rollbacks: 40 / 40
* replay / tamper: 240 / 240
* historical replay / parent preserved: 240 / 240
* false authorizations / denials: 0 / 0
* live manifest / registry mutations: 0 / 0

No clinical or production route was enabled.

Reproduce with `cargo run --quiet --bin stage275_health_ratio_promotion_rollback`.
