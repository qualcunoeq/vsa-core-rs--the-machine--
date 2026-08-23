# Stage 447 — sealed external portfolio checkpoint

This is the first sealed evaluation of the current 15-route Goal 6 portfolio.
The evaluator was invoked explicitly in privileged mode; development code did
not receive sealed answers, and no production authorization or curriculum
mutation occurred.

## Result

| Metric | Result |
|---|---:|
| Sealed questions / answer hashes read | 1000 / 1000 |
| Unique shadow candidates | 5 |
| Correct candidate hashes | 2 |
| Incorrect candidate hashes rejected | 3 |
| Candidate replay verification | 5 / 5 |
| Multiple-route ambiguities / no executable route | 0 / 995 |
| Plaintext answers read | 0 |
| Production authorizations / false authorizations | 0 / 0 |
| Registry or curriculum mutation | 0 |

The two correct sealed candidates used `ArithmeticSequence` and
`BaseConversion`. The three incorrect candidates were rejected by the
hash-only evaluator and were not authorized. This is a transfer result, not a
claim of five correct answers.

The development checkpoint immediately preceding this run was 24/24 correct
among 24 unique candidates on 3,000 development questions. The sealed result
therefore gives a measured external generalization point: **2 correct shadow
candidates out of 1,000 sealed questions, with zero false authorization**.

## Evidence

* dataset SHA-256: `3cf924116a0f8f6a0c84d0ce7949b0c1e16221e0d4b5fcb0c4322110e30714f2`
* sealed probe report SHA-256: `acfbefddac3790ff9aebaa380b35faab336877164b264df4930e3949fb61e30e`
* sealed score report SHA-256: `81c2f3872d0e3edf1a3cb898c7d7587220f8bd42af0ffe063fa64d324a81c9b2`

Reproduce only as a deliberate sealed checkpoint:

```text
GOAL6_PORTFOLIO_PARTITION=sealed \\
GOAL6_PORTFOLIO_PRIVILEGED_EVAL=true \\
  cargo run --quiet --bin goal6_external_portfolio_probe

GOAL6_PORTFOLIO_PARTITION=sealed \\
GOAL6_PORTFOLIO_PRIVILEGED_EVAL=true \\
  cargo run --quiet --bin goal6_external_portfolio_score
```
