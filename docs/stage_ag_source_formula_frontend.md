# Stage AG — source-derived formula technical frontend

| Measure | Result |
| --- | ---: |
| Development cases | 240 |
| Supported / ambiguous / refused | 120 / 40 / 80 |
| Frontend exact / replay / tamper | 300 / 300 / 300 |
| Downstream artifacts / exact / replay / tamper | 180 / 180 / 180 / 180 |
| Holdout frontend / downstream / replay | 60 / 60 / 60 |
| Ambiguity preserved / unsupported refused | 40 / 80 |
| Runtime domain-specific branches | 0 |
| False authorizations / denials | 0 / 0 |
| Live mutation | 0 |

The frontend derives its candidate aliases and required inputs from the source catalog. It contains no economics-specific route branch.

Reproduce with:

```text
cargo run --quiet --bin stage_ag_source_formula_frontend
```

Machine-readable report: `docs/stage_ag_source_formula_frontend.json`
