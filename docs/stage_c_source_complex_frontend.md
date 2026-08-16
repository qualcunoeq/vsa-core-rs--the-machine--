# Stage C: source-derived complex arithmetic frontend

This shadow benchmark measures the technical-language boundary for the
source-derived bounded complex arithmetic pack. It accepts only explicit
rectangular literals and explicit operation evidence; it does not infer polar,
analytic, decimal, or approximate semantics.

| Measure | Result |
|---|---:|
| Cases | 240 |
| Supported / ambiguous / unsupported | 120 / 40 / 80 |
| Exact decisions | 240/240 |
| Complete frontends | 120/120 |
| Downstream typed artifacts | 120/120 |
| Values correct | 240/240 |
| Provenance preserved | 240/240 |
| Frontend replay | 240/240 |
| Downstream replay | 240/240 |
| Tamper rejection | 240/240 |
| False authorizations / denials | 0 / 0 |

The 120 supported cases traverse the full shadow path:

```text
technical text → bounded complex request → source-derived pack → typed artifact
```

Ambiguous operation references remain non-authorizing. Unsupported cases cover
polar/argument and analytic requests, decimal or approximate requests, and
incomplete or malformed rectangular inputs. The frontend benchmark is isolated
from production routing and does not mutate the curriculum registry.

Reproduction manifest:

* schema: `stage-c-source-derived-complex-frontend-v1`
* source pack: `source_derived_complex_arithmetic`
* corpus SHA-256: `5dad90d5dfd089e6a9c121a7e2996f1d3d3327cddeb588c5e06ed267b345d82f`
* machine-readable output: `docs/stage_c_source_complex_frontend.json`
