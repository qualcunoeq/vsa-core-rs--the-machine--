# Phase 14 — Governed Shadow Ontology Realization

Phase 14 turns one Phase 13 shadow proposal into a complete, cloned semantic
realization without enabling promotion:

```text
validated proposal
→ generic typed attribute schema
→ numeric/unit normalization
→ temporal artifact
→ shadow ledger storage
→ contradiction detection
→ query/replay and tamper checks
```

The schema is synthesized from proposal data. The interpreter is driven by
surface terms, unit definitions, contexts, and explicit safety requirements;
there is no temperature-specific execution branch and no live registry write.

## Temperature realization

The schema supports only explicitly identified ambient or object temperature
readings with an entity, measurement time, and Celsius/Fahrenheit unit. It
normalizes Fahrenheit to exact milli-Celsius integer values, preserves an
approximate-reading flag, and retains source provenance.

The cloned ledger detects same-entity/context/time readings that disagree, while
keeping both source observations for later hypothesis reasoning. Queries and
replay operate on the shadow ledger only.

## Independent corpus

The 240-case corpus includes:

| Family | Cases |
| --- | ---: |
| Celsius readings | 70 |
| Fahrenheit readings | 40 |
| Approximate/contextual readings | 30 |
| Paraphrase rewrites | 20 |
| Conflicting sensor readings | 10 |
| Missing numeric/unit semantics | 50 |
| Unsupported humidity/pressure reports | 20 |

Corpus SHA-256:

`a339ea5a4da036e6df513831c64cedfeb5b5ab1967ae28464c8f240bff74254e`

## Results

| Metric | Result |
| --- | ---: |
| Outcome decisions | 240 / 240 |
| Typed supported artifacts | 170 |
| Ambiguities preserved | 50 / 50 |
| Unsupported reports rejected | 20 / 20 |
| Rewrite pairs stable | 20 / 20 |
| Contradictions detected | 10 |
| Downstream-safe queries | 170 / 170 |
| Replay receipts | 240 / 240 |
| Tamper checks rejected | 240 / 240 |
| Live ontology mutations | 0 |

Promotion remains disabled. This validates a complete shadow realization and
its boundaries, not deployment of a new ontology category.
