# Stage AD — promotion under environment drift

Stage AD composes the governed promotion lifecycle with the independent
protocol environment. A candidate is staged only in a cloned registry. Clean
deployments execute through the protocol; a later-counterexample deployment
uses a degraded candidate, detects the resulting regression, proposes rollback,
restores the historical active version, and replays the environment episode.

The 300-case corpus contains 50 cases each for clean promotion, policy denial,
dependency conflict, migration failure, competing boundaries, and later
counterexamples.

Run:

```text
cargo run --quiet --bin stage_ad_promotion_environment
```

The machine-readable report is `docs/stage_ad_promotion_environment.json`.

| Metric | Result |
| --- | ---: |
| Cases | 300 |
| Exact promotion decisions | 300/300 |
| Staged promotions | 100 |
| Blocked or denied | 200 |
| Environment executions / replays | 100 / 100 |
| Environment tamper rejections | 100/100 |
| Post-deployment regressions detected | 50/50 |
| Rollback proposals / applications | 50 / 50 |
| Accumulated world-state preservation | 50/50 |
| Historical replays after rollback | 50/50 |
| Registry receipt replay / tamper | 300/300 each |
| False authorizations / denials | 0 / 0 |
| Production registry/world-model mutations | 0 / 0 |

This is still shadow-only. It demonstrates recovery from a later discovered
counterexample without allowing the candidate to mutate live routing.
