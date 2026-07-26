# Phase 7 — Open-Set Hypothesis Management

Phase 7 detects when the active hypothesis set cannot explain reliable
evidence. It emits a bounded, falsifiable hypothesis proposal without adding
that proposal to active belief.

## Outcomes

```text
BestKnownHypothesis
MultiplePlausibleHypotheses
NoAdequateHypothesis
NovelHypothesisNeeded
InsufficientEvidence
```

Reliable evidence is evaluated using the Phase 6 policy: timestamp validity and
the `reliability × confidence >= 2,000` decisive threshold. A residual
observation is unexplained when no active hypothesis predicts its outcome.

## Diagnostic proposal

Each novel proposal records:

* unexplained observation IDs;
* the minimum latent cause required to explain the residual;
* predicted outcomes;
* overlap with existing hypotheses;
* introduced assumptions;
* falsification conditions;
* exact expected information gain of testing it.

Proposals are immutable diagnostics. They are never promoted automatically.

## Synthetic benchmark

The corpus contains 300 deterministic investigations:

| Family | Cases |
| --- | ---: |
| True hypothesis included | 80 |
| True hypothesis omitted | 80 |
| Inadequate active set | 40 |
| Misleading high-confidence evidence | 30 |
| Correlated sensors | 20 |
| Stale accurate evidence | 20 |
| Adversarial unexplained evidence | 20 |
| No preferred hypothesis | 10 |

Corpus hash:

`30511af3a69d9bfd628b43d5192a99a8d4ece8587e889033bafa93f64265facf`

Measured result:

| Metric | Result |
| --- | ---: |
| Outcome classification | 300 / 300 |
| Missing-hypothesis detection | 140 / 140 |
| Falsifiable proposal quality | 140 / 140 |
| Recovery after hidden hypothesis introduction | 140 / 140 |
| Known-truth calibration under adversarial evidence | 120 / 150 |
| Recommendation/proposal quality | 300 / 300 |
| Replay-verified receipts | 300 / 300 |

The 120/150 known-truth calibration result is intentional: 30 misleading
high-confidence cases cause the best-known active hypothesis to differ from
the hidden truth. Those cases demonstrate the misspecification pressure that
motivates open-set proposals rather than being counted as successes.

## Boundaries

This phase does not infer latent causes from raw language, add hypotheses to
belief, learn source reliability, or execute evidence collection. The next
pressure phase should inject multiple residual anomalies, correlated evidence
with common-mode failures, malformed proposals, competing novel causes, and
cases where every current hypothesis is plausible but jointly inadequate.
