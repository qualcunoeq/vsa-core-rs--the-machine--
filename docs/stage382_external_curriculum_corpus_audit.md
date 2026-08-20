# Stage 382 — external curriculum corpus audit

- integrity checks: 14/14
- source files: 12
- records with prompts: 1921 development + 572 validation
- sealed holdout records: 260 (prompt text absent from development artifacts)
- instruction / numbered candidates: 2315 / 178
- duplicate prompt hashes: 0
- baseline ready: `false`
- corpus SHA-256: `25908c284af53959dd813bd9df51aec3761b7f637437da1e51fc348efa60fa7e`

The source boundary, file hashes, split membership, answer-like filtering, duplicate policy, and replay hashes pass independently. Baseline scoring is intentionally not authorized yet: the extracted records are line-level candidates and require complete problem assembly plus answer-key alignment before they can be treated as benchmark questions.

Reproduce with `cargo run --quiet --bin stage382_external_curriculum_corpus_audit`.
Machine-readable report: `docs/stage382_external_curriculum_corpus_audit.json`
