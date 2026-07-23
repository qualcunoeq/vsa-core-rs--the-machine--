# Phase 13 — Planner-generated composition

This phase moves from predefined composition routes to route selection. The
500-task corpus provides candidate one-, two-, and three-stage edges but does
not expose the expected route to the evaluator. The bounded planner selects a
candidate only after validating every typed handoff and replaying every stage.

```bash
cargo run --release --quiet --bin compositional_planner_bench \
  data/compositional_planner_ood_v1.json
```

Observed result:

```text
cases=500
authorized=465
correct_decisions=500
false_auth=0
false_denials=0
replayed_stages=840
ambiguous=10
invalid_handoffs=25
route_failures=0
```

The 25 intentional negative tasks abstain because no candidate is executable.
The accepted set includes direct routes, two-stage routes, and three-stage
recurrence → algebra → recurrence routes. Unsupported candidates and invalid
handoffs cannot become fallback paths.

The unified integration command now accepts a fourth corpus argument and runs
the 1,000 mixed, 340 typed-composition, 500 planner-selection, and 500
raw-decomposition cases in one release-mode process.
