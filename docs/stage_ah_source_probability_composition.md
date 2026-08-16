# Stage AH — source-derived probability composition

| Measure | Result |
| --- | ---: |
| Cases | 240 |
| Supported / ambiguous / refused | 120 / 40 / 80 |
| Exact decisions | 240/240 |
| Complete expectations | 120 |
| Source replay / bridge replay / tamper | 240 / 240 / 240 |
| Value checks | 240/240 |
| False authorizations / denials | 0 / 0 |
| Live mutation | 0 |

The bridge requires explicit outcome mapping, finite normalized probabilities, replayable source formulas, and integer-compatible values.

Reproduce with:

```text
cargo run --quiet --bin stage_ah_source_probability_composition
```

Machine-readable report: `docs/stage_ah_source_probability_composition.json`
