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

## First unseen-contract experiment

`ClockTimeDifferenceV1` is supplied to the generic synthesizer only as typed
contract data: input/output artifacts, required bindings, predicates, the
trusted graph capability, and budgets. No clock-specific branch is present in
`synthesize_from_contract`.

Frozen contract hash:
`e6ada0b6f8ac761d7d6d64dcaa2de1c711b27ed8a2c2b9b18eef86d44d4e46f2`

Synthesizer configuration:

```text
version: phase4-generic-contract-v1
operation_budget: 16
depth_budget: 8
development_hash: 5b64f347fa80347de52517a5fa6e808d96e383f53b6e605f22e322acc9b38829
holdout_hash: 506fdc57fed2454e74e68d9d84cd06f60cd521f376671a1f3eaf005e0462fd35
```

The synthesized DSL operation trace is:

```text
ExtractBinding(start_time)
RequireBinding(start_time)
ExtractBinding(end_time)
RequireBinding(end_time)
NormalizeNumeric
MatchSupportedForm
CheckPredicate(explicit_notation)
CheckPredicate(bounded_rollover)
CheckPredicate(no_calendar_or_external_time_context)
RejectAmbiguous
RejectUnsupported
InvokeCapability(clock_time_difference)
VerifyArtifact
Replay
```

The initial development/holdout corpus contains 18 cases (10/8 split),
covering same-day 12-hour and 24-hour notation, one explicit overnight
rollover, missing meridiem, unclear rollover, dates, time zones, DST, and
recurring schedules.

| Metric | Development | Untouched holdout |
| --- | ---: | ---: |
| Correct decisions | 10 / 10 | 8 / 8 |
| Authorized cases | 6 | 4 |
| Accepted artifacts replay-verified | 6 / 6 | 4 / 4 |
| False authorizations | 0 | 0 |
| False denials | 0 | 0 |

The result is shadow-only and does not mutate the registry or production
router. It is a first bounded unseen-contract result, not yet a broad claim
about natural-language time reasoning.

The generic synthesizer derives this trace from the typed contract fields; it
contains no branch on `ClockTimeDifferenceV1` or clock terminology. The
clock-time parser and its replay verifier are trusted substrate supplied to
the DSL's allowlisted `clock_time_difference` invocation. They are not
claimed as synthesized code. This separation is intentional: Phase 4
synthesizes the method wiring and governance checks, while the shadow
substrate remains independently reviewed and non-authorizing.

## Deliberate non-goals

This phase does not yet synthesize arbitrary new parsers, invent DSL
operations, apply revisions to production contracts, or promote a method to a
capability. The next gate is to hide the historical implementations, compare
the DSL boundary against larger frozen corpora, and run Phase 3B counterexample
refinement against a deliberately weakened synthesized specification.
