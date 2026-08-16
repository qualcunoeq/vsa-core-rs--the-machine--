# Stage C: shifted-language source-derived complex frontend

This campaign independently varies wording, clause order, whitespace, exact
rational notation, and operator phrasing before the source-derived complex
arithmetic pack. Unsupported polar, analytic, approximate, malformed, and
incomplete requests remain fail-closed.

| Measure | Result |
|---|---:|
| Cases | 240 |
| Supported / ambiguous / unsupported | 120 / 40 / 80 |
| Exact decisions | 240/240 |
| Complete frontends | 120/120 |
| Downstream artifacts emitted | 120/120 |
| Downstream values correct | 120/120 |
| Provenance preserved | 240/240 |
| Frontend replay | 240/240 |
| Downstream replay (emitted) | 120/120 |
| Frontend tamper rejection | 240/240 |
| Downstream tamper rejection (emitted) | 120/120 |
| False authorizations / denials | 0 / 0 |

The benchmark is shadow-only. It does not mutate the curriculum manifest,
production registry, or live routing. Conditional downstream denominators are
reported explicitly because non-complete frontend decisions do not emit pack
artifacts.

Reproduction manifest:

* schema: `stage-c-source-derived-complex-shifted-frontend-v1`
* source pack: `source_derived_complex_arithmetic`
* corpus SHA-256: `8f8f72b79c0f5d679b65a9c80799293f72f62c590d18a85fbeef96f2ddce158f`
* machine-readable output: `docs/stage_c_source_complex_shifted_frontend.json`
