# Stage 365 — unknown-domain shadow bootstrap

- cases / repeated residual clusters: 4 / 2
- shadow proposals / proposal replay: 2 / 2
- proposals proven non-executable: 2
- residual-preserving cases / tamper rejections: 2 / 4
- false authorizations / live registry mutations: 0 / 0
- corpus SHA-256: `685506ff26c17da02f8e407f7f163f5ee4331ea5c94a61759a87ecd41cac8b16`

Repeated explicit operation scopes from independent source lineages now produce a typed, provenance-bound bootstrap proposal. Single-lineage, scope-mismatched, and tampered residuals remain preserved rather than generalized. The proposal contains a candidate artifact schema, prerequisite, validation plan, and falsification conditions; it cannot authorize execution or mutate the curriculum.

Reproduce with `cargo run --quiet --bin stage365_unknown_domain_bootstrap`.
Machine-readable report: `docs/stage365_unknown_domain_bootstrap.json`
