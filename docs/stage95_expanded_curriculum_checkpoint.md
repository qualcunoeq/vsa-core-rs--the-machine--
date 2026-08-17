# Stage 95 — expanded curriculum checkpoint

This is a lineage-preserving sealed checkpoint after adding the two new
source-derived domains. It does not rewrite or rerun the prior 5,000-question
holdout. Instead, it aggregates the immutable Stage-K report with the
independently generated Stage-94 interpolation/Bayes transfer corpus.

## Aggregate results

| metric | result |
|---|---:|
| total cases | 5,480 |
| supported / ambiguous / unsupported | 3,240 / 1,120 / 1,120 |
| supported authorized | 3,240/3,240 |
| ambiguities preserved | 1,120/1,120 |
| unsupported refused | 1,120/1,120 |
| replay verified | 5,480/5,480 |
| tamper rejected | 5,480/5,480 |
| provenance preserved | 5,480/5,480 |
| false authorizations / denials | 0 / 0 |
| route leakage | 0 |

## Sealed holdout lineage

The combined sealed lineage contains **1,096** cases:

* 600 supported, 200 ambiguous, 200 unsupported from the immutable Stage-K
  holdout;
* 24 interpolation, 24 Bayes, 24 ambiguous, and 24 unsupported cases from the
  Stage-94 sealed partition.

The combined sealed results are 648/648 supported authorizations, 224/224
ambiguities preserved, 224/224 unsupported refusals, and 1,096/1,096 replay and
tamper checks, with zero false authorizations or denials.

The machine-readable report records both parent report hashes and the current
curriculum manifest hash: [stage95_expanded_curriculum_checkpoint.json](stage95_expanded_curriculum_checkpoint.json).
The aggregate is intentionally not presented as a newly authored 5,480-case
corpus; its immutable lineage is explicit.
