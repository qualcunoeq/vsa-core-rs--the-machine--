# Stage 471 — extended exact-combination pressure benchmark

The existing bounded-counting frontend now has a separately governed exact
combination evaluator for explicit numeric binomials beyond the frozen
`n <= 20` factorial scope. The original evaluator and Stage 458 corpus remain
unchanged; the extension is selected only after a complete combination request
reaches that range boundary.

| Metric | Result |
|---|---:|
| Cases | 240 |
| Supported / ambiguous / unsupported | 120 / 40 / 80 |
| Exact decisions | 240/240 |
| Supported values | 120/120 |
| Frontend replay | 240/240 |
| Execution replay | 160/160 |
| Tamper rejections | 400/400 |
| False authorizations / denials | 0 / 0 |

Supported cases use independently varied numeric `\\binom`/`\\dbinom` forms,
including large `n` and low-order or edge coefficients. Boundaries include
ordered-semantics conflicts, symbolic coefficients, compound expressions,
range violations, and exact-u128 overflow. The evaluator preserves explicit
provenance and refuses non-combination operations.

Evidence:

* corpus SHA-256: `bb4fa5ccf1de4419526ffd50f309cf1b013eaf126f2c338bf5813090899903d2`
* report SHA-256: `faa59624d3c05868e627c45882fe65f08f24945ff07205ce5116651b77e89cad`

The benchmark is independent of the Goal 6 portfolio and does not mutate the
curriculum manifest or production registry.
