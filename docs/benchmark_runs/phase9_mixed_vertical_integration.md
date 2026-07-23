# Phase 9: Blind Mixed-Vertical Integration

Date: 2026-07-23

This benchmark evaluates the router and orchestrator on a blind 1,000-case
corpus combining direct algebra, linear systems, proposition prompts, and a
deliberate unsupported recurrence boundary. The expected route and
authorization labels are generated independently in
`scripts/generate_mixed_ood.py` and stored in `data/mixed_ood_v1.json`.

## Initial failure and hardening

The first run exposed 100 false authorizations on degenerate linear systems:
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
authorized=570
replay_successes=570
false_authorizations=0
false_denials=0
rewrite_pairs=189
route_stable=189
decision_stable=189
answer_stable=189
rewrite_regressions=0
route_confusion={}
failure_taxonomy={}
```

The 230 recurrence prompts are intentionally safe abstentions: the router
recognizes their mathematical surface, but no prose recurrence executor is
registered yet. This benchmark therefore measures integration safety, not
recurrence capability.

The result is evidence for this tested distribution only. It is not a claim
of universal routing or mathematical correctness. The benchmark command is:

```bash
cargo run --release --bin mixed_ood_bench -- \
  data/mixed_ood_v1.json /tmp/mixed_ood_v1_report.json
```
