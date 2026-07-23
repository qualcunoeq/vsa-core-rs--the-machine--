# Phase 28 — Quantity Planning v2

This release pressures the quantity-family planner rather than adding a new
mathematical primitive. The deterministic generator (`seed=42`) produces 240
tasks covering:

- direct QuantityRelation → Algebra routes;
- UnitQuantity → Algebra and conversion → LinearSystem routes;
- FractionalQuantity → Algebra routes;
- QuantityRelation ratio → LinearSystem routes;
- bounded multi-step quantity routes;
- cheaper competing routes;
- equal-cost ties with different results;
- unsupported mathematics and incompatible handoffs.

The benchmark is diagnostic-only. It does not alter the global router or
capability registry. Every eligible front-end artifact and downstream result
must pass replay before route ranking.

## Release result

```text
cases=240 authorized=220 correct_decisions=240 false_auth=0 false_denials=0
intermediate_replays=220 final_replays=220 invalid_handoffs_rejected=5
route_failures=0 ambiguous=10 rewrite_decisions=8/8 rewrite_results=8/8
regressions=0 deterministic=true
```

The ten equal-cost competing routes remained ambiguous. The five incompatible
handoffs were rejected, and no unsupported route was promoted by planner cost
or support. Existing v1 quantity, unit, fractional, and multi-step benchmarks
remained unchanged.
