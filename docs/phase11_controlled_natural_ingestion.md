# Phase 11 — Controlled Natural-Language Ingestion

Phase 11 places a conservative semantic boundary before world-model updates:

```text
raw report
→ candidate entities, claims, and times
→ provenance and ambiguity
→ typed observation
→ replayable world-model ingestion
```

The parser never silently resolves an unknown entity, collision, hedge, or
unsafe date. Candidate parses retain confidence, alternative interpretations,
unresolved bindings, and source spans. Quoted reports are represented as
quoted candidates but are not inserted as asserted facts.

## Synthetic corpus

The independently generated controlled corpus contains 300 reports:

| Family | Cases |
| --- | ---: |
| Canonical assertions | 80 |
| Paraphrased assertions | 40 |
| Aliases and pronoun-style references | 30 |
| Invalid/uncertain times | 30 |
| Conflicting date formats | 20 |
| Hedged claims | 20 |
| Source quotations | 20 |
| Negation | 20 |
| Irrelevant details | 20 |
| Entity collisions/unresolvable references | 20 |

Corpus hash:

`b4ef26d44752b11c07c01a07ea339e51a9c79f1fa0d110dc43ac44f61d587df7`

Measured result:

| Metric | Result |
| --- | ---: |
| Claim/event extraction decisions | 300 / 300 |
| Ambiguity preservation | 300 / 300 |
| Rejection decisions | 300 / 300 |
| Downstream typed-ingestion outcomes | 300 / 300 |
| False fact insertions | 0 |
| Provenance/world replay receipts | 300 / 300 |

## Boundaries

This is a controlled grammar, not unrestricted web-text understanding. It does
not perform broad coreference resolution, external date interpretation, or
automatic fact publication for quotations and hedges. The next pressure phase
should use independently authored reports with cross-sentence references,
multiple timestamps, source assertions, and paraphrases not used by the
generator.
