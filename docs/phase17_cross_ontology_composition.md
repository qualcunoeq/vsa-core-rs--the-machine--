# Phase 17 — Generic Cross-Ontology Composition

Phase 17 composes independently realized temperature, location, and battery
artifacts through typed bridge contracts:

```text
typed ontology artifacts
→ generic route selection
→ entity/time/scope/causal validation
→ joint finding artifact
→ contradiction and missing-evidence diagnosis
→ replay/tamper verification
```

The planner has no ontology-pair-specific execution branch. Bridges declare
input domains, output type, scope, overlap, entity, provenance, and causal
requirements; route selection chooses the lowest-cost valid bridge.

## Negative boundaries

Composition is refused or left ambiguous when:

* entity identity is not proven;
* validity intervals do not overlap;
* provenance or investigation scope is unauthorized;
* a causal relationship is only assumed;
* an ontology input is missing;
* an intermediate artifact already carries a contradiction.

## Independent corpus

The corpus contains 300 investigations:

| Family | Cases |
| --- | ---: |
| Location/battery valid routes | 80 |
| Temperature/battery valid routes | 40 |
| Rewritten valid routes | 40 |
| Entity mismatch | 30 |
| Non-overlapping timestamps | 30 |
| Unauthorized causal links | 20 |
| Domain contradiction localization | 20 |
| Missing ontology evidence | 20 |
| Invalid provenance/scope | 20 |

Corpus SHA-256:

`723120bfd1300290c0ebb96d8dae2b7bac170b0a5b288ebf6c567bd40a39c915`

## Results

| Metric | Result |
| --- | ---: |
| Route/outcome decisions | 300 / 300 |
| Valid joint intermediate artifacts | 160 |
| Contradiction localization | 300 / 300 |
| Missing-evidence detection | 300 / 300 |
| False composition authorizations | 0 |
| Ambiguity preservation | 300 / 300 |
| Rewrite groups stable | 40 / 40 |
| Downstream rankings | 160 |
| Replay receipts | 300 / 300 |
| Tamper checks rejected | 300 / 300 |
| Live mutations | 0 |

This validates cross-ontology composition as a governed typed operation rather
than a shortcut that inherits authority from either input ontology.
