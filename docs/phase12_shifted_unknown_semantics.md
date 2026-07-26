# Phase 12 — Distribution-Shifted and Partially Supported Ingestion

Phase 12 places an independently templated language distribution in front of
the Phase 11 world-model boundary:

```text
shifted report
→ conservative classification
→ justified claim extraction
→ unsupported residual preservation
→ typed observation only when safe
→ replay
```

The corpus is generated from semantic families that are distinct from the
controlled Phase 11 templates. It includes nested quotations, indirect source
language, cross-sentence pronouns, dynamic aliases, contradictory clauses,
uncertain attribution, ellipsis, irrelevant narrative, temporal relations, and
unknown ontology terms.

## Outcomes

```rust
SafelyIngestible
Ambiguous
PartiallyIngestible
OntologyExtensionRequired
Unsupported
```

`PartiallyIngestible` is fail-closed for the residual: the supported status/time
claim is canonicalized and replayed, while the unsupported temporal or ontology
fragment remains in the receipt and is never inserted as a fact.

## Independent shifted corpus

The release contains 320 cases:

| Family | Cases |
| --- | ---: |
| Unfamiliar direct paraphrases | 50 |
| Nested quotations / indirect speech | 40 |
| Supported claim plus temporal residual | 40 |
| Cross-sentence pronouns | 40 |
| Dynamically introduced aliases | 30 |
| Contradictory clauses | 30 |
| Ellipsis and omitted arguments | 25 |
| Irrelevant narrative detail | 25 |
| Ontology extension residuals | 20 |
| Unknown semantic domains | 20 |

Corpus SHA-256:

`cb8457468962e3c5436f0e30c4913f07742ebd54fe9c62fd905d1074883375e8`

## Results

| Metric | Result |
| --- | ---: |
| Classification decisions | 320 / 320 |
| Safe/partial insertion decisions | 320 / 320 |
| False fact insertions | 0 |
| Ambiguity preservation | 320 / 320 |
| Ontology-gap identification | 320 / 320 |
| Replay receipts | 320 / 320 |

Observed class distribution:

| Outcome | Cases |
| --- | ---: |
| SafelyIngestible | 105 |
| Ambiguous | 135 |
| PartiallyIngestible | 40 |
| OntologyExtensionRequired | 20 |
| Unsupported | 20 |

This is a controlled distribution-shift test, not unrestricted natural-language
understanding. The important safety result is that unsupported semantic residue
does not become a guessed fact. The residual classifications are now suitable
inputs to the capability/ontology proposal pipeline.
