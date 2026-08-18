# Stage 280 — four-candidate sealed benchmark

Four source-derived bounded modules were evaluated through one route-blind generic frontend.

* cases: 1600
* exact decisions: 1600
* authorized: 1200
* sealed exact / authorized: 400 / 400
* boundary refusals: 400
* frontend replay / tamper: 6400 / 6400
* route leakage: 0
* false authorizations / denials: 0 / 0
* manifest / registry mutations: 0 / 0

Reproduce with `cargo run --quiet --bin stage280_four_candidate_sealed_benchmark`.
