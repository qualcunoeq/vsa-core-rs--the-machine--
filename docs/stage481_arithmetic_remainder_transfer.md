# Stage 481 — arithmetic-remainder portfolio transfer

The frozen arithmetic-expression remainder route was evaluated through the
answer-key-blind development and sealed portfolio. Sealed scoring reads only
the privileged answer-hash oracle. No production route, registry, or manifest
was changed.

| Metric | Development | Sealed |
|---|---:|---:|
| Questions / answer hashes | 3000 / 3000 | 1000 / 1000 |
| Route invocations | 72000 | 24000 |
| Unique candidates | 61 | 18 |
| Correct candidate hashes | 60 | 14 |
| Incorrect candidates rejected | 1 | 4 |
| Candidate replay | 61/61 | 18/18 |
| No executable route | 2939 | 982 |
| False authorizations | 0 | 0 |
| Manifest unchanged | true | true |

The route adds four correct development candidates and two correct sealed
candidates. The sealed shadow result rises from **12/1000 to 14/1000**. The
remaining candidates were rejected by hash comparison; all accepted traces
replay and no production authorization occurs.

Representative accepted cases include development IDs
`math-v1-development-0899`, `math-v1-development-1660`,
`math-v1-development-1983`, and `math-v1-development-2499`, plus sealed IDs
`math-v1-sealed-0161` and `math-v1-sealed-0860`.

Evidence:

* development probe report SHA-256: `aef50503af67f1384bec10702c483795f96997635bc7db3341ed865020317339`
* development score report SHA-256: `c703a337c54c5c705ae03651854d48be9b43433253b7cc64cdfa13e57a3095a5`
* sealed probe report SHA-256: `c9fa10e3b31635be396508f5abbc1508fa08f8a2197f56b9be4e096419d5208c`
* sealed score report SHA-256: `4956290df3822357ab26e74dc4c6bea84be76fe1832aaea3db75a42bd499a5fe`

