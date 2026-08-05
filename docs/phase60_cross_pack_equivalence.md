# Phase 60 — Cross-pack equivalence and route selection

This is a shadow-only benchmark. It exercises route selection and semantic
equivalence across the validated finite-dimensional linear algebra, finite
probability, graph, bounded dynamics, and finite-state packs. It does not
modify the production registry, router, or execution authority.

## Scope

The accepted half contains four independently typed equivalence families:

```text
scalar affine recurrence  ↔  augmented matrix evolution
vector recurrence          ↔  matrix evolution
random walk                ↔  probability-preserving matrix evolution
finite-state trace         ↔  one-hot vector evolution (trace witness only)
```

The selected route remains the semantically strongest route. For example, a
random walk uses the random-walk route because graph identity, vertex order,
transition convention, and probability invariants are required; generic
matrix evolution is only an equivalence witness.

The refusal half covers semantic-erasing conversions and route hazards:

* adjacency shape without explicit transition semantics;
* vertex-order mismatch;
* non-normalized transitions;
* horizons beyond the bounded dynamics budget;
* dimension mismatch;
* spectral or stationary shortcuts;
* finite-state labels erased into an untyped numeric route;
* signed weights treated as probabilities.

## Frozen result

The machine-readable receipt is [phase60_cross_pack_equivalence.json](phase60_cross_pack_equivalence.json).

| Metric | Result |
|---|---:|
| Cases | 240 |
| Accepted equivalence cases | 120 |
| Safe refusals | 120 |
| Exact route decisions | 240/240 |
| Equivalent routes agree | 120/120 |
| Stronger invariants preserved | 120/120 |
| Replay verification | 240/240 |
| Tamper rejection | 240/240 |
| False authorizations | 0 |
| False denials | 0 |
| Route leakage | 0 |
| Semantic-erasure refusals | 120/120 |

Every accepted route preserves its domain-specific invariant: exact affine
coordinates, vector dimensions, normalized nonnegative probability mass, or
finite-state labels and guards. Numeric equivalence never promotes an
artifact into a richer semantic type by shape alone.

## Reproducibility

Run:

```text
cargo run --bin cross_pack_equivalence_bench
```

The benchmark writes `docs/phase60_cross_pack_equivalence.json` and asserts
all gate metrics before writing the receipt.

