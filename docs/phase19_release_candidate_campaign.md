# Phase 19 — Frozen Release-Candidate Campaign

Phase 19 adds no reasoning module. It freezes a release manifest and evaluates
public system boundaries through independently authored mixed scenarios:

```text
ingestion
→ calibrated abstention
→ cross-ontology composition
→ policy-gated promotion
→ rollback and historical replay
```

The campaign does not expose generator internals to the exercised boundaries,
and it does not mutate the live registry. Module versions and the corpus hash
are recorded in an immutable release manifest.

## Corpus

The 120-case corpus includes safe and ambiguous ingestion, unknown ontology
reports, valid and refused mixed-ontology routes, clean and blocked promotion,
and rollback after a candidate version has been staged.

Corpus SHA-256:

`c459acbddb520a84f6eb040ccea8f0c5196a947915e1ba2c7489c5564617d550`

Manifest SHA-256:

`294f449b49ce115d49bdc163b05b4f72d9f1aef2a4656bd47c1e27fe96bd5a33`

## Results

| Metric | Result |
| --- | ---: |
| Cases | 120 |
| Supported truth outcomes | 50 |
| Calibrated abstentions | 55 |
| False fact insertions | 0 |
| False authorizations | 0 |
| Successful cross-ontology routes | 20 |
| Clean promotions | 10 |
| Correct rollbacks | 5 |
| Historical replays | 120 / 120 |
| Resource-event accounting | 120 / 120 |

This is a frozen release-candidate baseline, not evidence of broad uncontrolled
real-world robustness. Its purpose is to expose integration regressions before
adding further architecture.
