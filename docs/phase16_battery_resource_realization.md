# Phase 16 — Evolving Battery/Resource Realization

Phase 16 adds a third ontology shape to the shadow realization pipeline:

```text
battery observation/event
→ typed charge or capacity artifact
→ charging/discharging/swap transition
→ resource ledger
→ impossible-increase and stale-state checks
→ threshold prediction and replay
```

Charge percentage, qualitative level, capacity, charging, discharging, and
replacement are distinct semantic kinds. Device aliases and ownership bindings
are resolved from the synthesized schema; no generic percentage is silently
treated as battery state. Promotion and live registry mutation remain disabled.

## Independent corpus

The 300-case corpus includes:

| Family | Cases |
| --- | ---: |
| Numeric charge levels | 70 |
| Qualitative levels | 30 |
| Capacity readings | 30 |
| Charging events | 30 |
| Replacement events | 20 |
| Rewrites | 20 |
| Stale readings | 20 |
| Ambiguous/missing time | 40 |
| Impossible increases | 20 |
| Unsupported non-battery reports | 20 |

Corpus SHA-256:

`047ff408db1d383d6dbd372eda1c02526e346a37a2fd0fa3c2cadb22c29cb41a`

## Results

| Metric | Result |
| --- | ---: |
| Outcome decisions | 300 / 300 |
| Typed artifacts | 220 |
| Charging events | 30 |
| Replacement events | 20 |
| Impossible increases rejected | 20 |
| Stale readings detected | 30 |
| Threshold predictions | 50 |
| Rewrite groups stable | 20 / 20 |
| Downstream-safe queries | 220 / 220 |
| Replay receipts | 300 / 300 |
| Tamper checks rejected | 300 / 300 |
| Live ontology mutations | 0 |

This validates an evolving consumable-resource ontology alongside scalar
temperature and relational/temporal location, while retaining explicit
state-transition and provenance boundaries.
