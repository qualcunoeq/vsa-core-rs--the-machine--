# Phase 29 — GSM8K Quantity Planner Reclassification

This release re-evaluates the frozen `third_party_gsm8k_restricted_release_v2`
using planner-selected quantity-family routes. The production router and the
earlier v2 release remain unchanged.

For every source-preserved prompt, the diagnostic planner considers:

- GSM8K quantity relations;
- UnitQuantity;
- general QuantityRelation;
- FractionalQuantity;
- bounded multi-step quantity relations.

Only accepted, replay-verified routes can be selected. Unsupported and
ambiguous source cases remain outside the candidate capability boundary.

## Result

Release-mode output:

```text
cases=100 structural=100/100 existing=4 promoted_expected=12
promoted_realized=12 planner_accepted=12 planner_replayed=12
gsm_quantity=11 multi_step=4 unit_aware=1 fractional=0
ambiguous=12 unsupported=76 results=12/12
false_auth=0 false_denials=0 candidate_leakage=0 failures={}
deterministic=true
```

Four previously unsupported GSM8K cases migrated into the bounded
`multi_step_quantity` family. The existing eight quantity/unit candidate routes
also remained exact, giving 12/12 promoted results with planner replay. The
planner selected the unit-aware route for the wire conversion despite a
competing GSM quantity edge, demonstrating contextual route preference.

This is a diagnostic candidate release only; no global route or capability
registry mutation occurred.
