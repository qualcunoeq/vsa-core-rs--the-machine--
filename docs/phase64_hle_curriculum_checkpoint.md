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

