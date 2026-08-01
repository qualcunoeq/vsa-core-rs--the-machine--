# Phase 36 — Frozen HLE shadow run with classical-mechanics pack

This phase evaluates the externally sourced Phase 34 mechanics pack against
the frozen `data/hle.jsonl` release without changing the production router,
registry, or HLE score. It is a diagnostic grounding scan, not an answer
benchmark: a question is never authorized merely because it contains a
mechanics word.

## Reproducibility

```text
cargo run --quiet --bin hle_classical_mechanics_shadow -- \
  docs/phase36_hle_classical_mechanics_shadow.json
```

The frozen dataset and pack hashes, plus one record per question, are stored
in [`phase36_hle_classical_mechanics_shadow.json`](phase36_hle_classical_mechanics_shadow.json).
The report hash is:

```text
f4fae9996143a2c6e4aa9897b642ad7be92754b056560f5d17b139d32551dfc8
```

## Results

| Metric | Result |
| --- | ---: |
| HLE questions scanned | 2,500 |
| Exact pack-law mentions | 4 |
| Generic/ambiguous `energy` mentions | 77 |
| Broad mechanics vocabulary signals | 176 |
| Uniquely grounded pack candidates | 0 |
| Grounding failures | 77 |
| Pack-reached cases | 0 |
| Shadow correct answers | 0 |
| Shadow incorrect answers | 0 |
| False authorizations | 0 |
| Replay-verified answers | 0 |
| Production router mutated | no |
| Production HLE score changed | no |

The four exact law mentions were all rejected at the grounding boundary: the
questions use the phrase “kinetic energy” in advanced or non-pack contexts,
without a uniquely supported elementary numerical request. The 77 generic
`energy` mentions remain ambiguous because that alias intentionally maps to
multiple pack records. The 153 remaining broad mechanics signals do not name a
pack law and were not inferred into one.

## Interpretation

This run produced no new HLE answer and therefore does not change the frozen
baseline of 2/2,500 correct authorized answers. It does validate the external
growth loop's safety boundary: the pack did not turn broad physics vocabulary,
generic energy language, or advanced mechanics into false authorizations.

The next bottleneck is domain/language grounding, not the mechanics formulas or
their pressure-tested execution. A future domain pack should only be evaluated
after its terms can be uniquely connected to a typed request without broad
lexical inference.

