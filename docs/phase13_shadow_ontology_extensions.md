# Phase 13 — Shadow Ontology-Extension Proposals

Phase 13 connects shifted-language residuals to the existing diagnostic
capability-proposer model without mutating the live ontology:

```text
unknown residuals
→ residual clusters
→ bounded semantic contract
→ generated boundary corpus
→ sandbox evaluation
→ diagnostic parser/world-model extension
```

The proposal is explicitly non-authorizing. It has no registry write path and
the generated `ParserWorldModelExtension` remains `applied: false`.

## Residual clustering

The Phase 12 corpus produced three repeated ontology residual clusters meeting
the evidence threshold:

```text
battery, location, temperature
```

These are represented as residual evidence with source case IDs and original
report text. The proposer creates `ObservedAttributeV1` using the existing
`CapabilityContractProposal` type, including supported, ambiguous, and
unsupported pattern boundaries, assumptions, safety invariants, and a
diagnostic world-model bridge.

## Independent sandbox corpus

The proposal-generated corpus contains 100 cases:

| Family | Cases |
| --- | ---: |
| Positive explicit attributes | 30 |
| Positive paraphrases | 30 |
| Hedged/ambiguous attributes | 20 |
| Unsupported ownership attributes | 20 |

Sandbox result:

| Metric | Result |
| --- | ---: |
| Boundary decisions | 100 / 100 |
| Paraphrase decisions | 30 / 30 |
| Downstream-safe candidates | 60 / 60 |
| False fact insertions | 0 |
| Live ontology mutations | 0 |

Sandbox proposal hash:

`48662bca3aaa72109cbc4450d929ecef9c6b7110bdc86b79c19c5ba1574d2c46`

This is a shadow proposal, not a promoted ontology extension. The result shows
that repeated semantic residuals can produce a bounded, testable proposal while
preserving ambiguity and preventing unsupported language from becoming facts.
