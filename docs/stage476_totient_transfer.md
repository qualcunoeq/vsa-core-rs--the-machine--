# Stage 476 — canonical totient portfolio transfer

This checkpoint evaluates the frozen literal Euler-totient/unit-count route
in the answer-key-blind Goal 6 portfolio. Sealed scoring uses only the
privileged hash oracle.

| Metric | Development | Sealed |
|---|---:|---:|
| Questions / answer hashes | 3000 / 3000 | 1000 / 1000 |
| Route invocations | 66000 | 22000 |
| Unique candidates | 56 | 15 |
| Correct candidate hashes | 55 | 12 |
| Incorrect candidates rejected | 1 | 3 |
| Candidate replay | 56/56 | 15/15 |
| No executable route | 2944 | 985 |
| False authorizations | 0 | 0 |
| Manifest unchanged | true | true |

The transfer run is executed only after the independent Stage 475 corpus
passes. It adds one correct development candidate (`math-v1-development-2591`)
and no sealed candidate. The sealed shadow result remains **12/1000**. The
one development and three sealed mismatches were rejected by hash comparison;
false authorization remains zero. No production authorization occurs.

Evidence:

* development probe report: `014fcac10dd8a8e9d7c5c4e953cfb9ebc1e9e1d78b3b2469ec85fa398f769016`
* development score report: `7bc181432a8d4e6335dd33c373427009ac749d2dbec547c1eb51e8a6c69d3cbf`
* sealed probe report: `6de6ed9f4f91a2df4462060e664be0f2f6239b8d3e6836607245dea2be563848`
* sealed score report: `fec65c6c7167ab3a7dbc0983ff54a1b7da1912f94e8893b8d7a3dcb5bb7654c7`
