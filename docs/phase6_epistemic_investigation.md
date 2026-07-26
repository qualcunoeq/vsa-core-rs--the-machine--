# Phase 6 — Hypothesis Testing and Information Seeking

Phase 6 adds a bounded epistemic layer above the replayable world ledger. It
compares explicit hypotheses, predicts outcomes for typed evidence queries,
preserves shared predictions, and recommends the next discriminating
observation. Recommendations are diagnostic only: they do not authorize
collection, execution, or registry mutation.

## Evidence policy

Evidence is timestamped and may expire through an inclusive `valid_until`
interval. Reliability and confidence combine deterministically as
`reliability × confidence`. Only evidence at or above the fixed decisive score
of `2,000` can eliminate hypotheses; weaker evidence remains a clue but cannot
collapse uncertainty. Equal-strength conflicting sources preserve all
compatible hypotheses.

## Information gain

For a query partitioning `N` plausible hypotheses into groups of sizes `nᵢ`,
the exact gain is represented as:

```text
(N² − Σ nᵢ²) / N
```

This avoids floating-point tie instability. Queries with equal maximal gain
produce `Ambiguous`; queries whose predictions are shared by every hypothesis
are not recommended.

## Synthetic benchmark

The controlled corpus contains 300 investigations:

| Family | Cases |
| --- | ---: |
| Clear discriminating queries | 120 |
| Redundant prior evidence | 60 |
| Weak misleading sensor evidence | 50 |
| Stale/delayed evidence | 40 |
| Genuinely unresolved alternatives | 30 |

Corpus hash:

`0689f2d6e12180d991522edd593ead8bf2fc7459028afcaebc19f7b2c66839e5`

Measured result:

| Metric | Result |
| --- | ---: |
| Cases | 300 / 300 |
| Ground-truth hypothesis retained | 300 / 300 |
| Predictions represented | 300 / 300 |
| Recommendation matches oracle | 300 / 300 |
| Ambiguities preserved | 30 / 30 |
| Belief updates retain supported truth | 300 / 300 |
| Calibration/uncertainty safe | 300 / 300 |
| Replay-verified receipts | 300 / 300 |

## Boundaries

This phase does not infer hypotheses from raw language, learn source
reliability, execute evidence collection, or publish beliefs to the live fact
index. The next pressure phase should inject contradictory high-confidence
sources, malformed predictions, query costs, stale evidence, and tampered
belief updates before any world-model deployment path is considered.
