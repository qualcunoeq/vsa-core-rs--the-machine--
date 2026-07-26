# Phase 8 — Adversarial Evidence and Dependency Modeling

Phase 8 hardens epistemic updates against evidence that is duplicated,
correlated, stale, causally incompatible, or produced by a known source
failure.

## Evidence model

Each evidence record now carries:

* ancestry and an explicit correlation group;
* a typed source-failure mode (`ClockDrift`, `IdentityConfusion`,
  `CopiedReport`, `StaleCache`, `SelectiveOmission`, or
  `AdversarialFabrication`);
* an optional causal path from hypothesis event to observation process;
* the existing timestamp, validity, reliability, and confidence metadata.

Only one claim per query/origin/outcome contributes to support. Known failure
records are retained for diagnosis but excluded from positive support. Outcomes
are selected by independent-origin count, not raw observation count. A
hypothesis must also have a causally compatible path when one is declared.

## Synthetic benchmark

The corpus contains 300 investigations:

| Family | Cases |
| --- | ---: |
| Duplicated evidence from one origin | 40 |
| Correlated sensors | 40 |
| Copied testimony | 30 |
| Clock drift | 30 |
| Identity confusion | 30 |
| Stale cache | 30 |
| Adversarial fabrication | 30 |
| Omitted true hypothesis | 30 |
| Causal incompatibility | 20 |
| Genuine unresolved ties | 20 |

Corpus hash:

`8153d808867647d3f200b4d3a91ed15d8176fdc0174b7181dc51c95ed61befd7`

Measured result:

| Metric | Result |
| --- | ---: |
| Outcome classification | 300 / 300 |
| Known-truth resistance | 230 / 230 |
| Correlation-aware handling | 300 / 300 |
| Source-failure identification | 300 / 300 |
| Correlation overcount cases | 140 / 140 |
| Causal compatibility checks | 300 / 300 |
| Replay-verified receipts | 300 / 300 |

## Boundaries

This phase does not learn source reliability, infer causal graphs from raw
language, execute evidence collection, or publish revised beliefs. The known
truth is deliberately retained as benchmark metadata only. The next pressure
phase should vary query costs, inject common-mode failures not declared in the
metadata, and test whether the investigator requests independent evidence
instead of simply rejecting correlated reports.
