# Stage 132 — mixed mathematical frontend routing

Stage 132 puts three independently implemented technical-language frontends
behind one route-blind boundary:

* bounded simplicial homology;
* bounded elementary number theory;
* bounded finite Dirichlet characters.

Every report is presented to all three frontends. Supported reports must select
exactly one route; ambiguous and unsupported reports must select none. Only a
selected typed request reaches a downstream evaluator.

| Measure | Result |
|---|---:|
| Cases | 720 |
| Supported / ambiguous / unsupported | 360 / 120 / 240 |
| Frontend invocations | 2,160 |
| Exact route decisions | 720/720 |
| Supported authorizations | 360/360 |
| Downstream artifacts emitted | 360 |
| Frontend replay verified | 720/720 |
| Downstream replay verified | 360/360 emitted artifacts |
| Frontend tamper rejected | 720/720 |
| Downstream tamper rejected | 360/360 emitted artifacts |
| Ambiguity/unsupported preserved | 360/360 |
| False authorizations / denials | 0 / 0 |
| Route leakage | 0 |

Corpus SHA-256: `7b10271944ad3806a4b1efd14d5975236ce2a5dc80039286dcee6152aed37b06`

Reproduce with:

```text
cargo run --quiet --bin stage132_mixed_math_frontend_route_blind
```

The machine-readable receipt is
`docs/stage132_mixed_math_frontend_route_blind.json`. This is shadow-only and
does not mutate the curriculum manifest, production registry, or HLE holdout.
