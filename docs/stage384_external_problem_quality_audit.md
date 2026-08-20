# Stage 384 — external problem quality audit

- integrity checks: 11/11
- development / sealed candidates: 114 / 9
- question-like / clean development candidates: 114 / 16
- quality flags: `{"heading_or_section_leakage":4,"layout_or_external_context":31,"multiple_numbered_items":24,"truncated_tail_review":26,"unpunctuated_tail_review":61}`
- duplicate prompt hashes: 11
- sealed prompt text stored: `false`
- replay verified: `true`
- baseline ready: `false`

This independent audit consumes only Stage 383 records. It measures likely multi-problem merges, layout/context leakage, truncation indicators, and short candidates without re-extracting PDFs or reading answer keys. Sealed prompts remain hash-only. The benchmark is not baseline-ready until exact problem assembly and separately governed answer-key alignment are complete.

Reproduce with `cargo run --quiet --bin stage384_external_problem_quality_audit`.
Machine-readable report: `docs/stage384_external_problem_quality_audit.json`
