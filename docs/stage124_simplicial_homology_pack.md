# Stage 124 — bounded finite simplicial homology

The Machine now has a shadow-only finite simplicial-complex substrate.  It
validates explicitly closed complexes with at most eight vertices and dimension
three, constructs exact boundary matrices over `F_2`, and computes unreduced
Betti numbers and Euler characteristics.  The empty simplex, torsion, signed
integer coefficients, irrational or unbounded complexes, and numerical
approximations remain outside the contract.

| Measure | Result |
|---|---:|
| Cases | 240 |
| Supported / ambiguous / refused | 120 / 40 / 80 |
| Exact decisions | 240/240 |
| Supported artifacts | 120/120 |
| Replay verified | 240/240 |
| Tamper rejected | 240/240 |
| False authorizations / denials | 0 / 0 |
| Clone-only curriculum admission | true |
| Production manifest unchanged | true |

The refused set covers non-`F_2` coefficients, missing faces, duplicate
simplices, and complexes beyond the vertex bound.  Every artifact carries
source citation, assumptions, provenance, and a deterministic replay hash.

The candidate pack is admitted only to a cloned curriculum manifest with
prerequisites `source_derived_finite_topology` and
`linear_algebra_spectral`; the production manifest and HLE routing remain
unchanged.

Reproduce with:

```text
cargo run --quiet --bin stage124_simplicial_homology_pack
```

Machine-readable report: `docs/stage124_simplicial_homology_pack.json`.
