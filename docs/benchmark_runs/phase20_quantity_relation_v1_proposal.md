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
pilot.  The expanded release is generated from the checked-in, reviewed
template families by `quantity_relation_corpus_expand` and is stored at
`data/quantity_relation_v1_expanded.json`.  It deliberately does not contain
executor results because no QuantityRelation executor exists yet.

Every positive case must satisfy:

1. all quantities and units are explicit;
2. the relation can be represented as a finite linear constraint set;
3. the target is uniquely named;
4. the expected typed signature is independent of surface wording.

Every negative case must remain non-authorizing.  Rewrite pairs must preserve
the expected typed signature, while minimally changed negative pairs must not
be promoted into supported relations.

Reproduce the expanded release and validate its invariants with:

```bash
RUSTFLAGS='-Awarnings' cargo run --release --quiet --bin quantity_relation_corpus_expand -- --emit data/quantity_relation_v1_expanded.json
RUSTFLAGS='-Awarnings' cargo run --release --quiet --bin quantity_relation_corpus_check -- data/quantity_relation_v1_expanded.json
```

The expanded release contains 300 cases: 200 supported relation cases, 100
negative/ambiguous cases, and 50 rewrite families (the rewrite variants are
included in the 200 supported cases).  Its generated-release hash is
`0612b09834b41be2e7ec900b49330dce7f74609b73fe0405f5e5f9f8d0c89fcc`.

```text
quantity-relation-corpus: cases=300 supported=200 ambiguous=30 unsupported=70 rewrite_pairs=50 deterministic=true
```

This remains a contract and failure-taxonomy gate, not a capability result.
The cases are project-authored and template-generated pending independent
review; they must not be described as third-party evidence.

## Candidate promotion gate

Implementation is justified only if the reviewed corpus has:

- a stable typed oracle;
- zero unresolved authorization ambiguities in the positive set;
- explicit rejection reasons for every negative;
- replayable relation signatures;
- no requirement to infer missing units or constants.

Only after that gate should a translator be implemented and connected to the
existing algebra planner.
