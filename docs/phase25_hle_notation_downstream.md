# Phase 25 — shadow downstream evaluation of HLE notation artifacts

Phase 24 produced 16 accepted equations/expressions artifacts from the frozen
Phase 23 audit. This phase sends those artifacts through the existing router in
shadow mode. It does not alter production routing, authorization, registries,
the HLE release, or answer scores.

## Protocol

For each accepted artifact:

```text
HLE question
→ Phase 24 notation normalization
→ replay-verified typed notation artifact
→ existing downstream router (normalized math region)
→ terminal classification
→ candidate answer and downstream replay result
```

The original HLE question is also routed as a baseline for comparison. An
answer is counted as authorized only when the normalized downstream route
itself returns an answer that exactly matches the frozen HLE answer. A
replayable notation artifact without an executable downstream target remains a
safe coverage gap.

## Frozen inputs

* Phase 23 audit SHA-256: `5b9f7f2a49b9e69b8a0d4e1768615a14a34a03446aa68d09741605cf6edc6af6`
* Phase 23 source trace SHA-256: `3b0cc1aac3819b8f41343f21cd02ff8c54b0e5b3be1f99ce9164b5c6a2cb2348`
* HLE dataset SHA-256: `31b26cc8e352af16bedb9a714feb788ae562be38898ab92dc54b4665882bf1c6`
* Report SHA-256: `9bdea2a6f272494c6e447469873077b0c46740c9bc3ac7bb9d868cf1443f5dc8`

## Result

| Outcome | Cases |
|---|---:|
| Candidate audit rows | 58 |
| Accepted notation artifacts | 16 |
| Accepted artifacts replayed | 16 |
| Correct authorized answers | 0 |
| Incorrect authorized answers | 0 |
| Safely formalized but downstream unsupported | 16 |
| Downstream replay-verified answers | 0 |
| False authorizations | 0 |
| Interpretation changes between original and normalized route | 3 |

Every accepted artifact was locally replay-verified. None carried enough
target/context for the existing downstream route to produce an answer, so the
experiment produced no HLE score gain and no false authorization. The three
route changes are retained in the per-record report; they are diagnostic and
not promoted behavior.

The normalized regions also expose an important follow-up issue: the current
contract selects the first accepted math region, which can be an incidental
symbol or condition rather than the question's target formula (for example,
an angle marker or a single letter). This is a concrete Phase 25 failure mode,
not evidence that the HLE questions are solved. The next normalization change
should require target-linked region selection or preserve multiple candidates
until the question context disambiguates them.

## Reproduction

```text
cargo test --bin hle_notation_recovery
cargo test --bin hle_notation_downstream
cargo run --bin hle_notation_downstream -- \
  /tmp/hle_notation_downstream_2147e9e.json \
  /tmp/hle_notation_audit_2147e9e.json
```

The full library suite retains the pre-existing failures documented in the
Phase 24 record; no new Phase 25 production-state mutation was observed.

## Next gate

Do not widen the parser based on these 16 rows. First cluster the selected
region failures and build a fresh equations/expressions corpus that tests
target-linked selection, multiple formulas, answer choices, and incidental
notation. Only a normalized artifact that survives that gate should be sent
to a downstream executor.
