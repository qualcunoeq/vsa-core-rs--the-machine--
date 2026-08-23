# Stage 458 — LaTeX binomial frontend pressure benchmark

The existing source-derived bounded-counting frontend now accepts only
explicit numeric `\binom{n}{r}` and `\dbinom{n}{r}` notation and lowers it to
the already validated exact-combination operation. Symbolic, malformed,
conflicting, out-of-range, and non-counting cases remain closed.

| Metric | Result |
|---|---:|
| Cases | 240 |
| Supported / ambiguous / unsupported | 120 / 40 / 80 |
| Exact decisions | 240/240 |
| Supported values | 120/120 |
| Frontend replay | 240/240 |
| Emitted execution replay | 140/140 |
| Tamper rejections | 380/380 |
| False authorizations / denials | 0 / 0 |

The execution denominator is conditional because only 140 cases produced a
typed request; every emitted execution was replay-verified. The corpus is
independent of the Goal 6 portfolio and contains numeric boundary cases,
symbolic notation, malformed braces, conflicting ordered semantics, invalid
ranges, and unrelated domains.

Evidence:

* corpus SHA-256: `a285cca17218ac8a6d770b4971fc945841be12c52c85c15fc64ba5dcb0b1cf2d`
* report SHA-256: `d52aa35180939c4d14f452a065a05340c029e2851e5f973d8b36a98ca14c471b`

The extension does not introduce a new solver or registry mutation; it is a
notation bridge into the existing source-counting pack.
