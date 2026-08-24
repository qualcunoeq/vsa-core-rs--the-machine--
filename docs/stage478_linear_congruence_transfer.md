# Stage 478 — canonical linear-congruence portfolio transfer

The frozen congruence route was evaluated after the independent Stage 477
pressure corpus. Development and sealed scoring use the answer-key-blind
portfolio probe; sealed scoring reads only the privileged hash oracle. No
production route, registry, or curriculum manifest was changed.

| Metric | Development | Sealed |
|---|---:|---:|
| Questions / answer hashes | 3000 / 3000 | 1000 / 1000 |
| Route invocations | 69000 | 23000 |
| Unique candidates | 57 | 16 |
| Correct candidate hashes | 56 | 12 |
| Incorrect candidates rejected | 1 | 4 |
| Candidate replay | 57/57 | 16/16 |
| No executable route | 2943 | 984 |
| False authorizations | 0 | 0 |
| Manifest unchanged | true | true |

The route adds one correct development candidate (`math-v1-development-2590`)
and one sealed candidate that is rejected by the hash oracle
(`math-v1-sealed-0829`). The sealed shadow result therefore remains
**12/1000**. Both candidate traces replay, and no candidate authorizes
production behavior.

Evidence:

* development probe report SHA-256: `b8276588b4f405ffde1226b472d731ee85d3dabfb43683d884100a7abc11e6e3`
* development score report SHA-256: `2fcd23a03108c97ef9974e3b844d3808943707a89a91a4d8096f8f56f90f6850`
* sealed probe report SHA-256: `3274d2425ca349a5d35480fa11b980831bad48027fc7572ed45936aca3a8878a`
* sealed score report SHA-256: `2d2fe401faac6736aa2b7a6804d3e6bd438602fe20502c970d7e2df99a08ff36`
* serialized report files were generated under `/tmp` and are not part of the
  tracked repository; the hashes above are the immutable evidence references.

