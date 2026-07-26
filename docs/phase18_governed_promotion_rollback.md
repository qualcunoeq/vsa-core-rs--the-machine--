# Phase 18 — Governed Promotion and Rollback

Phase 18 exercises a complete capability lifecycle in a cloned versioned
registry:

```text
shadow candidate
→ policy/dependency/migration checks
→ staged promotion receipt
→ induced drift or counterexample
→ rollback
→ historical replay
```

Promotion requires frozen holdout success, zero false authorization, bounded
regressions, dependency compatibility, migration safety, and explicit policy
authorization. Competing candidates for the same semantic boundary are
blocked. The live registry is never touched by this benchmark.

## Scenarios

The 240-case corpus covers clean promotion, regression blocking, dependency
conflicts, migration failures, behavior drift, later counterexamples, rollback
with accumulated world-state hashes, historical replay, and competing
proposals.

Corpus SHA-256:

`21b34010d8fd6d876f2be7523ed81442039f0e8bc914d92bb80e08aea4317812`

## Results

| Metric | Result |
| --- | ---: |
| Lifecycle decisions | 240 / 240 |
| Clean promotions | 80 |
| Blocked/denied proposals | 160 |
| Rollbacks | 40 |
| World-state hashes preserved | 40 / 40 |
| Historical replays | 40 / 40 |
| Competing-boundary conflicts | 20 |
| Replay receipts | 240 / 240 |
| Tamper checks rejected | 240 / 240 |
| Live registry mutations | 0 |

Promotion remains policy-gated and rollback restores a historical active version
without rewriting accumulated world state.
