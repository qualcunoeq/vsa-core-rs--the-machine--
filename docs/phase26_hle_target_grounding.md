# Phase 26 — target-linked mathematical-region grounding

Phase 25 showed that replayable notation was not necessarily the operative
formula for the question. Phase 26 adds a shadow grounding layer that keeps
all mathematical spans, assigns bounded provisional roles, and selects a
target only when the request identifies a unique region.

```text
question
→ all math regions
→ role candidates
→ target-linked selection or ambiguity
→ Phase 24 normalization
→ downstream probe (shadow-only)
```

No production parser, router, registry, authorization policy, or HLE score was
changed.

## Independent multi-region corpus

The corpus contains 50 cases with definitions, assumptions, quoted formulas,
answer-format text, irrelevant spans, and multiple plausible targets.

| Metric | Result |
|---|---:|
| Cases | 50/50 |
| False accepts | 0 |
| False rejections | 0 |
| Target selections correct | 30/30 |
| Accepted artifacts replayed | 30/30 |
| Rewrite regressions | 0/0 |

Corpus SHA-256: `4df6e93e5e79acbd1be4a789f6501d303f814f135264a28a5822df6ca37b48d3`

## Rerun of Phase 23 candidates

The Phase 23 equations/expressions filter still contains 58 rows. Grounding
classifies all of them rather than silently dropping non-selected rows:

| Grounding outcome | Cases |
|---|---:|
| Candidate rows | 58 |
| Unique accepted target groundings | 13 |
| Ambiguous target | 23 |
| Unsupported/no usable target | 22 |
| Accepted groundings replayed | 13 |

Audit artifact SHA-256: `d6e8e69133f4d5eebb5b3208bbdfdb409ddf76ffb04fef4ea5f5e963316b3ab7`

Source trace SHA-256:
`3b0cc1aac3819b8f41343f21cd02ff8c54b0e5b3be1f99ce9164b5c6a2cb2348`

The 13 accepted groundings are diagnostic only. The existing downstream stack
still has no target-specific executable route for these HLE questions, so this
phase reports no newly correct answers and does not change the HLE score. The
23 ambiguous rows are preserved rather than forced into a first-span choice.

The generated report is hashed as:

`a8ce98fbf44f0d2a382468d61df257e9feaad07f2381e9297d74fc687943cc38`

## Reproduction

```text
cargo test --lib notation_grounding
cargo test --bin hle_notation_grounding
cargo run --bin hle_notation_grounding -- \
  /tmp/hle_notation_grounding_2147e9e.json \
  /tmp/hle_notation_audit_2147e9e.json
```

## Next gate

Do not broaden notation parsing yet. Use the 13 accepted target groundings to
separate downstream method gaps from remaining target/answer-format gaps, and
promote the 23 ambiguous rows into an adversarial regression suite. A future
executor may consume a grounded artifact only after its request-linked target,
supporting definitions, and provenance are replay-verified.
