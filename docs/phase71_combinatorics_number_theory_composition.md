# Phase 71 — Combinatorics and number-theory composition

This campaign composes bounded exact counts with elementary number theory only
when the arithmetic role of each count is explicitly declared. A scalar count
is not implicitly a residue, coefficient, or modulus. Supported routes cover
Bezout certificates, modular inverses, linear congruences, and compatible CRT
classes. Arithmetic conditions remain visible and fail closed.

| Measure | Result |
|---|---:|
| Cases | 240 |
| Supported / ambiguous / refused | 120 / 40 / 80 |
| Exact decisions | 240/240 |
| Supported routes | 120 |
| Replay verified | 240/240 |
| Tamper rejected | 240/240 |
| False authorizations / denials | 0 / 0 |

Refusal cases include non-unit inverse requests, incompatible CRT systems,
oversized combinatorial inputs, and invalid number-theory domains. The
ambiguous cases carry no arithmetic role for the count and therefore never
enter the number-theory pack.

Reproduction:

```text
cargo run --quiet --bin combinatorics_number_theory_composition_bench
```

Manifest:

* schema: `phase71-combinatorics-number-theory-composition-v1`
* corpus SHA-256: `a50c4ea49ea206070af82d39b47ac8fe39138a16d5b6eec5020b17e8283633a6`
* machine-readable output: `docs/phase71_combinatorics_number_theory_composition.json`

This remains a shadow curriculum evaluation. It does not modify production
routing, registries, or the frozen HLE holdout.
