# Stage 93 — source-derived Bayes rule

This checkpoint acquires a second independent source-derived domain from
OpenStax *Principles of Data Science*, §3.4, Bayes' Theorem. The catalog is a
single declarative formula (`prior * likelihood / evidence`) executed by the
generic source-formula interpreter. A bounded language frontend requires all
three probabilities explicitly and refuses ambiguity, zero evidence,
continuous/approximate semantics, and inferred independence.

## Independent benchmark

| metric | result |
|---|---:|
| cases | 300 |
| supported / ambiguous / unsupported | 180 / 60 / 60 |
| supported authorized | 180/180 |
| ambiguity preserved | 60/60 |
| unsupported refused (including missing input) | 60/60 |
| source-formula replays | 300/300 |
| finite-probability bridge replays | 180/180 |
| tamper rejections | 300/300 |
| provenance preserved | 300/300 |
| source-catalog mutations rejected | 6/6 |
| false authorizations / denials | 0 / 0 |

The development, validation, and sealed partitions are independently generated
with the same 60/20/20 outcome proportions. The complete machine-readable
report is [stage93_source_bayes_bench.json](stage93_source_bayes_bench.json).

## Source lineage

* Source: OpenStax *Principles of Data Science*, §3.4 Probability Theory.
* URL: https://openstax.org/books/principles-data-science/pages/3-4-probability-theory
* The source states `P(A|B) = P(A)·P(B|A)/P(B)` and requires a positive
  conditioning probability; the record preserves the source span and license.
* Source hash, catalog hash, corpus hashes, replay receipts, and all per-case
  provenance are recorded in the JSON report.

## Cross-domain result

Every authorized source result was lowered into the existing
`finite_exact_probability` Bayes operation. The bridge preserved exact rational
semantics and replay provenance; a source scalar alone was never treated as a
probability artifact without the explicit prior/likelihood/evidence mapping.
