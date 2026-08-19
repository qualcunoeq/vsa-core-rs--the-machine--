# Stage 347 — post-externality frozen baseline

The 4,000-question external MATH release was rerun after the implementation
examples identified by the independent externality audit were replaced with
non-release fixtures.

| Split | Cases | Correct authorized | Incorrect authorized | False authorizations | First-gate replay |
|---|---:|---:|---:|---:|---:|
| Development | 3,000 | 0 | 0 | 0 | 3,000/3,000 not applicable |
| Sealed | 1,000 | 0 | 0 | 0 | 1,000/1,000 not applicable |

The development first-gate counts are language normalization 485, missing
knowledge 2,097, missing method 372, missing prerequisite 40, and
representation gap 6.  The sealed counts are language normalization 182,
missing knowledge 678, missing method 132, and missing prerequisite 8.

Producer commit: `2484254`.  Question, source-manifest, and oracle hashes are
recorded in the split JSON reports.  No registry or curriculum mutation
occurred, and no answer was authorized.  This is the current frozen baseline;
it is not a claim of HLE transfer.

