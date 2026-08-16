# Stage D — source-derived bounded complex arithmetic

This is the first source-derived domain added after the foundational packs
without a hand-written subject evaluator.  The source document is an
attributed transcription of OpenStax *Precalculus 2e*, section 3.1.  It
defines paired real/imaginary formula records for rectangular addition,
subtraction, multiplication, division, conjugation, and squared magnitude.
The runtime validates the record structure and delegates every component to
the generic source-formula interpreter.

The scope is intentionally narrow.  Polar conversion, branch choices,
analytic functions, approximate magnitudes, and other complex-analysis
semantics remain unsupported.  Division by a zero complex divisor is
inconsistent rather than guessed.

| metric | result |
| --- | ---: |
| cases | 240 |
| supported artifacts | 120/120 |
| ambiguous cases preserved | 40/40 |
| refused cases | 80/80 |
| exact decisions | 240/240 |
| values correct | 240/240 |
| source provenance preserved | 120/120 |
| replay verified | 240/240 |
| tamper rejected | 240/240 |
| mutated source documents rejected | 6/6 |
| false authorizations | 0 |
| false denials | 0 |

The source document and machine-readable receipt are immutable artifacts under
`docs/sources/` and `docs/stage_d_source_complex_arithmetic.json`.  The pack
is shadow-only; it does not mutate the live curriculum, router, or registry.

Run:

```text
cargo run --quiet --bin source_complex_pack_bench
```
