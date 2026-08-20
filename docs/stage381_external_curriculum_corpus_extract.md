# Stage 381 — external curriculum corpus seed

- seed sources: 12
- extracted files: 12
- failed files: 0
- answer-like lines rejected: 11305
- structural non-problems rejected: 315636
- records with prompts: 2493 (development 1921, validation 572, sealed holdout 260)
- answer-key policy: answer keys are never read, parsed, inferred, or stored
- source manifest SHA-256: `90d42a6945eea636b041539138cd98b3e475557f7d0d4733df98f611ea01c346`
- sealed holdout manifest SHA-256: `f5db14844a7c7b83c038652c328599c9d640466fac84966bfc7a22faaf9a3e04`
- corpus SHA-256: `25908c284af53959dd813bd9df51aec3761b7f637437da1e51fc348efa60fa7e`

This corpus preserves naturally authored OpenStax exercise instructions and numbered problems from hash-pinned sources. Any line containing answer/solution language and all obvious decimal, table-of-contents, and page-heading artifacts are rejected. Answer keys and solutions are excluded and never read or inferred. Sealed holdout prompts are not present in the development report; only their provenance and prompt hashes are retained in the holdout manifest. This stage establishes a real external corpus boundary; it does not claim that the records are solved or that the source set is broad enough for the final exam.

Reproduce with `cargo run --quiet --bin stage381_external_curriculum_corpus_extract`.
Development records: `docs/stage381_external_curriculum_dev.json`
Sealed manifest: `docs/holdouts/stage381_external_curriculum_sealed_manifest.json`
Machine-readable report: `docs/stage381_external_curriculum_corpus_extract.json`
