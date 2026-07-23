# Phase 9/10: Blind Mixed-Vertical Integration

Date: 2026-07-23

This benchmark evaluates the router and orchestrator on a blind 1,000-case
corpus combining direct algebra, linear systems, proposition prompts, and a
bounded prose-recurrence vertical. The expected route and
authorization labels are generated independently in
`scripts/generate_mixed_ood.py` and stored in `data/mixed_ood_v1.json`.

## Initial failure and hardening

The first pre-recurrence run exposed 100 false authorizations on degenerate linear systems:
the generic algebra fallback solved one equation from a multi-equation prompt.
The router was hardened to classify the complete system first, authorize only
unique systems, disable the generic fallback for system-like prompts, and
recognize the explicit `solve ` grammar used by the corpus.

## Final result

```text
cases=1000
route_correct=1000 (1.000)
formalized=1000 (1.000)
correct_decisions=1000 (1.000)
authorized=720
replay_successes=720
false_authorizations=0
false_denials=0
rewrite_pairs=238
route_stable=238
decision_stable=238
answer_stable=238
rewrite_regressions=0
route_confusion={}
failure_taxonomy={}
```

The recurrence slice now contains 150 supported affine evaluations and 80
intentional refusals (missing target, unsupported definition, or malformed
input). The executor is deliberately limited to exact first-order affine
unrolling; it does not authorize closed-form discovery, nonlinear recurrences,
or higher-order recurrences.

The integrated result is now:

```text
authorized=720
replay=720
rewrite_pairs=238
rewrite_regressions=0
false_authorizations=0
false_denials=0
```

The result is evidence for this tested distribution only. It is not a claim
of universal routing or mathematical correctness. The benchmark command is:

```bash
cargo run --release --bin mixed_ood_bench -- \
  data/mixed_ood_v1.json /tmp/mixed_ood_v1_report.json
```
