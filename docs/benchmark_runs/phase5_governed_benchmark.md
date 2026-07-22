# Phase 5 Governed Benchmark

Run date: 2026-07-22  
Command: `cargo run --release --bin governed_bench -- 1000 1000 42 ...`  
Seed: `42`

The benchmark was expanded so generated algebra cases receive their own tier,
and the requested case count is also used for proposition and recurrence
generators. This prevents a large requested run from silently reporting only
the small hand-authored algebra slice.

## Results

| Tier | Cases | Expected positives | Positive success | Replay | False authorization | False denial |
|---|---:|---:|---:|---:|---:|---:|
| Hand-authored algebra | 27 | 27 | 100% | 100% | 0 | 0 |
| Generated algebra | 1,000 | 1,000 | 100% | 100% | 0 | 0 |
| Algebra prose | 20 | 10 | 100% | 100% | 0 | 0 |
| Proposition kernel | 1,000 | 645 | 100% | 100% | 0 | 0 |
| Strategic method selection | 1,000 | 1,000 | 100% | 100% | 0 | 0 |
| Recurrence | 1,000 | 502 | 100% | 100% | 0 | 0 |
| Adversarial algebra | 21 | 0 | — | — | 0 | 0 |

The benchmark runtime was approximately 24.4 seconds in release mode.

## Failure classification

The reported failure counts are expected negative cases, not missing positive
capabilities:

- Recurrence refusals cover missing/conflicting initial conditions, unroll
  limits, out-of-domain targets, targets before the base case, and arithmetic
  overflow. All 502 expected-authorized cases succeeded; false denials were 0.
- Proposition failures cover unknown theorems, uninstantiated binders,
  certificate rejection, premise-count mismatch, and expected-conclusion
  mismatch. All 645 expected-accept cases succeeded; false rejections were 0.
- Strategic-route diagnostics report contextual retrieval, safety rejection,
  and unsupported-domain scenarios, but all 1,000 expected route decisions were
  correct. `method_not_found` and `planning_failure` were both 0.
- The single prose formalization failure was a negative/refusal case; all 10
  expected-positive prose cases succeeded.

The verification ablation independently rejected 32/32 tampered receipts,
while the no-verification control falsely accepted 32/32. Contextual support
also remained safety-preserving: global-only evidence produced 667 misleading
decisions, while contextual evidence produced 778 correct decisions.

## Decision

No capability implementation was justified by this run. The highest-frequency
observed labels are intentional refusals or diagnostic scenario classes, not
false denials. The next benchmark should therefore target a new domain or
expand positive coverage rather than add method-acquisition machinery.
