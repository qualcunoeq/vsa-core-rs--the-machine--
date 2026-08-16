# Stage D — bounded source-document extraction

The finite-statistics source pack now has a generic extraction path from an
attributed source transcription.  `extract_formula_records` accepts only
explicit formula blocks with declared inputs, assumptions, constraints, and
evidence spans.  It parses a small arithmetic grammar into the existing
declarative formula AST; it does not infer omitted variables, domain meaning,
or source claims.

The extracted records are then evaluated by the same generic rational formula
interpreter used by the source catalog.  The independent campaign exercises
all five records and mutates the source document in six ways.

| metric | result |
| --- | ---: |
| extracted records | 5 |
| independent exercises complete | 120/120 |
| exact decisions | 120/120 |
| replay verified | 120/120 |
| tamper rejected | 120/120 |
| evidence spans | 5/5 |
| mutated documents rejected | 6/6 |
| false authorizations | 0 |

The source document and machine-readable receipt are immutable artifacts under
`docs/sources/` and `docs/stage_d_source_document_extraction.json`.  This is a
shadow acquisition path; extraction never mutates the curriculum manifest or
live routing.

Run:

```text
cargo run --quiet --bin source_document_extraction_bench
```
