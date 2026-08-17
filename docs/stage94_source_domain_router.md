# Stage 94 — mixed source-domain routing

This checkpoint tests the two newly source-derived domains together rather than
as isolated packs. A route-blind corpus interleaves bounded linear interpolation
and bounded Bayes requests, plus ambiguous and unsupported near-misses. Both
frontends inspect every case, but execution is authorized only when exactly one
typed route is complete.

## Results

| metric | result |
|---|---:|
| cases | 480 |
| interpolation / Bayes / ambiguous / unsupported | 120 / 120 / 120 / 120 |
| route decisions correct | 480/480 |
| authorized routes | 240/240 |
| ambiguity preserved | 120/120 |
| unsupported refused | 120/120 |
| replay verified | 480/480 |
| tamper rejected | 480/480 |
| provenance preserved | 480/480 |
| route leakage | 0 |
| false authorizations / denials | 0 / 0 |

The development, validation, and sealed partitions contain the same four-way
distribution (288/96/96 cases respectively). The full receipts and source
lineage hashes are in [stage94_source_domain_router.json](stage94_source_domain_router.json).

The result demonstrates that source-derived formula execution and the existing
finite-probability Bayes bridge compose through route selection without treating
interpolation text as probability evidence or probability text as an
interpolation request. No production registry or live routing state is changed.
