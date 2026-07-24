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
```

Every supported case carries the typed relation schema and explicit input and
output contracts. Ambiguous and unsupported cases carry no executable relation
schema. Rewrite pairs must share the same canonical relation schema.

The next phase, if approved, is an independent implementation corpus and
formalizer—not production integration. It must preserve the distinction among
“of,” “more than,” “less than,” “to,” and percentage points before any bridge to
the existing quantity/algebra graph is considered.
