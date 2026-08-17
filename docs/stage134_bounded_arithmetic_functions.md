# Stage 134 — bounded arithmetic functions

Stage 134 adds a source-attributed finite prerequisite layer for the planned
number-theory curriculum. It supports divisor-count and divisor-sum
certificates, exact Möbius values, and bounded prime counting using explicit
trial factorization. It does not claim analytic number theory.

| Measure | Result |
|---|---:|
| Cases | 240 |
| Supported / ambiguous / refused | 120 / 40 / 80 |
| Exact decisions | 240/240 |
| Supported artifacts | 120/120 |
| Replay verified | 240/240 |
| Tamper rejected | 240/240 |
| False authorizations / denials | 0 / 0 |

The refusal set covers oversized inputs, zero-domain inconsistencies, invalid
analytic domains, and missing values. Source metadata is preserved in every
complete artifact. The advanced `number_theory` node remains planned; this
pack does not authorize asymptotic counts, analytic continuation,
Dirichlet-series bounds, or unbounded factorization.

Corpus SHA-256: `893425c61cb612c36cfe5ff8f1abf0ff34b5a6a5630ef8a616bccd2d5a8b55d0`

Manifest transition:

* previous manifest: `37675d40f8291d9abc007547a34fc0aa9e01830ac68e3fdcd057a11aeb5d07eb`
* current manifest: `e252a0d7e1632815efde3dd5d6044e4e4aa3b9d697485b215e4269450943cb31`

The new `bounded_arithmetic_functions` node is shadow-validated; the broader
analytic `number_theory` node remains planned. Reproduce with:

```text
cargo test --quiet --lib bounded_arithmetic_functions_pack
cargo run --quiet --bin stage134_bounded_arithmetic_functions
cargo run --quiet --bin curriculum_manifest
```
