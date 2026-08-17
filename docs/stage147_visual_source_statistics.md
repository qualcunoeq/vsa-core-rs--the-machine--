# Stage 147 — Visual table to source-derived statistics

This stage composes coordinate-preserving table perception with the generic
source-derived finite-statistics catalog.  The bridge requires the exact
`quantity,value` header and explicit supported labels (`sum`, `count`,
`weighted_sum`, `total_weight`). Numeric-looking tables without those semantic
claims remain closed.

## Corpus and results

- 120 supported tables
- 80 ambiguous tables (wrong headers or duplicate labels)
- 40 unsupported tables (continuous/density or unknown quantities)
- Corpus SHA-256: `2c611bf8376932a8909ab777658f90b101df94cce67657e98e42e4a28096ed3f`

| Measure | Result |
|---|---:|
| Exact decisions | 240/240 |
| Authorized supported routes | 120/120 |
| Replay verification | 240/240 |
| Tamper rejection | 240/240 |
| False authorizations | 0 |
| False denials | 0 |

The visual artifact, source formula request, and source-derived result retain
their provenance independently.  This is shadow-only and does not alter the
source catalog, curriculum manifest, or production routing.

Machine-readable report: `docs/stage147_visual_source_statistics.json`.
