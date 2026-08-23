# Stage 456 — source-derived Bayes route transfer

This checkpoint evaluates the independently pressure-tested source-derived
Bayes route inside the route-blind Goal 6 portfolio.  Development and sealed
partitions were scored separately; no plaintext sealed questions were read,
and the registry and curriculum manifest remained unchanged.

| Metric | Development | Sealed |
|---|---:|---:|
| Questions / answer hashes | 3000 / 3000 | 1000 / 1000 |
| Route invocations | 54000 | 18000 |
| Unique candidates | 26 | 8 |
| Correct candidate hashes | 26 | 5 |
| Candidate replay verification | 26/26 | 8/8 |
| No executable route | 2974 | 992 |
| Bayes candidates | 0 | 0 |
| False authorizations | 0 | 0 |
| Manifest unchanged | true | true |

The Bayes frontend and source catalog are independently validated by Stage 93
(180 supported, 60 ambiguous, and 60 unsupported cases with zero false
authorizations).  They correctly produced no portfolio candidates here: the
external portfolio contains no explicit, safely bound prior/likelihood/evidence
request at the current language boundary.  The existing five sealed correct
candidates remain unchanged (`3/1000` before the complex route and `5/1000`
after it).

This is a transfer-negative result, not a Bayes capability failure.  It
prevents broad probability vocabulary from becoming a route and supplies
evidence for selecting the next source pack from measured external overlap.

Evidence:

* development probe file SHA-256: `bd0d11bff9b1d9c7f1aed7ddca6d79704b81ded5e4241c347bde3f80c707b483`
* development score file SHA-256: `62f75afc079b0e5b52d2bbdb81412307c2e30a5e2879f0bcd95931992ec0c27b`
* sealed probe file SHA-256: `feb2c3d01ea2817e38f897827c31905356174001c693a304bf2ce3c84cf5a23b`
* sealed score file SHA-256: `9b8748d5191a9029905861316778591446b3edf37018af55a6fb4e75cd16caf5`
* source-derived Bayes pressure evidence: `docs/stage93_source_bayes_bench.md`

The route remains shadow-only.  No production authorization was added.
