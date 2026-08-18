# Stage 289 — retrieval-guided investigation

A 1,000-case epistemic campaign selects an information-gain query, retrieves immutable versioned claims, applies lineage/freshness/budget policy, and updates beliefs only on authorized evidence.

* cases / exact decisions: 1000 / 1000
* recommendation and q0 selection: 1000 / 1000
* authorized retrievals / resolved beliefs: 300 / 300
* ambiguous outcomes: 700
* retrieval replay / tamper: 1000 / 1000
* belief replay / tamper: 1000 / 1000
* policy replay / tamper: 1000 / 1000
* provenance-complete cases: 1000
* false authorizations / denials: 0 / 0
* source-memory / registry / world-model mutations: 0 / 0
* HLE questions read: 0

The source documents are embedded immutable snapshots. The campaign never mutates live curriculum state and never treats a retrieved claim as a fact without independent current corroboration.

Reproduce with `cargo run --quiet --bin stage289_retrieval_guided_investigation`.
