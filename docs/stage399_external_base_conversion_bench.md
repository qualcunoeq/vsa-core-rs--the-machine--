# Stage 399 — external source-derived positional-base conversion

This stage adds a bounded source-derived capability for exact conversion of
finite nonnegative integer numerals between bases 2 through 36.  The source
record and implementation reject fractional, negative, approximate,
digit-statistic, and derived targets.  It is shadow-only.

Independent pressure corpus:

| Measure | Result |
|---|---:|
| Cases | 240 |
| Supported / ambiguous / refused | 120 / 40 / 80 |
| Exact decisions | 240/240 |
| Authorized supported values | 120/120 |
| Frontend replay / tamper rejection | 240/240 |
| Downstream replay | 120/120 supported |
| Downstream tamper rejection | 240/240 |
| False authorizations / denials | 0 / 0 |

The machine-readable receipt is
[`stage399_external_base_conversion_bench.json`](stage399_external_base_conversion_bench.json).
The source record is
[`openstax_base_conversion_source.txt`](sources/openstax_base_conversion_source.txt).

## Answer-key-blind external transfer

Every prompt was offered to every portfolio route.  No production registry or
curriculum manifest was changed.

| Partition | Questions | Unique candidates | Base candidates | Correct hash matches | Replay | False authorizations |
|---|---:|---:|---:|---:|---:|---:|
| Development | 3,000 | 22 | 3 | 3/3 | 3/3 | 0 |
| Sealed | 1,000 | 4 | 3 | 3/3 | 3/3 | 0 |

The three newly grounded development cases are `math-v1-development-0093`,
`math-v1-development-1108`, and `math-v1-development-1727`.  The sealed
cases are `math-v1-sealed-0616`, `math-v1-sealed-0651`, and
`math-v1-sealed-0687`.  Sealed scoring used answer hashes only; plaintext
answers were not read.

The transfer receipts are:

* [`stage399_external_base_conversion_score_development.json`](stage399_external_base_conversion_score_development.json)
* [`stage399_external_base_conversion_score_sealed.json`](stage399_external_base_conversion_score_sealed.json)

This is a shadow transfer result, not a production authorization or a claim
of broad base-conversion competence.
