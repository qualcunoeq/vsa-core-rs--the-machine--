# Stage 270 — health-ratio shadow validation

Fresh generic validation of a provenance-derived bounded health-ratio catalog.

* cases: 600 (360 supported / 120 ambiguous / 120 unsupported)
* exact decisions: 600
* supported authorization / replay / tamper: 360 / 360 / 360
* all frontend replay / tamper: 600 / 600
* provenance: 600
* false authorizations / denials: 0 / 0
* manifest / registry mutations: 0 / 0
* HLE questions read: 0

This is a bounded ratio calculator, not clinical advice, and remains shadow-only.

Reproduce with `cargo run --quiet --bin stage270_health_ratio_shadow_validation`.
