# Phase 77 — source-declared generic input bindings

This phase extends the source-formula catalog with a declarative `BINDINGS`
field.  A source record can now name bounded input-binding primitives without
the frontend branching on a route name.  The binding map is validated against
the record's declared inputs, retained in the source-derived artifact, and
used before ordinary labeled-value extraction.

## Frozen inputs

| Artifact | SHA-256 / value |
|---|---|
| Source plan | `c2e589c326cce225872a44f3e2d358a97950a24d49bed8bbb5632e8bd44c41b7` |
| External portfolio | `3cf924116a0f8f6a0c84d0ce7949b0c1e16221e0d4b5fcb0c4322110e30714f2` |
| Sequence source | `f891f41b69d873ee477a2714a23bdac69d12ccc4aaac17705de76f165830e741` |
| Route | `ArithmeticSequence` |

The source declaration is:

```text
BINDINGS: a1=first_integer_sequence_value; n=requested_ordinal; d=constant_integer_sequence_difference
```

## Results

The generic source-formula frontend was evaluated answer-key blind on the
existing external portfolio.  Candidate values remain hashes; no production
registry or router was changed.

| Partition | Questions | Complete frontends | Executions | Execution replay/tamper | False authorizations |
|---|---:|---:|---:|---:|---:|
| Development | 3,000 | 1 | 1 | 1/1 | 0 |
| Sealed | 1,000 | 1 | 1 | 1/1 | 0 |

All frontend decisions replayed and rejected tampering (`3000/3000` and
`1000/1000`).  No answer keys or plaintext answers were read, and the
curriculum manifest remained unchanged.

The sealed candidate is `math-v1-sealed-0001`; its value is recorded only as
the hash in
`stage340_goal6_generic_sequence_transfer_sealed.json`.  The independently
selected portfolio route remains `FiniteListMean` (two development candidates,
zero sealed candidates), so this sequence result is a route-level transfer
observation, not a promoted capability.

## Safety and regression evidence

* 31 focused source-formula tests passed.
* malformed or undeclared bindings are rejected by source validation.
* sum requests and non-constant sequences remain non-complete.
* no source mutation, answer-key access, production authorization, or live
  manifest mutation occurred.
* existing finite-statistics and unit routes retain their prior bounded
  outcomes; their regenerated reports carry the new source-plan hash.

This is progress toward source-derived acquisition, but it is not yet a
general self-education result: the primitive vocabulary is still bounded and
the route selector did not select this sequence route on development coverage.
