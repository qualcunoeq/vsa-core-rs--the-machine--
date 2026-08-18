# Stage 291 — governed visual plot frontend

A coordinate-preserving Cartesian-plot frontend emits only explicit axis, point, segment, kind, confidence, unit, and provenance artifacts. It does not infer functions, interpolation, monotonicity, or downstream answers.

* cases / exact decisions: 240 / 240
* supported / ambiguous / unsupported: 120 / 40 / 80
* supported artifacts / provenance: 120 / 120
* replay / tamper: 240 / 240
* false authorizations / denials: 0 / 0
* HLE questions read / registry mutations: 0 / 0

Reproduce with `cargo run --quiet --bin visual_plot_frontend_bench`.
