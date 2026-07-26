# Phase 15 — Relational and Temporal Location Realization

Phase 15 applies the generic ontology-realization path to the existing location
proposal:

```text
location proposal
→ containment-aware schema
→ presence/movement/proximity/badge artifacts
→ temporal ledger
→ compatibility-aware contradiction checks
→ impossible-event rejection
→ replay and downstream query
```

The realization is shadow-only. It uses aliases, parent-place relations,
precision, validity intervals, and event kinds as schema data. It does not add a
location-specific live executor or mutate the ontology registry.

## Boundary distinctions

The schema keeps these meanings separate:

```text
Alice is in Building A       → Presence / coarse
Alice is in Room 4            → Presence / fine / contained by Building A
Alice was near Building A     → Proximity / approximate
Alice entered Building A      → Movement event
Alice's badge was detected    → BadgeDetection, not presence
```

It preserves aliases (`Building Alpha`, `Bldg A`, `R4`), entity/place
ambiguity, stale timestamps, and compatible coarse/fine observations. Same-time
incompatible presences become contradictions; movement from an inconsistent
source state is rejected as impossible.

## Independent corpus

The corpus contains 270 cases:

| Family | Cases |
| --- | ---: |
| Coarse presence | 50 |
| Fine containment | 40 |
| Place aliases | 30 |
| Movement events | 30 |
| Approximate proximity | 20 |
| Badge detections | 20 |
| Rewrites | 20 |
| Simultaneous conflicting sensors | 10 |
| Ambiguous proximity/entity binding | 30 |
| Impossible movement | 10 |
| Unsupported ownership semantics | 10 |

Corpus SHA-256:

`0047e82ba91bb6728f19757870990e7f742ab80da6a00c8597d218e40fce9368`

## Results

| Metric | Result |
| --- | ---: |
| Outcome decisions | 270 / 270 |
| Typed location artifacts | 220 |
| Contradictions detected | 10 |
| Impossible movements rejected | 10 |
| Rewrite groups stable | 20 / 20 |
| Downstream-safe queries | 220 / 220 |
| Replay receipts | 270 / 270 |
| Tamper checks rejected | 270 / 270 |
| Live ontology mutations | 0 |

This validates a relational/temporal shadow realization while keeping presence,
movement, proximity, and sensor evidence from collapsing into one fact.
