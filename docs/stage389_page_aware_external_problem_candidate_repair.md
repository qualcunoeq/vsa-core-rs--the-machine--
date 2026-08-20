# Stage 389 — page-aware external problem candidate repair

- parent / split parents: 6539 / 131
- children before / after deduplication: 6741 / 5566
- duplicate child prompts removed: 1175
- clean candidates after repair: 2105
- sealed holdout records: 1091
- sealed prompt text stored: false
- integrity checks: 10/10
- replay verified: true
- baseline ready: false

This deterministic repair splits only development/validation records, preserves source and parent lineage, and removes exact duplicate prompt hashes. The sealed partition remains metadata-only. Answer alignment and baseline scoring are separate gates.

Reproduce with cargo run --quiet --bin stage389_page_aware_external_problem_candidate_repair.
Machine-readable report: docs/stage389_page_aware_external_problem_candidate_repair.json
