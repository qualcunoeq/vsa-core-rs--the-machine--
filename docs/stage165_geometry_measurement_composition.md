# Stage 165 — geometry and measurement composition

The generic composition layer converts explicitly declared measurements through the source unit catalog, checks expression dimensions, and delegates formula execution to the generic source runtime. No geometry-formula evaluator branch was added.

| Measure | Result |
|---|---:|
| Cases | 400 |
| Development supported / ambiguous / refused | 180 / 60 / 60 |
| Development exact / authorized | 300/300 / 180/180 |
| Holdout supported / exact / authorized | 100 / 100 / 100 |
| Geometry / unit / execution replay (development) | 300/300 / 300/300 / 300/300 |
| Composition replay / tamper (all) | 400/400 / 400/400 |
| Unit-boundary refusals | 60 |
| False authorizations / denials | 0 / 0 |
| Runtime domain-specific branches | 0 |
| Live registry mutations | 0 |

Parent language-transfer provenance is hash-bound to Stage 164.
