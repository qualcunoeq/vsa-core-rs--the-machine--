# Stage 470 — literal-remainder portfolio transfer

This checkpoint evaluates the frozen remainder bridge in the route-blind Goal
6 portfolio. Sealed scoring uses answer hashes only under the explicit
privileged boundary.

| Metric | Development | Sealed |
|---|---:|---:|
| Questions / answer hashes | 3000 / 3000 | 1000 / 1000 |
| Route invocations | 60000 | 20000 |
| Unique candidates | 46 | 11 |
| Correct candidate hashes | 46 | 8 |
| Incorrect candidates rejected | 0 | 3 |
| Candidate replay | 46/46 | 11/11 |
| No executable route | 2954 | 989 |
| False authorizations | 0 | 0 |
| Manifest unchanged | true | true |

The remainder bridge adds four correct development candidates:
`math-v1-development-0442`, `-0722`, `-1005`, and `-1283`. It adds one
correct sealed candidate, `math-v1-sealed-0195`, raising the frozen portfolio
shadow result from **7/1000 to 8/1000**. The three mismatches were rejected by
hash comparison and did not authorize production behavior.

Evidence:

* development probe report: `8c33ba55c0e71d103390f9eb62e2da2b62650dd76166beab16049618c494559a`
* development score report: `fe14fc39e98525822396279c0ebf61503db10f508087067d602354a8784644b4`
* sealed probe report: `0baf994bf4cb7ed3a8d92284b449fb967d8211290769dd1bc37f8d673a7adc9a`
* sealed score report: `8af8f474937dd584cc6efd75ae8c530a06ca0422c4d9eb0cd10ea0cdfae7d0e7`

The route remains shadow-only.
