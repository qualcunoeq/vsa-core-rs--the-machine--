# Stage 128 — bounded finite Dirichlet characters

This curriculum pack adds a narrow algebraic foundation for finite Dirichlet
characters.  For prime moduli at most 31 it produces exact roots-of-unity
values, finite partial-sum exponent histograms, and orthogonality certificates.
It explicitly refuses composite or oversized moduli, invalid character
exponents, asymptotics, analytic continuation, floating-point complex values,
and cryptographic conclusions.

| Measure | Result |
|---|---:|
| Cases | 240 |
| Supported / ambiguous / refused | 120 / 40 / 80 |
| Exact decisions | 240/240 |
| Supported artifacts | 120/120 |
| Replay verified | 240/240 |
| Tamper rejected | 240/240 |
| False authorizations / denials | 0 / 0 |

The implementation is source-attributed and shadow-only.  It does not claim
the planned advanced number-theory node, and it does not alter HLE routing.

Reproduce with:

```text
cargo run --quiet --bin stage128_dirichlet_character_pack
```

Machine-readable report: `docs/stage128_dirichlet_character_pack.json`.
