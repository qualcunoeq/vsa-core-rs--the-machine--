# Stage H — generic source-science catalog ingestion

The source-derived science laws are now loaded from a cited JSON catalog rather
than constructed in the evaluator.  A domain-agnostic catalog validator checks
law identity and aliases, expression inputs, constraint references, and
evidence-bearing source citations before shadow execution.  The interpreter
remains one generic exact rational expression walk; no law-specific evaluator
branch was added.

The independent ingestion campaign accepts the four-record catalog and rejects
five mutated catalogs (duplicate identity, undeclared expression input,
unknown constraint input, duplicate alias, and missing evidence span).

| metric | result |
| --- | ---: |
| valid catalogs accepted | 1/1 |
| records | 4 |
| mutated catalogs | 5 |
| mutated catalogs rejected | 5/5 |
| evidence spans preserved | 4/4 |
| false acceptances | 0 |
| execution mutation | none |

Catalog hash and the machine-readable receipt are recorded in
`docs/stage_h_source_science_catalog_ingestion.json`.  The catalog remains
shadow-only: validation does not promote laws, mutate registries, or authorize
answers by itself.

Run:

```text
cargo run --quiet --bin source_science_catalog_ingestion_bench
```
