# Stage 131 — finite-character technical-language frontend

Stage 131 adds the missing language boundary for the bounded finite-Dirichlet
character pack. The parser accepts explicit operation, prime modulus,
character exponent, and operation-specific inputs. It preserves ambiguity
for missing or competing operations and refuses analytic, asymptotic,
continuous, composite-modulus, and approximate-complex requests.

The independently authored corpus contains 120 supported, 40 ambiguous, and
80 unsupported reports. Complete frontend requests are routed to the existing
character evaluator; the frontend itself never authorizes an answer.

| Measure | Result |
|---|---:|
| Cases | 240 |
| Supported / ambiguous / unsupported | 120 / 40 / 80 |
| Exact frontend decisions | 240/240 |
| Downstream authorizations | 120/120 |
| Replay verified | 240/240 |
| Tamper rejected | 240/240 |
| False authorizations / denials | 0 / 0 |

Corpus SHA-256: `5ead848f00484f1da39b3cdaf0da4525aa2525ad09956216e6e9c0821e65c709`

Frontend status distribution is explicit: 120 complete, 14 ambiguous, 26
missing, and 80 unsupported. Missing fields are not silently inferred.

Reproduce with:

```text
cargo test --quiet --lib dirichlet_character_frontend
cargo run --quiet --bin stage131_dirichlet_character_language_frontend
```

The machine-readable receipts are in
`docs/stage131_dirichlet_character_language_frontend.json`. This remains a
shadow-only route; no production registry, curriculum manifest, or HLE
holdout is mutated.
