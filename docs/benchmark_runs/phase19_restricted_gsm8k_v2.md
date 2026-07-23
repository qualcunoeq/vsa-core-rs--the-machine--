# Phase 19 — 100-case restricted GSM8K release

This release expands the restricted third-party experiment to 100
source-preserved items from the official GSM8K test JSONL.  It is still a
bounded scope evaluation, not a general GSM8K solver benchmark.

The source file is recorded with SHA-256
`3730d312f6e3440559ace48831e51066acaca737f6eabec99bccb9e4b3c39d14`.
The checked-in 100-prompt subset is separately hashed in
`data/third_party_gsm8k_restricted_prompt_subset_v2.jsonl`.

Scope labels:

- 4 supported numeric relation cases;
- 2 ambiguous “times more” cases, conservatively abstained;
- 94 understandable but unsupported multi-step cases.

## Baseline

```text
RUSTFLAGS='-Awarnings' cargo run --release --quiet --bin third_party_corpus_bench -- data/third_party_gsm8k_restricted_release_v2.json
```

Release hash:
`c3d288f3ecf5f4bf1bbc6e8160e0edb1cf469e2df0db21f3476caddd330b1374`

```text
cases=100
structural=100/100
supported_expected=4
realized=4
replayed_stages=4
ambiguous_preserved=2
results=4/4
result_mismatches=0
false_auth=0
false_denials=0
development=90/90
holdout=10/10
failures={}
rejection_clusters: {fractional_quantity: 11, multi_step_quantity_arithmetic: 20, percentage_discount_finance: 19, ratio_rate_proportion: 22, temporal_or_sequential_reasoning: 7, unit_measurement_conversion: 15}
```

The holdout contains three supported cases, one ambiguity, and six
unsupported cases; all ten decisions are correct.  This is an acceptance and
refusal-integrity baseline for a narrow external slice.  It does not claim
coverage of the remaining GSM8K problem families.

## Unsupported-case diagnostic clusters

The evaluator now records a deterministic, case-level diagnostic reason for
each expected unsupported item and aggregates those reasons into research
clusters.  These labels describe likely missing capability families; they do
not change authorization or infer that a new capability is safe to add.

```text
fractional_quantity              11
multi_step_quantity_arithmetic   20
percentage_discount_finance      19
ratio_rate_proportion             22
temporal_or_sequential_reasoning  7
unit_measurement_conversion      15
total                            94
```

The largest cluster is `ratio_rate_proportion` (22 cases), followed by
`multi_step_quantity_arithmetic` (20) and
`percentage_discount_finance` (19).  This makes quantity/rate reasoning a
concrete candidate for a future bounded vertical, while preserving the
current safe refusal boundary.  Cluster assignments are exposed in the
`ThirdPartyReport` as `rejection_clusters` and immutable case-id keyed
`rejection_reasons` for follow-up analysis.
