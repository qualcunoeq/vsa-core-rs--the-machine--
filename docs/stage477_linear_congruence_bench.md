# Stage 477 — canonical linear-congruence pressure benchmark

This independent benchmark validates a deliberately narrow number-theory
frontend. It accepts one literal congruence only when the requested result is
a canonical representative and the evaluator yields one solution modulo the
stated modulus. Systems, non-canonical targets, symbolic operands, compound
expressions, and multi-solution classes remain closed.

| Metric | Result |
|---|---:|
| Cases | **240** |
| Supported / ambiguous / unsupported | **120 / 40 / 80** |
| Exact decisions | **240/240** |
| Supported values | **120/120** |
| Frontend replay | **240/240** |
| Execution replay | **120/120** |
| Tamper rejections | **360/360** |
| False authorizations / denials | **0 / 0** |

The corpus includes Unicode and LaTeX congruences, canonical positive and
least-nonnegative targets, simultaneous systems, non-coprime classes,
non-canonical negative targets, compound expressions, and symbolic operands.
The route exposes only unique residue classes; a complete multi-solution
artifact is not silently converted into an answer candidate.

Evidence:

* corpus SHA-256: `97fad8022deb2d62610089dc0b8a1bdc5f0693b0458f733e0d861973058f3866`
* report SHA-256: `95766c95037f8057dbde0e6dae8f3cb2be68eae80896dfd2f1131860b4592450`
* serialized report file SHA-256: `db75a4d5c11ef64e3cae89baa198ec92c14f0bc9eda2338e4ba5cff402eb0d64`

