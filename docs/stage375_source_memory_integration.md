# Stage 375 — source-derived curriculum memory integration

- acquisition / holdout / education preflight: true / true / true
- admitted modules: 2
- parent records / appended modules: 1 / 2
- duplicate refusals: 2
- exact retrievals / replay: 2 / 2
- stale-version / unknown-domain refusals: 2 / 1
- tampered module / tampered result refusals: 1 / 1
- parent unchanged / clone replay verified: true / true
- false authorizations / live registry mutations: 0 / 0
- corpus SHA-256: `fb5ecd4cf5318899d244336b93a274e848cd2af92f93fd92ef5229a51740f9fe`

Only the two admitted, replay-valid source modules enter a cloned curriculum memory. Their exact source hashes are immutable versions; duplicates, stale versions, unknown domains, tampered modules, and tampered retrieval receipts fail closed. The parent memory and live registry remain unchanged.

Reproduce with `cargo run --quiet --bin stage375_source_memory_integration`.
Machine-readable report: `docs/stage375_source_memory_integration.json`
