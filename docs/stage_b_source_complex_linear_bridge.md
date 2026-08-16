# Stage B: source-derived complex to linear-algebra bridge

This shadow composition campaign tests the lossless real-matrix
representation

```text
a + bi → [[a, -b], [b, a]]
```

for exact integral complex pairs. The delegated matrix determinant must equal
the source complex norm squared. Fractional coordinates, scalar/polar
artifacts, missing inputs, unresolved conventions, and invalid domains are not
rounded or reinterpreted.

| Measure | Result |
|---|---:|
| Cases | 240 |
| Supported / ambiguous / refused | 120 / 40 / 80 |
| Exact decisions | 240/240 |
| Supported handoffs | 120/120 |
| Norm/determinant invariant | 120/120 |
| Upstream complex replay | 120/120 |
| Bridge replay | 240/240 |
| Delegated linear-algebra replay | 120/120 |
| Tamper rejection | 240/240 |
| False authorizations / denials | 0 / 0 |

The handoff preserves complex provenance and delegates only the declared
finite exact integer matrix operation. The benchmark is shadow-only and does
not alter production routing or the curriculum registry.

Reproduction manifest:

* schema: `stage-b-source-complex-linear-bridge-v1`
* corpus SHA-256: `c40c09a1432d7b6899af01a43ea6f23852385935b9a811b4fbfe31decc4868e0`
* machine-readable output: `docs/stage_b_source_complex_linear_bridge.json`
