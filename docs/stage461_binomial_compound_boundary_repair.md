# Stage 461 — binomial compound-expression boundary repair

The first binomial transfer exposed three development candidates where the
frontend executed only the first term of a compound expression. The repaired
boundary now rejects multiple binomial operators and explicit compound
operators before execution. This preserves the validated single-coefficient
route while eliminating shadow false candidates.

| Metric | Development | Sealed |
|---|---:|---:|
| Questions / answer hashes | 3000 / 3000 | 1000 / 1000 |
| Route invocations | 54000 | 18000 |
| Unique candidates | 39 | 10 |
| Correct candidate hashes | 39 | 7 |
| Incorrect candidates rejected | 0 | 3 |
| Candidate replay | 39/39 | 10/10 |
| No executable route | 2961 | 990 |
| False authorizations | 0 | 0 |
| Manifest unchanged | true | true |

The sealed result remains **7/1000**, while the compound-expression repair
reduces executable-but-wrong candidates and raises the development result from
39/42 candidates with three rejected mismatches to **39/39** correct
candidates. The two sealed binomial answers remain `math-v1-sealed-0076` and
`math-v1-sealed-0355`.

Focused frontend tests: 9/9 passed. The independent Stage 458 corpus remains
240/240 exact with zero false authorization.

Evidence:

* development probe report: `226a2492a06a728d1fa59197e32157412752330957e3df5e233fdd4758befde4`
* development score report: `a79329dc849af717b90e04fd02d8b1096d9b1bb544ce66a47d71a80eb9e245ce`
* sealed probe report: `2d3d976765217a93d8a757558f08ffb633ea32bd525f299eefab8e99bb0f2d01`
* sealed score report: `8de852c5c9c17fdf141a92fb98c9f73aa73c67ada8780708be43b9e2a4546339`

This remains shadow-only; no production authorization or registry mutation
occurred.
