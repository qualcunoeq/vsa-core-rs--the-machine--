# Phase 21 — QuantityRelationV1 bounded implementation

This phase implements the narrow relation formalizer proposed in Phase 20.
It emits typed `QuantityRelationArtifact` values and local replay receipts; it
does not solve the relations, mutate the capability registry, or register a
word-problem route in the global router.

## Supported grammar

- unit rates;
- direct ratios;
- linear scaling/proportions;
- explicitly stated unit conversions;
- explicit sums and differences;
- simple linear quantity changes.

The implementation rejects or abstains on percentages, compound interest,
nonlinear relations, geometry, probability, missing anchors, incompatible
units, and unstated conversion factors.

## Expanded corpus result

```bash
RUSTFLAGS='-Awarnings' cargo run --release --quiet --bin quantity_relation_bench -- data/quantity_relation_v1_expanded.json
```

```text
quantity-relation: cases=300 structural=300/300 accepted=200 ambiguous=30 unsupported=70 replayed=200 rewrite_pairs=50/50 false_auth=0 false_denials=0 failures={}
```

The benchmark is a contract-validation result, not a general GSM8K result.
The corpus is project-authored and template-generated, so it is not
independent evidence.  The next gate is an independently reviewed corpus
before global-router integration.

## Safety boundary

Accepted artifacts contain typed signatures and explicit linear constraints.
`replay_verified()` checks the artifact structure without executing it.  Any
future algebra handoff must remain a separate typed and authorized stage.
