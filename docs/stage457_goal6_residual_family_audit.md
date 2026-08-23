# Stage 457 — Goal 6 residual-family audit

This answer-key-blind audit runs after the 18-route portfolio (including the
Bayes shadow route) over the 3,000-question development partition. It records
the full route-status signature and ranks explicit missing-field diagnoses
above generic route-level ambiguity. Ties are retained; no semantic family or
source is selected automatically.

| Metric | Result |
|---|---:|
| Development questions | 3000 |
| Residual questions | 2958 |
| Routes offered | 18 |
| Answer keys read | 0 |
| Distinct status signatures | 192 |
| Best frontend rank 4 (complete, execution boundary) | 8 |
| Best frontend rank 3 (explicit missing fields) | 2950 |
| Manifest unchanged | true |

The audit confirms that the prior aggregate `ambiguous` count was not a
reusable capability signal: most residuals expose missing typed fields across
many routes, with numerous ties. Lexical token clusters are retained only as
hashed triage evidence and do not authorize routing or source ingestion.

Evidence:

* report SHA-256: `28c1fb70166c0edf5bcc8e4b0cb603a8fe3c1d187451f57d8f67c7f13e82b785`
* report file SHA-256: `296fcdcc98e9ebbf12dd1b2f87e66265c434189f20bf1050d667aba20412c9e8`

The next acquisition decision remains blocked on an independently validated
semantic family, not on lexical overlap.
