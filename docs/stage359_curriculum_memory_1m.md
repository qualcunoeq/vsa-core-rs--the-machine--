# Stage 359 — one-million curriculum memory scale

* shadow packs / descriptors: 38 / 131
* records / segments: 1,000,000 / 3,907
* exact retrieval / prerequisite closure: 1,500 / 1,500
* ambiguous / stale / unknown / provenance refusals: 300 / 300 / 200 / 100
* replay verified / tamper sample rejected: 1,000,000 / 5,000
* reconstruction records / hash equal: 1,000,000 / true
* retrieval contamination: 0
* parent memory / manifest unchanged: true / true
* false authorizations / denials: 0 / 0
* live memory / registry mutations: 0 / 0

The existing curriculum-memory harness was run at the explicit one-million-record target in a shadow clone. Exact domain, artifact, and version retrieval remained isolated; ambiguous, stale, unknown, and provenance-invalid queries failed closed. Deterministic reconstruction matched the original memory, and the parent memory and live registry were unchanged.
