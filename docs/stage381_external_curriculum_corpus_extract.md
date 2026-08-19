# Stage 381 — external curriculum corpus seed

- seed sources: 3
- extracted files: 3
- failed files: 0
- answer-like lines rejected: 2242
- structural non-problems rejected: 66785
- records with prompts: 773 (development 597, validation 176, sealed holdout 59)
- answer-key policy: answer keys are never read, parsed, inferred, or stored
- source manifest SHA-256: `90d42a6945eea636b041539138cd98b3e475557f7d0d4733df98f611ea01c346`
- sealed holdout manifest SHA-256: `7508fd46089be5cac67df4116b2679ebdc0ee51edd7a9800bcc4d01fe6571f48`
- corpus SHA-256: `a40d65d98438538d0751a5d5cfe1cd115a7da6c779ae43350bbba6da163f7059`

This corpus preserves naturally authored OpenStax exercise instructions and numbered problems from hash-pinned sources. Any line containing answer/solution language and all obvious decimal, table-of-contents, and page-heading artifacts are rejected. Answer keys and solutions are excluded and never read or inferred. Sealed holdout prompts are not present in the development report; only their provenance and prompt hashes are retained in the holdout manifest. This stage establishes a real external corpus boundary; it does not claim that the records are solved or that the source set is broad enough for the final exam.

Reproduce with `cargo run --quiet --bin stage381_external_curriculum_corpus_extract`.
Development records: `docs/stage381_external_curriculum_dev.json`
Sealed manifest: `docs/holdouts/stage381_external_curriculum_sealed_manifest.json`
Machine-readable report: `docs/stage381_external_curriculum_corpus_extract.json`
