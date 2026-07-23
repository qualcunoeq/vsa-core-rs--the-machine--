# Phase 17 — first genuine third-party release

The first non-fixture release uses eight original prompts from the GSM8K test
split published by OpenAI.  The repository describes GSM8K as human-written
grade-school math problems and publishes the raw test JSONL; its repository
license is MIT.  The release preserves the original wording and records the
source locator, citation, license, retrieval date, and hash of the checked-in
prompt subset.

```bash
cargo run --release --quiet --bin third_party_corpus_bench \
  data/third_party_gsm8k_release_v1.json
```

Observed baseline:

```text
release=third-party-gsm8k-test-subset-v1
cases=8
development=6
holdout=2
structural=8/8
realized_plans=0
false_authorizations=0
false_denials=0
```

All eight cases are intentionally labelled `understandable_unsupported`:
they are multi-step word problems outside the current bounded raw-decomposer
grammar.  This is therefore a refusal baseline, not evidence of GSM8K solving
competence.  It does establish that the Machine does not mis-authorize these
independent prompts and that the source/release/hash path is reproducible.

The release is deliberately small.  The next acquisition should expand to at
least four independent sources and add clearly in-scope cases before making
claims about recognition or execution recall.
