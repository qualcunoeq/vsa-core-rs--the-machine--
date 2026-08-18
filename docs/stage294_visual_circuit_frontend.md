# Stage 294 — governed visual circuit frontend

A circuit frontend emits only explicit component, terminal, wire, value, ground, and provenance observations. It does not infer voltage, current, polarity, equivalent resistance, or circuit behavior.

* cases / exact decisions: 240 / 240
* supported / ambiguous / refused: 120 / 40 / 80
* supported artifacts / provenance: 120 / 120
* replay / tamper: 240 / 240
* false authorizations / denials: 0 / 0
* HLE questions read / registry mutations: 0 / 0

Reproduce with `cargo run --quiet --bin visual_circuit_frontend_bench`.
