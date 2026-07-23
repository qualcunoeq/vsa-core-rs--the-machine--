# Phase 14 — Raw-problem decomposition

The raw front-end now emits typed plan sketches from a deliberately bounded
prose grammar. The sketch is scored independently, then sent through the
existing route planner; decomposition alone never authorizes execution.

```bash
cargo run --release --quiet --bin raw_decomposition_bench \
  data/raw_decomposition_ood_v1.json
```

Observed result:

```text
cases=500
structural=500/500
correct decisions=500/500
realized plans=450
replayed stages=800
ambiguous preserved=25
false authorizations=0
false denials=0
unnecessary decompositions=0
missed direct routes=0
```

The 25 ambiguous and 25 unsupported cases are intentionally not authorized.
The supported set includes direct, two-stage, and three-stage sketches. The
unified integration command now covers 2,340 cases (1,000 mixed, 340 typed
composition, 500 planner-selection, and 500 raw-decomposition cases).

This remains a bounded parser, not open-ended language understanding. Its
value is that structural decomposition is measured separately from route
realization and final replay.

## Natural-language pressure corpus

`raw_decomposition_ood_v2.json` contains 1,000 independently generated
textbook-style variants: reordered clauses, “plus”/“sum of” wording, bracketed
indices, irrelevant quantities, ambiguous alternatives, and missing bridges.
It achieves 1,000/1,000 structural decisions, 850 realized plans, 1,500
replayed stages, 75 ambiguities preserved, and zero authorization errors.

The unified release integration command accepts both raw corpora as its fourth
and fifth arguments and now covers 3,340 cases in total.
