# Phase 27 — Quantity Cross-Domain Planner Pressure

This diagnostic release tests planner-level selection among typed quantity,
unit, fractional, algebra, and linear-system routes. It does not modify the
global router or capability registry.

The planner executes only routes whose source artifact is accepted and replay
verified, then ranks valid routes by declared cost and contextual support.
Invalid handoffs and unsupported front-ends fail closed before ranking.

Corpus: `data/quantity_cross_domain_v1.json` (25 cases).

Coverage includes:

- QuantityRelation → Algebra;
- QuantityRelation ratio → LinearSystem;
- UnitQuantity → Algebra;
- UnitQuantity conversion → LinearSystem;
- FractionalQuantity → Algebra;
- invalid handoffs, ambiguous inputs, and unsupported mathematics;
- rewrite pairs across quantity, unit, and fractional routes.

The benchmark is intended to establish that new quantity-family capabilities
can participate in planner comparison without bypassing typed handoffs or
replay verification. Results should be recorded from the release-mode binary
`quantity_cross_domain_bench`.

## Result

Release-mode output:

```text
cases=25 authorized=16 correct_decisions=25 false_auth=0 false_denials=0
intermediate_replays=16 final_replays=16 invalid_handoffs_rejected=4
route_failures=0 ambiguous=2 rewrite_decisions=2/2 rewrite_results=2/2
regressions=0 deterministic=true
```

The planner selected 16 valid cross-domain routes, preserved two ambiguous
cases, and rejected the remaining unsupported or invalid-handoff cases. All
accepted routes passed both front-end and downstream replay checks.
