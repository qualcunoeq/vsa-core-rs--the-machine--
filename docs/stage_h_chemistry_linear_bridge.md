# Stage H — chemistry and linear-algebra composition

This shadow composition exposes exact chemistry atom counts as a typed
finite-dimensional vector. The element basis and semantic kind remain part of
the handoff; a numeric vector is never treated as chemistry without that
provenance. Molecular formulas and validated balanced reactions are supported.
Stoichiometric ratios, ambiguous requests, charged formulas, and unbalanced
reactions remain outside the vector route.

The independently authored corpus contains 240 cases:

| Outcome | Cases |
| --- | ---: |
| Supported formula/reaction vectors | 120 |
| Ambiguous chemistry | 40 |
| Refused composition | 80 |

Corpus SHA-256:
`1c21d8f9a65ac586da6dd40386f5094d307f40d4b8595ed398767a85ec6fc29f`

| Check | Result |
| --- | ---: |
| Exact decisions | 240/240 |
| Valid typed handoffs | 120/120 |
| Chemistry replay | 240/240 |
| Bridge replay | 240/240 |
| Linear-algebra replay | 240/240 |
| Tamper rejection | 240/240 |
| Element basis preserved | 120/120 |
| Semantic kind preserved | 120/120 |
| False authorizations | 0 |
| False denials | 0 |

The composition remains shadow-only and does not mutate the curriculum
registry, production router, or HLE holdout.

Reproduction:

```text
cargo run --quiet --bin chemistry_linear_bridge_bench
```
