# Stage AE — autonomous source capability acquisition

The source document was parsed into five declarative formula records and evaluated by the domain-agnostic expression runtime. No economics-specific executor branch is present.

| Measure | Result |
| --- | ---: |
| Source records | 5/5 validated |
| Independent development exercises | 240 |
| Development supported / ambiguous / refused | 120 / 40 / 80 |
| Development exact decisions | 240/240 |
| Development artifacts / replay / tamper | 120 / 240 / 240 |
| Untouched holdout supported / exact / replay / tamper | 60 / 60 / 60 / 60 |
| Source mutations rejected | 6/6 |
| Provenance-preserved complete artifacts | 180 |
| Runtime domain-specific branches | 0 |
| False authorizations / denials | 0 / 0 |
| Live mutation | false |

Reproduce with:

```text
cargo run --quiet --bin stage_ae_source_capability_acquisition
```

Machine-readable report: `docs/stage_ae_source_capability_acquisition.json`
