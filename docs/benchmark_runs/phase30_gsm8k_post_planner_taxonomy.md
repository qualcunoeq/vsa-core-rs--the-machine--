# Phase 30 — Post-QuantityPlanner GSM8K Taxonomy Audit

This release audits the frozen 100-case GSM8K slice after the QuantityPlanner
reclassification. It does not change routing or authorize any new capability.
The planner candidate set and the v3 promoted release are hashed through the
existing frozen base release.

## Result

```text
cases=100
planner_ambiguous=12
ambiguous_expected=0
ambiguous_from_unsupported=12
planner_no_route=76
residual_unsupported=70
oracle_ambiguous_no_route=2
preexisting_supported_no_route=4
promoted_realized=12
false_auth=0
false_denials=0
deterministic=true
```

The 76 no-route cases are therefore not one homogeneous failure class:

- 70 are genuinely residual unsupported cases;
- 2 are oracle-labelled ambiguities conservatively abstained by the planner;
- 4 are already supported by the existing vertical, but have no quantity route.

The 12 planner-level ambiguities all came from unsupported prompts with
fractional scope/anchor uncertainty. They were preserved as diagnostics rather
than promoted.

## Residual unsupported clusters (70 cases)

```text
percentage_discount_finance       19
multi_step_quantity_arithmetic    16
ratio_rate_proportion              16
unit_measurement_conversion       10
temporal_or_sequential_reasoning    6
fractional_quantity                 3
```

The largest remaining coherent family is now percentage/discount/finance,
followed by multi-step quantity arithmetic and ratio/rate reasoning. This is a
diagnostic roadmap only; no percentage capability was added in this phase.
