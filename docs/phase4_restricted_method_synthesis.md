# Phase 4 — Restricted Method-Implementation Synthesis

Phase 4 turns a validated capability contract into an immutable,
declarative `MethodImplementationSpec`. It does not emit Rust, execute
arbitrary code, mutate registries, publish facts, or grant capability
authority.

## Restricted DSL

The method specification may contain only these operations:

```text
ExtractBinding
RequireBinding
NormalizeNumeric
MatchSupportedForm
CheckPredicate
ConstructTypedRelation
InvokeCapability
VerifyArtifact
RejectAmbiguous
RejectUnsupported
Replay
```

Each step has an input and output artifact type. Validation rejects broken
handoffs, untrusted capability names, missing verification or replay, budget
violations, and non-diagnostic specifications. The current budgets are at
most 16 operations and depth 8; historical synthesis uses a smaller explicit
budget.

## Shadow interpretation

`shadow_execute` invokes only the four existing, trusted formalizers:

* `QuantityRelationV1`;
* `UnitQuantity`;
* `FractionalQuantity`;
* `PercentageQuantityV1`.

An accepted artifact must pass its native replay gate before the method receipt
is marked replay-verified. Ambiguous and unsupported cases remain non-executed
diagnostic outcomes.

## Historical reconstruction campaign

The initial campaign contains eight independently labelled cases across all
four historical families (two per family):

| Metric | Result |
| --- | ---: |
| Cases | 8 |
| Correct decisions | 8 / 8 |
| Authorized cases | 4 |
| Accepted cases replay-verified | 4 / 4 |
| False authorizations | 0 |
| False denials | 0 |
| Invalid synthesized specs | 0 |

The campaign is a shadow reconstruction: it reproduces observable contract
behaviour using trusted primitives, rather than copying the original Rust
implementation structure. Malformed and unauthorized specifications are
rejected before interpretation, and no live registry or executor state is
changed.

## Deliberate non-goals

This phase does not yet synthesize arbitrary new parsers, invent DSL
operations, apply revisions to production contracts, or promote a method to a
capability. The next gate is to hide the historical implementations, compare
the DSL boundary against larger frozen corpora, and run Phase 3B counterexample
refinement against a deliberately weakened synthesized specification.
