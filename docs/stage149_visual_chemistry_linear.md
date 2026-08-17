# Stage 149 — Visual element-count tables to chemistry and linear algebra

This checkpoint adds a second source-derived scientific visual route.  A
coordinate-preserving table is admitted only with the exact `element,count`
header, positive bounded counts, and explicit uncharged element symbols.  The
bridge constructs a canonical molecular-formula artifact through the source
chemistry pack, then lowers it to a semantically labelled element-count vector
through the existing chemistry/linear-algebra bridge.

## Frozen corpus

- corpus: 240 independently authored coordinate-preserving tables
- supported / ambiguous / unsupported: 120 / 80 / 40
- corpus SHA-256: `c93173a60b32d9cadcf98dc77ea1d596fc9b2bce315f18f29af8dc192dada536`

## Results

- exact decisions: **240/240**
- authorized supported routes: **120/120**
- replay verification: **240/240**
- tamper rejection: **240/240**
- false authorizations: **0**
- false denials: **0**

Charges, phases, duplicate element rows, and non-element table semantics are
not inferred.  The route remains shadow-only and does not mutate the source
catalog, chemistry pack, linear-algebra pack, or production routing.

Machine-readable report: `docs/stage149_visual_chemistry_linear.json`.
