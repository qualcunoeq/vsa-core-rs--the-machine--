# Stage 385 — external problem candidate repair

- parent candidates / split parents: 114 / 32
- child candidates before / after deduplication: 284 / 257
- duplicate child prompts removed: 27
- clean candidates after repair: 36
- quality flags: `{"layout_or_external_context":28,"multiple_numbered_items_remaining":21,"short_candidate":2,"split_parent_context_review":182,"truncated_tail_review":56}`
- sealed holdout records: 9
- sealed prompt text stored: `false`
- integrity checks: 10/10
- replay verified: `true`
- baseline ready: `false`

This stage splits only already-extracted development candidates, preserving each parent record, source line, and prompt hash lineage. Exact duplicate child prompts are retained once and counted. The sealed holdout is copied as metadata only; no sealed prompt text or answer key is read. The result remains a candidate corpus, not a scored benchmark, until exact problem review and independently governed answer alignment are complete.

Reproduce with `cargo run --quiet --bin stage385_external_problem_candidate_repair`.
Machine-readable report: `docs/stage385_external_problem_candidate_repair.json`
