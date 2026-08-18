# Stage 292 — governed visual geometry frontend

A coordinate-preserving geometry frontend emits only explicit points, segments, circles, relations, confidence, and provenance. It does not infer lengths, angles, incidence, parallelism, or proofs from coordinates.

* cases / exact decisions: 240 / 240
* supported / ambiguous / refused: 120 / 40 / 80
* supported artifacts / provenance: 120 / 120
* replay / tamper: 240 / 240
* false authorizations / denials: 0 / 0
* HLE questions read / registry mutations: 0 / 0

Reproduce with `cargo run --quiet --bin visual_geometry_frontend_bench`.
