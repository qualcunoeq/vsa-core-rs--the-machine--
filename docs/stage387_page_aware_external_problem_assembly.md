# Stage 387 — page-aware external problem assembly

- run mode: `full_seed`
- source offset / limit / extracted / pages: 0 / 12 / 12 / 13442
- page-cache hits / misses: 12 / 0
- development / sealed candidates: 6539 / 1091
- answer-like / nonquestion rejects: 0 / 3527
- candidates retaining multiple markers: 858
- duplicate prompt hashes: 1132
- sealed prompt text stored: `false`
- baseline ready: `false`
- corpus SHA-256: `f9e19c72da060d16f403957e642f285911d34be6f5abb45bd1c0ffc8a3403a49`

This stage consumes the same hash-pinned seed sources through page-bounded extraction. Partitioning is derived from source/page/line provenance rather than prompt content. Sealed candidates are recorded as metadata and hashes only; development heuristics never classify sealed prompts. The corpus remains a candidate set until exact quality review and independently governed answer alignment are complete.

Reproduce with `cargo run --quiet --bin stage387_page_aware_external_problem_assembly`. For resumable extraction, set `STAGE387_SOURCE_OFFSET`, `STAGE387_SOURCE_LIMIT`, and `STAGE387_PAGE_CACHE_DIR`.
Machine-readable report: `docs/stage387_page_aware_external_problem_assembly.json`
