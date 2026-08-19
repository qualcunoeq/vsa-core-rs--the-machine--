# Stage 376 — memory-backed source execution

- preflight verified: true
- retrieved catalogs / retrieval replays: 2 / 2
- authorized executions / execution replays: 2 / 2
- provenance-preserving executions: 2
- safe refusals / refusal replays: 4 / 4
- refusal classes (ambiguous, missing input, wrong domain, unknown formula): 1 / 1 / 1 / 1
- tampered catalog refusals: 1
- parent unchanged: true
- false authorizations / live registry mutations: 0 / 0
- corpus SHA-256: `f3bc992476e160561a19322f5f8e63c849acf28773774340d8395ee93d89de1a`

Execution consumed only exact catalogs retrieved from cloned curriculum memory. Two source-derived formulas executed with source provenance and replay receipts; ambiguous, incomplete, wrong-domain, unknown-formula, and tampered-catalog paths remained fail-closed.

Reproduce with `cargo run --quiet --bin stage376_memory_backed_source_execution`.
Machine-readable report: `docs/stage376_memory_backed_source_execution.json`
