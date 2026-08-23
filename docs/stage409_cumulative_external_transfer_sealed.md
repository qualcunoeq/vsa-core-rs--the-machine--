# Stage 409 — cumulative external transfer

- Partition / questions: sealed / 1000
- Legacy routes / new source routes: 12 / 2
- Baseline unique / cumulative unique / cumulative ambiguities: 4 / 7 / 0
- New-route unique / incremental over baseline: 3 / 3
- Base frontend/execution complete: 1 / 1
- Complex frontend/execution complete: 2 / 2
- Base replay / tamper (frontend + execution): 1001 / 1001
- Complex replay / tamper (frontend + execution): 1002 / 1002
- Answer hashes / plaintext answers read: 1000 / 0
- Correct uniquely selected candidates: 3
- Production authorizations / false authorizations: 0 / 0
- Source-selection replay: base=true complex=true
- Manifest unchanged: true

This is a route-blind shadow checkpoint combining the independently validated Stage 407 positional-arithmetic and Stage 408 rectangular-complex-arithmetic routes. Sealed scoring reads only answer hashes under the explicit privileged boundary; no plaintext answers are loaded and production routing is unchanged.
