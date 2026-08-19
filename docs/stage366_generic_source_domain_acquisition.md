# Stage 366 — generic source-derived domain acquisition

- source records / source-module replay: 4 / true
- source mutation rejection: true
- supported / boundary cases: 4 / 10
- exact decisions / formula replays: 14 / 14
- tamper rejections: 1
- false authorizations / denials: 0 / 0
- live registry mutations: 0
- source id: `openstax-precalculus-2e:sequences-series`
- source SHA-256: `c328d9a310329947ae2fd34913178db7ab5cff37874db899a5e624d235159e91`

The source document was parsed into typed formula records and evaluated by the generic expression runtime. Exercise values and negative cases were generated from record-declared inputs and constraints; no formula identifier is an execution branch. The catalog remains shadow-only and rejects missing, inconsistent, unknown, and ambiguous requests.

Reproduce with `cargo run --quiet --bin stage366_generic_source_domain_acquisition`.
Machine-readable report: `docs/stage366_generic_source_domain_acquisition.json`
