# Stage 278 — unit-conversion shadow validation

Fresh generic validation of a source-derived bounded unit-conversion catalog. The pack supports four explicit nonnegative metric conversions and refuses ambiguous, incompatible, offset, or unsupported unit requests.

* cases: 600 (360 supported / 120 ambiguous / 120 unsupported)
* exact decisions: 600
* supported authorization / replay / tamper: 360 / 360 / 360
* all frontend replay / tamper: 600 / 600
* provenance: 600
* false authorizations / denials: 0 / 0
* manifest / registry mutations: 0 / 0
* HLE questions read: 0

The candidate remains shadow-only.

Reproduce with `cargo run --quiet --bin stage278_unit_conversion_shadow_validation`.
