# Stage 368 — source-selection acquisition handoff

- metadata lineages selected / selection replay: 2 / true
- selected source matches typed catalog: true
- executable catalog lineages: 1
- source records / module replay: 4 / true
- supported exercises / exact decisions / execution replay: 4 / 4 / 4
- promotion blocked without two executable lineages: true
- false authorizations / live registry mutations: 0 / 0
- corpus SHA-256: `9763174a963c696fe2c8f57f5b6e8bccd09b988931c6e68fe92c791b7446169c`

The exact source-selection receipt gates generic catalog discovery and execution. The selected OpenStax lineage produces four typed records and four replayable executions; a second metadata lineage is not treated as executable corroboration, so promotion remains blocked.

Reproduce with `cargo run --quiet --bin stage368_source_selection_acquisition_handoff`.
Machine-readable report: `docs/stage368_source_selection_acquisition_handoff.json`
