# Phase 18 — Restricted third-party GSM8K subset

Commit: `58f1ca6` plus the restricted-scope follow-up.

This release is a deliberately small capability-scope experiment, not a claim
of general GSM8K solving.  It preserves the original GSM8K prompts and labels
four numeric relation families that the bounded decomposer can represent:

- elapsed-time multiplication;
- chained numeric age relations;
- age-difference relation;
- percentage loss;

One source prompt uses the ambiguous phrase “2 times more”; it is intentionally
abstained rather than assigned a convention.

Six additional source prompts remain understandable but unsupported multi-step
word problems.  They are expected to be rejected.  The release therefore
measures both acceptance on a narrow supported slice and refusal integrity on
nearby unsupported cases.

Source provenance is recorded in
`data/third_party_gsm8k_restricted_release_v1.json`; the checked-in prompt
subset is hashed independently in
`data/third_party_gsm8k_restricted_prompt_subset_v1.jsonl`.

## Baseline

Command:

```text
RUSTFLAGS='-Awarnings' cargo run --release --quiet --bin third_party_corpus_bench -- data/third_party_gsm8k_restricted_release_v1.json
```

Result:

```text
cases=11
release_hash=5f91ba488739fac4715e5098d59aa82d68acfdee9fcdbfeaf81b9a7b8a9ddd1a
structural=11/11
realized=4
replayed_stages=4
ambiguous_preserved=1
results=4/4
result_mismatches=0
false_auth=0
false_denials=0
development=7/7
holdout=4/4
failures={}
```

The four labeled in-scope cases were all accepted and replayed.  The ambiguous
case was preserved as an ambiguity, and all six understandable-but-unsupported
cases were rejected.  The holdout contains one supported case and three
unsupported cases, all classified correctly.

## Regression checks

The original eight-case third-party refusal baseline remains unchanged:

```text
cases=8 structural=8/8 realized=0 false_auth=0 false_denials=0
```

Targeted raw-decomposition tests (`3 passed`) and third-party release-governance
tests (`1 passed`) also pass.  This phase does not claim broad GSM8K coverage;
the next expansion should be selected from independently observed failure
clusters rather than treating GSM8K as one monolithic capability.
