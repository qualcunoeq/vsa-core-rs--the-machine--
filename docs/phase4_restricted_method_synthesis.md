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

## Full frozen-corpus campaign

The same synthesized family specs were then evaluated against the complete
frozen corpora: 300 QuantityRelation cases, 27 UnitQuantity cases, 29
FractionalQuantity cases, and the 350-case PercentageQuantity contract corpus.

| Metric | Result |
| --- | ---: |
| Total cases | 706 |
| Correct decisions | 706 / 706 |
| Authorized cases | 432 |
| Accepted artifacts replay-verified | 432 / 432 |
| Method receipts replay-verified | 706 / 706 |
| False authorizations | 0 |
| False denials | 0 |
| Invalid synthesized specs | 0 |

This is still a shadow reconstruction using trusted historical formalizers;
it is not yet an unseen-contract synthesis result.

Per-family results are recorded by the deterministic campaign test:

| Family | Cases | Correct | Authorized | Accepted replay | Method replay | Steps / depth / budget |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| QuantityRelationV1 | 300 | 300 | 200 | 200 / 200 | 300 / 300 | 8 / 8 / 9 |
| UnitQuantity | 27 | 27 | 15 | 15 / 15 | 27 / 27 | 8 / 8 / 9 |
| FractionalQuantity | 29 | 29 | 17 | 17 / 17 | 29 / 29 | 8 / 8 / 9 |
| PercentageQuantityV1 | 350 | 350 | 200 | 200 / 200 | 350 / 350 | 8 / 8 / 9 |

All four families recorded zero false authorizations and zero false denials.
Downstream bridge correctness is intentionally **not claimed by this Phase 4
shadow interpreter**; the existing family-specific bridge benchmarks remain
the authority for bridge execution. The synthesized method itself only
constructs, verifies, and replays the typed artifact.

## Method-spec defect campaign

Seven implementation-level defect fixtures are rejected statically:

```text
OmitSafetyCheck
RemoveSupportedFormBranch
WrongBindingExtraction
WrongTrustedBridge
OmitReplay
ExceedBudget
ReorderChecksUnsafely
```

The sandbox revision test repairs an omitted replay step without mutating the
parent specification. The current focused module suite reports **6 passed, 0
failed**, including the full-corpus and defect campaigns.

## Deliberate non-goals

This phase does not yet synthesize arbitrary new parsers, invent DSL
operations, apply revisions to production contracts, or promote a method to a
capability. The next gate is to hide the historical implementations, compare
the DSL boundary against larger frozen corpora, and run Phase 3B counterexample
refinement against a deliberately weakened synthesized specification.
