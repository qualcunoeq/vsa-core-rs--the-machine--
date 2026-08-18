# Stage 264 — HLE geometry shadow probe

The frozen HLE corpus was evaluated against the cloned geometry manifest only.

* cases: 2500
* source records: 5
* complete frontends / executable candidates: 0 / 0
* unique shadow candidates: 0
* correct / rejected candidate answers: 0 / 0
* ambiguous or missing: 2500
* unsupported: 162
* frontend replay / tamper: 2500 / 2500
* production authorizations: 0
* false authorizations: 0
* live manifest / registry mutations: 0 / 0

A shadow candidate is never a production answer. The current live manifest and router remain unchanged.

Reproduce with `cargo run --quiet --bin stage264_hle_geometry_shadow_probe`.
