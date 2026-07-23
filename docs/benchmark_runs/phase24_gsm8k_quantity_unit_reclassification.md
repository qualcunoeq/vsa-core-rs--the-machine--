# Phase 24 — GSM8K QuantityRelation + UnitQuantity re-evaluation

This release re-runs the immutable 100-case GSM8K restricted slice after the
unit-aware vertical and its typed composition bridges were added.  The v2
candidate manifest is locked to the Phase 19 base release hash
`c3d288f3ecf5f4bf1bbc6e8160e0edb1cf469e2df0db21f3476caddd330b1374`; its
configuration hash is
`b769bcb7f39e651dca3fe8e1d1dddc193f0352bf6e0b724590e6b202a1a2fd66`.

The prior candidate release remains immutable.  This release distinguishes
the newly reviewed route families:

```text
existing raw route
QuantityRelation route
UnitQuantity route
ambiguous
unsupported
```

## Evaluation

```text
RUSTFLAGS='-Awarnings' cargo run --release --quiet --bin gsm8k_quantity_candidate_bench -- data/third_party_gsm8k_quantity_candidate_v2.json
```

```text
cases=100
structural=100/100
existing=4
quantity_expected=7
quantity_realized=7
quantity_replayed=7
unit_expected=1
unit_realized=1
unit_replayed=1
ambiguous=2
unsupported=86
results=12/12
false_auth=0
false_denials=0
candidate_leakage=0
failures={}
```

The total accepted slice remains 12 cases, but one previously QuantityRelation
candidate now travels through the UnitQuantity route.  Both route families
bridge into existing verified execution and replay successfully.  The two
ambiguous cases and all remaining unsupported cases retain their conservative
outcomes.

This is a diagnostic re-evaluation only.  The global router and the original
GSM8K v2 release are unchanged.  The result validates route attribution and
typed integration, not general GSM8K coverage.
