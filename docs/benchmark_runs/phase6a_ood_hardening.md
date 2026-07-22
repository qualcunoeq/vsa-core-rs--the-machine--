# Phase 6A — OOD formalization hardening

This pass treated the Phase 6 pilot as a frozen regression corpus rather than
adding another algebra capability.  The changes were deliberately limited to
formalization and audit boundaries:

- per-case divergence stage, canonical signature, and authorization blockers;
- conservative equation-side normalization (`equals` and common clause
  markers);
- explicit multiple-variable, unsupported-function, non-unique-equation, and
  untyped multi-equation ambiguity guards;
- typed equation-system recognition for prose with two equations;
- canonical rewrite comparison over normalized equations, variables, and
  domain constraints.

## Results

| Measure | Phase 6 pilot | Phase 6A |
|-|-:|-:|
| Cases | 48 | 48 |
| Authorization false positives | 11 | **0** |
| Authorization false denials | 14 | **0** |
| Rewrite decision/result regressions | 11 | **0** |
| Canonical signatures stable | not measured | 7/24 |
| Rewrite decisions stable | not measured | 24/24 |
| Rewrite results stable | not measured | 24/24 |
| Replay among successful executions | 3/3 | 24/24 |

The safety and rewrite exit criteria are met: no OOD false authorizations or
false denials remain, and every rewrite pair preserves authorization and result
behavior.  Seven pairs also share the stricter canonical signature; the other
pairs are semantically stable despite harmless representational differences in
the diagnostic signature.

The per-case report now records the first divergence as one of:

```text
formalization → authorization → execution → verification → none
```

and records the first authorization blocker for every refusal.  This makes
the remaining failures actionable without treating a heuristic parse as
permission to execute.

The final Phase 6A run classified the 48 cases as follows:

```text
authorization: 15
execution:      8
formalization:  1
none:          24
```

The eight execution-stage observations are the four positive linear-system
cases and their rewrites: authorization is correct, but the existing system
executor does not yet solve those OOD prompt forms.  They are therefore an
execution-coverage follow-up, not a formalization or rewrite-invariance
failure.

## In-distribution regression check

The corrected 1,000-case governed suite was rerun with seed 42:

- generated algebra: 1,000/1,000 positive execution and replay;
- recurrence: 502/502 positive execution and replay;
- multi-step propositions: 645/645 positive execution and replay;
- method selection: 1,000/1,000;
- all tiers: zero false authorizations and zero false denials.

No new capability was added.  The next step is expansion of the independent
corpus toward the 500-case target and external/adversarial coverage beyond this
frozen 48-case distribution.
