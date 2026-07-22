# Phase 6 — Independent out-of-distribution validation (pilot)

This evaluation is deliberately separate from the development and generated
corpora.  The 24 base cases in `data/algebra_ood_v1.json` were hand-authored
with different wording and adversarial structures.  Each base case has one
semantic-preserving rewrite, giving 24 paired invariance checks.  No capability
or authorization rule was changed to fit this corpus.

Run:

```bash
cargo run --release --bin ood_bench -- \
  data/algebra_ood_v1.json /tmp/algebra_ood_v1_report.json
```

## Result

| Measure | Result |
|-|-:|
| Total cases (base + rewrites) | 48 |
| Independently authored base cases | 24 |
| Rewrite pairs | 24 |
| Formalization complete | 43/48 (89.6%) |
| Authorization decision correct | 23/48 (47.9%) |
| Final result correct | 8/48 (16.7%) |
| Replay among successful executions | 3/3 (100%) |
| Authorization false positives | 11 |
| Authorization false denials | 14 |
| Rewrite decisions stable | 15/24 (62.5%) |
| Rewrite results stable | 21/24 (87.5%) |
| Rewrite regressions | 11 |

The benchmark therefore **does not meet** the proposed 90%/zero-false-
authorization milestone.  This is a useful result: the existing generated
benchmark is saturated, while independently worded cases expose a real
formalization/execution distribution gap.

## Failure classification

Refusals were assigned a first-blocker taxonomy in the per-case report.  The
dominant observed blockers were:

| Refusal blocker | Count |
|-|-:|
| `target_incomplete` | 17 |
| `representation_incomplete` | 2 |

The positive-case misses are primarily target/representation handling under
unseen phrasing and reordered system equations.  The negative cases also show
11 direct-authorization false positives: a target can pass the surface
assessment even though it is not a supported governed solve.  These are
formalization/authorization findings, not evidence for adding a new algebra
method.

## Interpretation and next action

The pilot validates that the OOD harness is sensitive to benchmark leakage and
rewrite instability.  It also shows that the next engineering investment
should be a failure-driven formalization audit (target extraction, equivalent
wording, and negative authorization), not a new capability or solver.

Before changing behavior, expand this corpus toward the 500-case target with
additional independently authored and sourced cases.  Re-run the same paired
metrics, and only implement a fix after a failure class remains dominant and
the fix is tested on held-out rewrites.

The machine's execution and replay authority remain unchanged; this commit is
evaluation infrastructure and evidence collection only.
