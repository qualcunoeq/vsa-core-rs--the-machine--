# Phase 10 — Independent Environment Boundary

Phase 10 places the investigation controller behind a narrow protocol. The
environment owns hidden state, scenario behavior, timing, costs, and expected
terminal truth. The controller receives only action acknowledgements,
observations, source metadata, delays, and environment events.

```text
MachineAction → ExternalEnvironment → delayed/noisy EnvironmentReply
```

The controller cannot access scenario labels or hidden truth. It stops after
independent evidence agrees, or emits a justified abstention.

## Synthetic independent corpus

The corpus contains 300 protocol episodes:

| Scenario | Cases |
| --- | ---: |
| Clean observations | 80 |
| Delayed responses | 50 |
| Unavailable primary query | 40 |
| World changes between actions | 40 |
| Deceptive source | 30 |
| Unknown entity | 30 |
| Irresolvable evidence | 30 |

Corpus hash:

`ce1a99fd5f82c6dc0fae5d9d58b780aa7032c8adae1eadf61ad05990f232159b`

Measured result:

| Metric | Result |
| --- | ---: |
| Terminal truth or justified abstention | 300 / 300 |
| Calibrated abstentions | 300 / 300 |
| Unnecessary actions after resolution | 0 |
| Delayed-response recovery | 300 / 300 |
| Unexpected-event recovery | 300 / 300 |
| Unsupported actions | 0 |
| Action-budget violations | 0 |
| Replay-verified protocol receipts | 300 / 300 |

## Boundaries

This is an independently structured synthetic protocol, not natural-language
ingestion or a live external system. The environment is deterministic per
episode and does not expose its hidden state to the controller. The next
pressure phase should introduce stochastic seeds, asynchronous observations
that arrive between actions, unknown query semantics, and longer episodes.
