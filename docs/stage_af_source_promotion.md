# Stage AF — governed promotion of a source-derived capability

| Measure | Result |
| --- | ---: |
| Cases | 240 |
| Source preflight | 240/240 |
| Exact promotion decisions | 240/240 |
| Promotions / blocked or denied | 90 / 150 |
| Registry replay / tamper rejection | 240/240 / 240/240 |
| Later source mutations rejected | 50/50 |
| Regressions / rollbacks | 50 / 50 |
| World-state preservation / historical replay | 50 / 50 |
| False authorizations / denials | 0 / 0 |
| Live registry/world-model mutation | 0 / 0 |

The source-derived candidate remains clone-only. A malformed later source catalog is treated as a counterexample and rolled back without changing the accumulated world-state hash.

Reproduce with:

```text
cargo run --quiet --bin stage_af_source_promotion
```

Machine-readable report: `docs/stage_af_source_promotion.json`
