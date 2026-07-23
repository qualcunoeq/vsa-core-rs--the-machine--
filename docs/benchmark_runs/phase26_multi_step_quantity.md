# Phase 26 — Multi-step quantity composition

This phase composes existing bounded quantity primitives rather than adding a
general word-problem solver.  The supported families are:

- fractional remainder/part followed by explicit addition or subtraction;
- explicit unit conversion followed by addition;
- a two-stage numeric add/subtract chain.

Every stage is formalized and replayed before its result is handed to the next
stage.  Three-stage chains, percentages, nonlinear transformations, and
ambiguous intermediate operations remain outside scope.

## Evidence

```text
RUSTFLAGS='-Awarnings' cargo run --release --quiet --bin multi_step_quantity_bench -- data/multi_step_quantity_v1.json
```

```text
cases=24
structural=24/24
accepted=18
replayed_plans=18
replayed_stages=36
ambiguous=3
unsupported=3
results=18/18
rewrite_pairs=3/3
false_auth=0
false_denials=0
failures={}
```

This is a diagnostic composition layer.  It does not alter global routing or
authorize new capabilities; it validates that existing typed quantity
primitives can be staged without laundering an unverified intermediate result.
