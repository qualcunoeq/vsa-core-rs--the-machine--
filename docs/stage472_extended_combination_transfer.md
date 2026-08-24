# Stage 472 — extended-combination portfolio transfer

This checkpoint evaluates the frozen extended-combination fallback in the
answer-key-blind route portfolio. Sealed scoring uses only the privileged
hash oracle; no plaintext answers are exposed to the route or implementation.

| Metric | Development | Sealed |
|---|---:|---:|
| Questions / answer hashes | 3000 / 3000 | 1000 / 1000 |
| Route invocations | 60000 | 20000 |
| Unique candidates | 52 | 14 |
| Correct candidate hashes | 51 | 11 |
| Incorrect candidates rejected | 1 | 3 |
| Candidate replay | 52/52 | 14/14 |
| No executable route | 2948 | 986 |
| False authorizations | 0 | 0 |
| Manifest unchanged | true | true |

The extension adds five correct development candidates relative to the prior
remainder checkpoint and three correct sealed candidates. The frozen sealed
shadow result therefore rises from **8/1000 to 11/1000**. One development
counting candidate and three sealed candidates remain hash-mismatches and were
rejected; none authorized production behavior.

The route remains shadow-only. The old bounded-counting evaluator and its
pressure corpus retain their original scope.
