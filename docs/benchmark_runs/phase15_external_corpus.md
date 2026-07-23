# Phase 15 — Frozen external-style corpus evaluation

The generated OOD suites are now complemented by a separate corpus format that
records prompt provenance and an untouched holdout split.  The evaluator does
not alter the raw decomposer or relabel failures as unsupported: each case is
annotated independently as `supported`, `ambiguous`, or `unsupported`.

```bash
cargo run --release --quiet --bin external_decomposition_bench \
  data/external_decomposition_v1.json
```

The corpus contains 500 hand-audited textbook-style prompts (400 development,
100 stratified holdout).  It is deliberately kept outside the generated OOD
corpora.  The current seed is an external-style baseline rather than a claim
of third-party sourcing; the schema's `source` field is ready for replacing
entries with independently sourced textbook, exam, or worksheet prompts.

Observed baseline:

```text
cases=500
development=400
holdout=100
structural=480/500
realized_plans=420
replayed_stages=700
ambiguous_preserved=30
false_authorizations=0
false_denials=20
```

The 20 failures are all supported arithmetic paraphrases that are currently
outside the frozen parser grammar (`supported_case_not_realized`), split 16/4
between development and holdout.  The holdout therefore provides a genuine
baseline: it contains both already-supported constructions and unseen wording
that the current implementation safely abstains on.

The evaluator also reports failures by source family, so parser hardening can
be performed only on development cases and then checked once against the
locked holdout.  No automatic promotion, registry mutation, or capability
expansion is performed by this benchmark.
