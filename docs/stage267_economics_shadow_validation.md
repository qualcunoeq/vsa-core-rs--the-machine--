# Stage 267 — economics shadow validation

Fresh generic-front-end validation of the utility-selected economics candidate.

* cases: 600 (360 supported / 120 ambiguous / 120 unsupported)
* exact decisions: 600
* supported authorization / replay / tamper: 360 / 360 / 360
* all frontend replay / tamper: 600 / 600
* provenance: 600
* false authorizations / denials: 0 / 0
* live manifest / registry mutations: 0 / 0

The candidate remains clone-only; no production authorization occurred.

Reproduce with `cargo run --quiet --bin stage267_economics_shadow_validation`.
