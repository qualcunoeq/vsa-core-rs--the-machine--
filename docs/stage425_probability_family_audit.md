# Stage 425 — answer-key-blind probability-family audit

- source records / quality-clean / rejected: 5566 / 2105 / 3461
- probability-signal records: 108
- promising repeated family: finite uniform/labeled experiment events
- candidate records / existing typed frontends: 19 / 0
- contracts proposed: 0
- answer keys / plaintext answers / production authorizations / false authorizations: 0 / 0 / 0 / 0
- sealed manifest records: 1091 (prompt text not consumed)
- manifest unchanged: true

| Family | Cases | Dev | Val | Sources | Explicit experiment | Event target | Literal distribution inputs |
|---|---:|---:|---:|---:|---:|---:|---:|
| `uniform_die` | 12 | 11 | 1 | 2 | 0 | 12 | 0 |
| `labeled_die` | 37 | 26 | 11 | 1 | 19 | 37 | 0 |
| `card_or_tile_draw` | 34 | 27 | 7 | 1 | 22 | 34 | 0 |
| `repeated_game` | 6 | 3 | 3 | 1 | 0 | 6 | 0 |
| `empirical_or_subjective` | 7 | 5 | 2 | 1 | 2 | 6 | 0 |
| `coin_experiment` | 7 | 6 | 1 | 2 | 0 | 7 | 0 |
| `other_finite_experiment` | 5 | 4 | 1 | 3 | 0 | 4 | 0 |

The repeated uniform/labeled experiment family is a source-selection candidate, not yet a capability contract. The next phase must build an independent finite-experiment corpus and validate sample-space/event lowering before any external alignment or promotion.
