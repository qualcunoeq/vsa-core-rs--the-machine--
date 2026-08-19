# Stage 338 — Goal 6 source-selected unit technical language

- Selected source / route: `docs/sources/openstax_unit_conversion_goal6_catalog.txt` / `UnitConversion`
- Source records: 5
- Development cases / all frontend exact / replay / tamper: 240 / 300 / 300 / 300
- Development supported / ambiguous / refused: 120 / 40 / 80
- Downstream artifacts / exact / replay / tamper: 180 / 180 / 180 / 180
- Holdout cases / frontend / downstream / replay: 60 / 60 / 60 / 60
- Source mutations rejected: 6 / 6
- Provenance-preserved artifacts: 180
- False authorizations / denials: 0 / 0
- Answer keys / HLE questions / production mutations: 0 / 0 / 0
- Manifest unchanged: true

The generic frontend derives aliases and required inputs from the selected source records; no statistics-specific parser or evaluator branch was added, and promotion remains disabled.
