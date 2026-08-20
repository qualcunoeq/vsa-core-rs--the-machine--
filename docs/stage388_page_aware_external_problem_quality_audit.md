# Stage 388 — page-aware external problem quality audit

- integrity checks: 11/11
- development / sealed candidates: 6539 / 1091
- clean candidates: 2475
- quality flags: `{"heading_or_section_leakage":245,"layout_or_external_context":755,"multiple_numbered_items":858,"short_candidate":36,"truncated_tail_review":1230,"unpunctuated_tail_review":3286}`
- duplicate prompt hashes: 1132
- full pinned seed and cache-complete: `true`
- sealed prompt text stored: `false`
- replay verified: `true`
- baseline ready: `false`

This audit consumes only Stage 387 records. It does not re-extract PDFs, read sealed prompt text, or inspect answer keys. Duplicate and quality-review findings must be repaired before answer alignment and baseline scoring.

Reproduce with `cargo run --quiet --bin stage388_page_aware_external_problem_quality_audit`.
Machine-readable report: `docs/stage388_page_aware_external_problem_quality_audit.json`
