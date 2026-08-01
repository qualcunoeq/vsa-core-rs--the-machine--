# Phase 27 — downstream method-family audit

Phase 26 produced 13 uniquely grounded HLE targets, but grounding alone did
not make them executable. Phase 27 adds a deterministic, shadow-only audit of
the downstream method requirements for those targets. It does not change the
router, capability registry, ontology, authorization policy, or HLE score.

```text
grounded math target
→ requested output artifact
→ operation / transformation
→ prerequisite knowledge
→ reusable method-family hypothesis
→ nearest existing capability
→ typed-interface mismatch
```

## Audit record

`hle_method_audit` emits one `MethodRequirement` for every accepted record in a
Phase 26 grounding report. Each record retains the source question ID and
explicitly records:

* input and requested output artifact types;
* the proposed operation;
* prerequisite theorem, law, definition, or convention cues;
* whether the method looks reusable;
* the nearest existing capability;
* whether a typed bridge is missing;
* unsupported operators or representations;
* lexical evidence used for the diagnostic family label.

The family label is a clustering hypothesis, never an authorization decision.
Unclassified cases remain unclassified rather than being forced into a nearby
method. Family summaries expose the largest observed family, shared operation,
artifact types, prerequisite cues, bridge gaps, and a contract status. This is
the input to a later independent contract proposal, not a capability
promotion.

Current diagnostic families are deliberately bounded:

* geometry inequalities;
* abstract algebra;
* fractal dimension;
* graph theory;
* differential equations;
* category theory;
* algorithmic complexity;
* number theory;
* topology;
* linear algebra;
* probability and statistics;
* applied PDE;
* combinatorics;
* optimization.

## Provenance

The upstream Phase 26 run reported 13 accepted groundings, 23 ambiguities,
and 22 unsupported rows. The immutable Phase 26 report hash is
`a8ce98fbf44f0d2a382468d61df257e9feaad07f2381e9297d74fc687943cc38`; the
dataset hash is `31b26cc8e352af16bedb9a714feb788ae562be38898ab92dc54b4665882bf1c`.
The generated JSON report is intentionally kept outside the repository, like
the Phase 26 HLE artifacts. This prevents benchmark material from becoming a
silent checked-in source of truth.

## Reproduction

Provide the Phase 26 grounding report as the first argument and an output path
as the second:

```text
cargo test --bin hle_method_audit
cargo run --bin hle_method_audit -- \
  /tmp/hle_notation_grounding_2147e9e.json \
  /tmp/hle_method_audit_2147e9e.json
```

The report records hashes of both the grounding artifact and
`data/hle.jsonl`, so a later family cluster can be reproduced against the
exact same 13 inputs. A missing upstream artifact is a reproducibility error,
not permission to silently substitute a different HLE slice.

## Safety and next gate

The audit is diagnostic only. It cannot invoke an executor, publish a fact,
modify a registry, or authorize an answer. The 23 ambiguous groundings remain
outside the method audit and are preserved as a frozen boundary set.

Before proposing the largest family as a capability, collect an independent
positive/ambiguous/unsupported corpus for that family. Require repeated shared
method evidence, typed bridge validation, and an untouched HLE-family holdout.
Only then may the restricted synthesis and promotion pipeline be used.

Focused validation for this phase: `cargo test --bin hle_method_audit` (three
deterministic tests). Existing unrelated library warnings and known failures
are not changed by this shadow-only tool.
