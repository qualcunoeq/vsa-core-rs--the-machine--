# Stage 90 — source-derived linear interpolation education

Stage 90 is the first new source-derived domain after the full curriculum
integration gate.  The source is an attributed OpenStax Precalculus 2e
formulation of slope and point-slope equations; the catalog records the
linear-interpolation relation

```text
y = y1 + (x - x1) * (y2 - y1) / (x2 - x1)
```

The frontend emits a generic `FormulaRequest` only when all five bindings are
explicit, endpoint coordinates are distinct, and the target lies between the
endpoints.  It preserves ambiguity for multiple targets and refuses
extrapolation, nonlinear/spline requests, approximate values, and missing
models.  Execution uses the existing generic source expression interpreter;
there is no interpolation-specific evaluator branch.

| Measure | Result |
|---|---:|
| Cases | 300 |
| Supported / ambiguous / unsupported | 180 / 60 / 60 |
| Exact decisions | 300/300 |
| Supported values | 180/180 |
| Replay verification | 300/300 |
| Tamper rejection | 300/300 |
| Source provenance | 300/300 |
| Source mutations rejected | 6/6 |
| False authorizations / denials | 0 / 0 |

Source and catalog hashes, plus per-case receipts, are recorded in
`stage90_source_linear_interpolation.json`.  The report is shadow-only and
does not mutate the live registry or curriculum manifest.

Reproduction:

```text
RUSTFLAGS='-Awarnings' cargo run --quiet --bin stage90_source_linear_interpolation
```

Source basis: [OpenStax Precalculus 2e, §2.1 Linear Functions](https://openstax.org/books/precalculus-2e/pages/2-1-linear-functions),
including the slope and point-slope equations and the interpolation boundary.
