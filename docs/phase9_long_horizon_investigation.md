# Phase 9 — Long-Horizon Governed Investigation

Phase 9 runs the epistemic loop across bounded multi-step episodes rather than
one static evidence snapshot:

```text
objective → inspect → select query → receive evidence → diagnose → revise
→ replan → stop or continue
```

Query selection is typed and deterministic. It uses exact information-gain
assessments, refuses already-used correlation groups, and removes known-failed
evidence from planning without deleting it from the replay trace.

## Synthetic benchmark

The corpus contains 300 episodes, each with a five-step budget:

| Scenario | Cases |
| --- | ---: |
| Clear supported investigations | 120 |
| Disconfirming observation and revision | 50 |
| Correlated evidence trap | 50 |
| Missing hypothesis | 40 |
| Irreducibly unresolved | 40 |

Corpus hash:

`3478bc33f987d42bbf711c17d3f50be5b0c60b2218d077a5fad7679801e50a71`

Measured result:

| Metric | Result |
| --- | ---: |
| Correct terminal outcome | 300 / 300 |
| Unsupported actions | 0 |
| Redundant correlation-group queries | 0 |
| Premature resolutions | 0 |
| Failure to revise after contradiction | 0 |
| Hypothesis thrashing | 0 |
| Evidence-budget waste | 0 |
| False certainty | 0 |
| Replay-verified episode receipts | 300 / 300 |

## Boundaries

This phase uses a bounded synthetic environment and a fixed query catalog. It
does not infer objectives from raw language, invent new actions, execute real
evidence collection, or mutate the world-model registry. The next pressure
phase should vary query costs, introduce delayed observations, and test
longer episodes with changing source reliability before any persistent agency
is considered.
