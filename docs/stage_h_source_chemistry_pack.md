# Stage H: source-derived bounded chemistry

This is the first source-derived science extension beyond the equation-centric
physics packs. It represents molecular formulas, validates explicitly balanced
chemical equations, and derives exact stoichiometric ratios from validated
coefficients. It does not infer products, charges, phases, molar masses, or
reaction mechanisms.

The contract is grounded in OpenStax *Chemistry 2e*, sections 2.4, 4.1, and
4.3: formulas identify substances through element symbols and subscripts;
balanced equations conserve each element; and coefficients provide
stoichiometric factors.

| Measure | Result |
|---|---:|
| Cases | 240 |
| Supported / ambiguous / refused | 120 / 40 / 80 |
| Exact decisions | 240/240 |
| Supported artifacts | 120/120 |
| Replay verified | 240/240 |
| Tamper rejected | 240/240 |
| Source provenance preserved | 120/120 |
| False authorizations / denials | 0 / 0 |

Boundary cases include malformed formulas, unknown symbols, ionic charges,
unbalanced reactions, and incomplete ratio targets. All accepted artifacts
carry the source citation and a replay hash. The pack is shadow-only and does
not alter production routing or the frozen HLE holdout.

Reproduction:

```text
cargo test --lib source_formula_pack::chemistry_pack::tests -- --nocapture
cargo run --quiet --bin chemistry_pack_bench
```

Source: [OpenStax Chemistry 2e, Reaction Stoichiometry](https://openstax.org/books/chemistry-2e/pages/4-3-reaction-stoichiometry)

Manifest:

* schema: `stage-h-source-chemistry-pack-v1`
* corpus SHA-256: `bb70db6a26e7828e9a7f2712ea5f8a1f1af80d95bc12075b98e898f5664ecad1`
* machine-readable output: `docs/stage_h_source_chemistry_pack.json`
