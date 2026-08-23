# Stage 436 — source-derived regression frontend

- cases / supported / ambiguous / unsupported: 240 / 120 / 40 / 80
- exact decisions: 240/240
- supported values: 120/120
- frontend replay / tamper: 240/240 / 240/240
- execution replay / tamper: 120/120 / 120/120
- source provenance: 120/120
- false authorizations / denials: 0 / 0
- source SHA-256: `267e1f495d229d25f7d729029907f42253805c11e2a9c840300670114afef597`
- corpus SHA-256: `abc151e39a03343f3994d86b8f5c9a7c03cbd045ec4a4b46430b35c555b6bed5`
- report SHA-256: `f22b8ef63b8110ec4b9831e7bbb92a83b07c1c5137e2e61dd0ea592134764e61`

The frontend accepts only explicit regression targets and labeled rational
inputs. It preserves ambiguity for multiple targets and rejects confidence
intervals, hypothesis tests, nonlinear regression, and missing quantities.
