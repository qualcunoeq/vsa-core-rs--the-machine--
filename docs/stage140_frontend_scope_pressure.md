# Stage 140 — embedded-notation scope pressure

An independently authored 240-case corpus tested the arithmetic-functions and
elementary-number-theory frontends on standalone requests, quoted formulas,
multiple variable scopes, and advanced context. This was a diagnostic pressure
run before changing either frontend.

| Measure | Result |
|---|---:|
| Cases | 240 |
| Standalone arithmetic / number-theory cases | 60 / 60 |
| Ambiguous scope cases | 80 |
| Unsupported cases | 40 |
| Exact decisions before repair | 200/240 |
| Replay / tamper verification | 240/240 |
| Overbroad completions | 80 |
| Shadow false authorizations | 40 |
| Production authorizations | 0 |

The 40 false authorizations are sandbox diagnostics, not live answers: the
number-theory frontend selected the first visible binding in repeated scopes.
The other 40 overbroad completions are the expected multiple-route cases,
which the route-blind dispatcher correctly refuses. No production router or
registry was changed. This result establishes a generic repair requirement:
frontends must reject repeated local bindings and competing operation markers
unless scope evidence makes one interpretation unique.

Corpus SHA-256: `97c22dd23c8bc6e33990c465791d746887e1fc7869bcf7d0f26d02eeda6ea23b`

Reproduce with:

```text
cargo run --quiet --bin stage140_frontend_scope_pressure
```
