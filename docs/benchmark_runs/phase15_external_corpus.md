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

## Phase 15A — bounded arithmetic language expansion

The 20 v1 denials clustered into three arithmetic paraphrase families.  The
canonicalizer now maps those forms to the same `Evaluate a + b` representation:

```text
Determine the total when a is added to b.
Work out the result obtained by combining a and b by addition.
What number results if a is increased by b?
```

Each family has negative regression pairs (variables instead of literals and
explicit alternatives).  After the change, the frozen v1 corpus reports:

```text
structural=500/500
realized_plans=440
replayed_stages=720
development=400/400
holdout=100/100
false_authorizations=0
false_denials=0
```

The existing 3,340-case integration suite remains unchanged and passes with
zero authorization errors.

## Fresh holdout v2

To avoid repeatedly tuning the same 100 cases, `external_decomposition_v2.json`
is a new 200-case corpus with an untouched 120-case holdout.  It was created
after the v1 hardening and was not used to define the parsing rules:

```bash
cargo run --release --quiet --bin external_decomposition_bench \
  data/external_decomposition_v2.json
```

Observed result:

```text
structural=200/200
development=80/80
holdout=120/120
realized_plans=190
replayed_stages=200
false_authorizations=0
false_denials=0
```
