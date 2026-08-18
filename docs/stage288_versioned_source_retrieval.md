# Stage 288 — versioned source retrieval

A fresh immutable-source campaign extends lineage-aware retrieval with freshness and query-budget gates. Claims remain non-authorizing unless the current version is unique, two independent lineages agree, the retrieval and policy receipts replay, and the query budget is sufficient.

* cases / exact decisions: 800 / 800
* current corroborated claims authorized: 160
* copied lineages refused: 120
* stale / conflicting / missing refused: 120 / 120 / 120
* budget / scope refused: 80 / 80
* retrieval replay / tamper: 800 / 800
* policy replay / tamper: 800 / 800
* provenance-complete cases: 800
* total query-cost units: 880
* false authorizations / denials: 0 / 0
* source-memory / registry / world-model mutations: 0 / 0
* HLE questions read: 0

The source documents are embedded immutable snapshots; no network, live registry, source memory, or world model is accessed.

Reproduce with `cargo run --quiet --bin stage288_versioned_source_retrieval`.
