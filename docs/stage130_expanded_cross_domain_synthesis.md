# Stage 130 — expanded cross-domain synthesis

This is a new independent synthesis corpus after the finite simplicial and
finite-character curriculum additions.  It does not modify the historical
Stage-B report or read HLE.  Five route families are evaluated through typed
artifacts:

1. finite simplicial complex → one-skeleton graph → adjacency linear algebra;
2. finite simplicial complex → one-skeleton graph → one-step random walk;
3. finite Dirichlet character → abstract-algebra unit check;
4. exact combinatorial count → explicit Bézout certificate;
5. finite graph → exact probability → one-step random walk.

Each family contains 140 supported, 30 ambiguous, and 30 refused cases.  The
second stage is only authorized when the first artifact and its domain
invariants are present; a matrix is not silently treated as a graph or a
transition matrix, and a character generator is not treated as a generic
number-theory result without checking its modulus and unit semantics.

| Measure | Result |
|---|---:|
| Cases | 1,000 |
| Supported / ambiguous / refused | 700 / 150 / 150 |
| Exact decisions | 1,000/1,000 |
| Supported authorizations | 700/700 |
| Emitted intermediate entries | 2,420 |
| Handoff receipts verified | 1,000/1,000 |
| Replay verified | 1,000/1,000 |
| Tamper rejected | 1,000/1,000 |
| Failure gates localized | 300/300 |
| False authorizations / denials | 0 / 0 |
| Route leakage | 0 |

Corpus SHA-256: `53dccc52df30a9f260e538b36748252793ee15f843221ba1919de62af88805a0`

Reproduce with:

```text
cargo run --quiet --bin stage130_expanded_cross_domain_synthesis
```

The run writes the immutable machine-readable receipt to
`docs/stage130_expanded_cross_domain_synthesis.json`.  It is shadow-only:
the production registry, curriculum manifest, and frozen HLE holdout are not
mutated.
