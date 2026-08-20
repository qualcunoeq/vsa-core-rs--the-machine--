# Stage 383 — external problem assembly

- source files / extracted: 12 / 12
- question candidates with context: 114 (development 114, non-question fragments 0)
- sealed holdout candidates: 9 (question candidates 9)
- answer-like lines rejected: 17805
- non-question fragments rejected: 12
- sealed prompt text stored: `false`
- baseline ready: `false`
- reason: candidates still require exact multi-line quality review and answer-key alignment
- corpus SHA-256: `514981326fe58910cbad22285c407628ee97146a03fc1f939bcb136a825a4ab0`

The assembler joins source exercise context and continuations without reading solutions. It rejects declarative/non-question fragments and preserves a hash-only sealed manifest. These are question candidates, not authorized benchmark problems; baseline scoring remains blocked until exact problem assembly and answer-key alignment are supplied through independent governed inputs.

Reproduce with `cargo run --quiet --bin stage383_external_problem_assembly`.
Machine-readable report: `docs/stage383_external_problem_assembly.json`
