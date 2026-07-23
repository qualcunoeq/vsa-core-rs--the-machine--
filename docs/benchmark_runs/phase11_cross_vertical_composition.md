# Phase 11 — Cross-vertical composition

This phase tests whether independently governed verticals can hand off typed
artifacts without laundering authority across the boundary.  The benchmark is
separate from the per-vertical corpora and uses an independent Python oracle.

## Scope

The corpus contains 340 cases:

- 100 recurrence → algebra compositions;
- 100 algebra → recurrence compositions;
- 80 recurrence → linear-system compositions;
- 20 forged intermediate artifacts;
- 10 incompatible typed handoffs;
- 10 unsupported recurrence stages;
- 10 semantic rewrite pairs (20 cases).

Each accepted case must replay both stages.  The second stage receives an
artifact only after the first stage's exact result and replay receipt pass.  A
forged intermediate or mismatched artifact kind is rejected before stage two
executes.

## Reproduction

```bash
python3 scripts/generate_cross_vertical_ood.py
cargo run --release --quiet --bin cross_vertical_bench -- data/cross_vertical_ood_v1.json
cargo test --release --lib cross_vertical_benchmark -- --nocapture
```

## Result

```text
cases=340
authorized=300
false_auth=0
false_denials=0
intermediate_replay=300
final_replay=300
forged_rejected=20
incompatible_rejected=10
rewrite_pairs=10
rewrite_regressions=0
```

The 300 accepted compositions all have replay-verified intermediate and
final artifacts.  The remaining 40 cases are intentional fail-closed
controls.  This benchmark does not grant a stage authority merely because a
previous stage returned plausible text: artifact kind, exact value, and replay
verification are required at each handoff.

## Limits

This is a bounded two-stage composition slice over exact integer artifacts. It
does not yet cover proposition-to-algebra proof obligations, multi-stage
assumption propagation, or arbitrary artifact schemas. Those should be added
only from independently observed failure clusters.
