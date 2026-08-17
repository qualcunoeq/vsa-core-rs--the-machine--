# Stage 152 — OCR TSV to source-science composition

This checkpoint closes the multimodal handoff from coordinate-bearing OCR to
typed source-science artifacts.  The corpus passes raw Tesseract-style TSV
through the table frontend before invoking the finite-statistics, bounded DNA,
or bounded chemistry bridges.

## Corpus

- cases: 600
- supported / ambiguous / unsupported: 360 / 120 / 120
- route families: statistics, biology, chemistry (200 each)
- corpus SHA-256: `e2d799d5c26ff25b1524812c64a15647af27c24e32542749520eea5c00839da8`

## Results

- exact decisions: **600/600**
- authorized supported routes: **360/360**
- frontend replay: **600/600**
- downstream replay: **600/600**
- frontend tamper rejection: **600/600**
- downstream tamper rejection: **600/600**
- false authorizations: **0**
- false denials: **0**

This validates the complete visual path rather than assuming a preconstructed
table artifact.  Headers, coordinate alignment, provenance, chemistry charge
boundaries, DNA sampling policy, and finite-statistics semantics remain
explicit.  No source catalog, registry, or HLE holdout was mutated.

Machine-readable report: `docs/stage152_visual_science_tsv_composition.json`.
