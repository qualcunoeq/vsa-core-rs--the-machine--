# Phase 20 — QuantityRelationV1 proposal and pre-implementation corpus

This phase turns the largest GSM8K rejection families into a bounded capability
proposal.  It does **not** add an executor, broaden authorization, or claim
word-problem competence.  The artifact under evaluation is a typed relation
extraction, not a solved answer.

## Proposed contract

```text
QuantityRelationV1

input:
  explicitly named entities, numeric quantities, and declared units

output:
  TypedQuantityRelation {
    variables: [(name, unit)],
    constraints: linear equalities/ratios,
    target: named quantity or relation,
    assumptions: explicit unit and domain requirements
  }
```

Supported in the first version:

- single-step unit rates (`cost per item`, `distance per hour`);
- direct ratios and proportions with an explicit anchor quantity;
- linear unit conversions when the conversion factor is stated;
- finite sums of explicitly stated quantities;
- target extraction for `total`, `difference`, or one named quantity.

Explicitly out of scope:

- percentages, discounts, tax, interest, and compound growth;
- implicit unit conversions or unstated real-world constants;
- probability, geometry, nonlinear relations, and optimization;
- multi-stage temporal narratives;
- missing, contradictory, or ambiguous anchors.

The proposed capability only emits typed constraints.  The existing algebra
vertical remains responsible for solving authorized linear constraints.

## Evaluation design

`data/quantity_relation_v1_pilot.json` is a frozen, hand-authored contract
corpus for the pre-implementation review.  It contains positive relation
cases, negative/ambiguous cases, and semantic rewrite links.  It deliberately
does not contain executor results because no QuantityRelation executor exists
yet.

Every positive case must satisfy:

1. all quantities and units are explicit;
2. the relation can be represented as a finite linear constraint set;
3. the target is uniquely named;
4. the expected typed signature is independent of surface wording.

Every negative case must remain non-authorizing.  Rewrite pairs must preserve
the expected typed signature, while minimally changed negative pairs must not
be promoted into supported relations.

The pilot is a contract and failure-taxonomy gate, not a capability result.
The full follow-up target is 200 positives, 100 negatives/ambiguities, and 50
rewrite pairs after the schema and oracle are independently reviewed.

## Candidate promotion gate

Implementation is justified only if the reviewed corpus has:

- a stable typed oracle;
- zero unresolved authorization ambiguities in the positive set;
- explicit rejection reasons for every negative;
- replayable relation signatures;
- no requirement to infer missing units or constants.

Only after that gate should a translator be implemented and connected to the
existing algebra planner.
