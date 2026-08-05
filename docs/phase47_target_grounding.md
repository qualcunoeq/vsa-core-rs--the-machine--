# Phase 47 — shadow target-grounding contracts

Phase 47 validates two separate target contracts without changing production
routing or authorization:

* `PropertyTargetGroundingV1` for classification groups, scalar extrema, and
  predicate truth;
* `SymbolicTargetExpressionV1` for Greek/non-ASCII symbols and compound
  expressions.

## Independent validation

The cross-domain corpus contains 100 cases with supported, ambiguous, and
unsupported boundaries:

* 100/100 exact decisions;
* 100/100 replay verified;
* 0 incorrect target bindings;
* 20 rewrite groups;
* 0 downstream authorizations.

The symbolic contract preserves `α + β` as one requested expression while also
retaining its components. The property contract distinguishes classification
groups from scalar bounds and records optimization direction.

Benchmark artifact: [`phase47_target_grounding_bench.json`](phase47_target_grounding_bench.json)
(SHA-256 `dddf7366f7db7b32e349aeb15381fc756d32b1211323ea59c79e84b498843c04`).

## Frozen HLE rerun

All four Phase 46 residuals now receive complete target artifacts:

* 4/4 target decisions complete;
* 4/4 target replays verified;
* 4/4 existing equation bindings remain context-ambiguous;
* 0 complete target-plus-binding routes;
* 0 candidate answers;
* 0 downstream authorizations.

The target layer is therefore validated, but it exposes a separate remaining
handoff gap: the existing equation binder still needs cross-region context to
complete the full problem representation. No solver or target guess was added.

HLE artifact: [`phase47_hle_target_grounding_rerun.json`](phase47_hle_target_grounding_rerun.json)
(SHA-256 `cc671e4e098970dc8b5836c1ca352f8f03ecf3460cecf8abf80e82092235c601`).
