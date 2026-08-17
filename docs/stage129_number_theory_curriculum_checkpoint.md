# Stage 129 — finite-character curriculum checkpoint

The curriculum now contains a separate `shadow_validated` node for bounded
finite Dirichlet characters.  It supports exact character values as roots of
unity, finite partial-sum histograms, and orthogonality certificates for prime
moduli at most 31.  This is a prerequisite layer, not a claim of analytic
number theory or asymptotic character estimates.

| Measure | Result |
|---|---:|
| Independent pack cases | 240 |
| Exact decisions | 240/240 |
| Supported artifacts | 120/120 |
| Replay verified | 240/240 |
| Tamper rejected | 240/240 |
| False authorizations / denials | 0 / 0 |
| Advanced `number_theory` node | remains planned |
| Production registry mutations | 0 |

Manifest transition:

```text
before: 91b53e24c925bfd9ba6c5a087f19ab21a575029536b24d70527e0872c80a8194
after:  37675d40f8291d9abc007547a34fc0aa9e01830ac68e3fdcd057a11aeb5d07eb
```

The existing advanced number-theory node remains deliberately unvalidated;
finite characters do not authorize asymptotic counts, Dirichlet-series
claims, or analytic continuation.

Reproduce the pack campaign with:

```text
cargo run --quiet --bin stage128_dirichlet_character_pack
```
