# Phase 20 — frozen HLE release-candidate diagnosis

This evaluation freezes the release candidate at `2147e9e` and runs the
complete checked-in `data/hle.jsonl` export without changing routing,
capabilities, ontology state, or the registry during scoring. The evaluator
refuses to run if implementation files changed after the frozen commit;
evaluator and report files are explicitly allowed so the committed harness
can reproduce the baseline without silently evaluating a newer implementation.

Run:

```text
cargo run --bin hle_release -- /tmp/hle_release_candidate_2147e9e.traces.jsonl /tmp/hle_release_candidate_2147e9e.summary.json
```

The per-question JSONL trace is intentionally written outside the repository
because the source corpus and traces are local benchmark material. Each row
contains the question hash, route trace, required capabilities, registry and
ontology versions, authorization/abstention receipt, answer provenance,
replay status, execution time, and bounded resource counters. The summary is
reproducible from the trace and the dataset hash below.

## Frozen inputs

| Field | Value |
|---|---|
| Release | `machine-release-candidate-19` |
| Machine commit | `2147e9e` |
| Dataset | `data/hle.jsonl` |
| Dataset SHA-256 | `31b26cc8e352af16bedb9a714feb788ae562be38898ab92dc54b4665882bf1c6` |
| Registry version | `machine-release-candidate-19` |
| Ontology version | `ontology-phases-11-17` |
| Questions | 2,500 |

## Diagnosis-first result

| Terminal classification | Questions |
|---|---:|
| Correct authorized answer | 2 |
| Incorrect authorized answer | 0 |
| Safely formalized but unsupported | 0 |
| Missing factual knowledge | 1,842 |
| Missing reasoning method | 222 |
| Missing ontology | 48 |
| Composition failure | 0 |
| Language-normalization failure | 234 |
| Visual input required | 115 |
| Ambiguous or defective question | 37 |

The decisive safety result is **zero incorrect authorized answers**. The run
produced 2 correct authorized answers and no false authorizations. The other
2,498 questions remained non-authorized; their terminal classes identify the
next work rather than treating all non-answers as one score failure.

Replay status was `not_applicable` for 2,498 abstentions, `not_recorded` for
the two accepted chess answers (the router supplied verified evidence but no
typed execution receipt), and `failed` for zero cases. This distinction keeps
absence of a replay receipt separate from a replay failure.

Total evaluation time was approximately 50.21 seconds (20.08 ms per
question); the slowest question took 544.44 ms. No registry, ontology, or
regression corpus was mutated. This is a frozen baseline and diagnosis
instrument, not evidence of broad HLE competence. Capability growth must use
the diagnostic clusters with independently validated contracts and a fresh,
untouched holdout.
