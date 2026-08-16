# Stage J: governed visual table frontend

This shadow frontend lowers coordinate-bearing OCR observations into typed
table artifacts. It preserves text and cell coordinates as provenance and
refuses to infer chart meaning, units, formulas, or relationships from visual
appearance. Ragged rows, overlapping boxes, and misaligned columns remain
ambiguous.

| Measure | Result |
|---|---:|
| Cases | 240 |
| Supported / ambiguous / unsupported | 120 / 40 / 80 |
| Exact decisions | 240/240 |
| Supported table artifacts | 120/120 |
| Supported provenance preserved | 120/120 |
| Replay verification | 240/240 |
| Tamper rejection | 240/240 |
| False authorizations / denials | 0 / 0 |

The input boundary is coordinate-bearing OCR TSV rather than free-form visual
semantics. This is intentional: the module produces candidate typed
observations for later routing, not a guessed answer. It is isolated from live
routing and the curriculum registry.

Reproduction manifest:

* schema: `stage-j-visual-table-frontend-v1`
* corpus SHA-256: `866b1d8b34517991d40eb8a4dd6f2f90f41f0762bcc124a9c221b6f62cd3a831`
* machine-readable output: `docs/stage_j_visual_table_frontend.json`
