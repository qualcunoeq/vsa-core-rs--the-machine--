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

Typed integration checks also pass:

```text
quantity-relation: ... algebra_bridge=200/200 ratio_system_bridge=40/40 ...
quantity-mixed: quantity_routes=200 ambiguous=30 unsupported=70 route_errors=0 leakage=0 legacy=3/3 deterministic=true
```

The mixed route tries QuantityRelation only for a unique typed artifact and
falls back to the existing raw decomposition path otherwise.  Existing
GSM8K release prompts remain on their previous raw-prose path; the current
QuantityRelation grammar intentionally does not claim to understand their
multi-step narratives.  Expanding that front end requires a separate
source-preserved GSM8K candidate corpus and oracle review.

The benchmark is a contract-validation result, not a general GSM8K result.
The corpus is project-authored and template-generated, so it is not
independent evidence.  The next gate is an independently reviewed corpus
before global-router integration.

## Safety boundary

Accepted artifacts contain typed signatures and explicit linear constraints.
`replay_verified()` checks the artifact structure without executing it.  Any
future algebra handoff must remain a separate typed and authorized stage.  In
this phase, every accepted artifact has an algebra replay receipt, and the
anchored ratio family additionally has a replayed linear-system handoff.
