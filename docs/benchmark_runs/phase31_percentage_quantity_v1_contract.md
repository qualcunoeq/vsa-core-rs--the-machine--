# Phase 31 — PercentageQuantityV1 Contract Corpus

This is a pre-implementation contract release selected from the post-planner
GSM8K residual taxonomy. It defines the narrow shared semantic primitive
`PercentageQuantityV1`; it does not add an executor, alter authorization, or
modify global routing.

## Scope

Supported contracts:

- percentage of an explicit numeric whole;
- one-step discount with explicit base and rate;
- one-step increase/markup with explicit base and rate.

Explicitly unsupported in V1:

- compound or repeated growth;
- interest and finance-specific calculations;
- percentage points;
- overlapping adjustments;
- probability and symbolic unknowns.

## Corpus result

```text
cases=350
supported=200
ambiguous=50
unsupported=100
rewrite_pairs=50
validation_errors=0
deterministic=true
release_hash=f6ebdcf826e10125050f5b83b50a31a38fcb4b7f8c197dc6d0833a91bf7d336c
```

Every supported case carries the typed relation schema and explicit input and
output contracts. Ambiguous and unsupported cases carry no executable relation
schema. Rewrite pairs must share the same canonical relation schema.

## Implementation Release (e3a5778)

The formalizer, typed artifact, algebra bridge, replay verification, and
adversarial ablation test suite have been implemented.  All 200 supported
cases are accepted with correct operation kind and replay verification.
All 50 ambiguous and 100 unsupported cases are correctly rejected.

The formalizer supports all four contract forms:
- **PercentageOf** — "What is X% of Y?" / "Find X% of Y." / "Calculate X percent
  of the whole quantity Y."
- **IncreaseByPercentage** — "A quantity with base value B increases by R%."
- **DecreaseByPercentage** — "An item priced at $B receives an R% discount." /
  "Apply an R percent reduction to a base price of B dollars."
- **RecoverBase** — "After an R% increase, the new value is F." / "After an R%
  reduction, the discounted price is F." / "The final price is F after an R%
  discount."

Test results: 34/34 pass (29 individual form tests + 5 bridge/replay tests),
plus 3 cross-domain planner tests, 2 proposal corpus tests.
