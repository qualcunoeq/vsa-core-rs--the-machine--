# Phase 5 — Persistent World-Model Substrate

Phase 5 adds a bounded, deterministic world model above the finite-state
transition capability. It is a synthetic proving ground, not an autonomous
world simulator or a production belief store.

## Evidence layers

The substrate keeps four concepts distinct:

```text
Observation  →  observed claim from a named source
Derived      →  state produced by a verified transition
Hypothesis   →  unresolved competing interpretation
World state  →  current replayable belief per entity
```

Entities have stable typed identities. Observations carry timestamps, source
identity, reliability, confidence, and an optional inclusive validity interval,
plus a typed value.
Events carry timestamps and source metadata and are applied only when a
deterministic transition rule and any required Boolean guard are available.

## Deterministic replay

`replay_investigation` sorts observations and events by `(timestamp, id)`,
updates entity beliefs, records contradictions and missing evidence, applies
guarded transitions, and emits an immutable `WorldReplayReceipt`. The receipt
hash covers the complete update trace, final beliefs, and diagnostic counters.
Tampering with a trace or counter therefore fails `replay_verified()`.

Impossible transitions are rejected. Missing current state or missing guard
evidence is reported separately as `MissingEvidence`. Equal-strength
conflicting observations remain competing hypotheses rather than being
silently collapsed.

## Synthetic benchmark

The initial corpus has 240 deterministic investigations:

| Family | Cases |
| --- | ---: |
| Valid multi-event trajectories | 100 |
| Equal-strength contradictions | 40 |
| Impossible events | 40 |
| Missing initial evidence | 30 |
| Lower-confidence conflicting observations | 30 |

Corpus hash:

`eca5c0b2c29c2e37fb21d99f1c72b84b6208587d3c8df451336c00ba788672f0`

Measured result:

| Metric | Result |
| --- | ---: |
| Exact expectation matches | 240 / 240 |
| Replay-verified receipts | 240 / 240 |
| Contradictions detected | 70 |
| Impossible events rejected | 40 |
| Missing-evidence reports | 70 |
| Competing hypotheses preserved | 80 |

The valid cases demonstrate state evolution and derived claims. Contradiction
cases preserve equal-strength alternatives; lower-confidence conflicts retain
the stronger belief while still recording the contradiction. Missing evidence
never becomes an implicit authorization.

## Boundaries

This phase does not infer entity identity from raw language, learn transition
rules, merge external knowledge bases, or publish beliefs to the live fact
index. Multiple source identities are retained, and source reliability is
explicit metadata, not an automatically learned trust score. The next pressure
phase should mutate observations, source
reliability, event ordering, guard claims, and replay traces before any
persistent deployment path is considered.
