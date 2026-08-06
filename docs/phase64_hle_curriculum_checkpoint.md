# Phase 64 — Frozen HLE curriculum checkpoint

This checkpoint evaluates the unchanged frozen `data/hle.jsonl` export after
the Phase 63 curriculum milestone. The existing router is observed for the
baseline answer/replay path, while curriculum signals are recorded in a
strictly shadow-only funnel. No curriculum pack is invoked or promoted from
HLE text without a complete typed formalizer.

```text
question
→ curriculum signal
→ strict typed candidate
→ pack invocation (shadow only)
→ candidate answer
→ replay/reference match
```

Each question also receives a first-failure classification: language or
notation grounding, unsupported target, missing prerequisite, missing theorem,
representation gap, unverified assumptions, pack boundary, answer
equivalence, visual dependency, or no curriculum signal.

Run:

```text
cargo run --bin hle_curriculum_checkpoint
```

The run writes a per-question trace and summary under `/tmp` by default. The
summary records the producer commit, dataset hash, curriculum-manifest hash,
route funnel, first-failure counts, replay counts, and resource timing. The
checkpoint is diagnostic only; production routing and authorization remain
unchanged.

## Frozen result

The machine-readable summary is [phase64_hle_curriculum_checkpoint.json](phase64_hle_curriculum_checkpoint.json).

| Metric | Result |
|---|---:|
| Questions | 2,500 |
| Correct authorized answers | 2 |
| Incorrect authorized answers | 0 |
| False authorizations | 0 |
| Curriculum signals | 705 |
| Shadow pack invocations | 0 |
| Visual dependencies | 260 |
| No curriculum signal | 1,614 |
| Language normalization failures | 65 |
| Missing factual prerequisites | 451 |
| Missing specialist theorems | 87 |
| Unsupported target type | 13 |
| Assumptions not established | 8 |

Replay was `not_applicable` for 2,498 abstentions and `not_recorded` for the
two baseline authorized answers; no replay failure was observed.

The unchanged baseline remains **2/2,500**. The zero invocation count is
intentional: current curriculum packs require complete typed formalizers, so
keyword-level HLE signals are recorded as candidates but never promoted to
pack execution. The checkpoint therefore identifies semantic grounding and
typed target construction—not permissive curriculum matching—as the next
bottleneck.
