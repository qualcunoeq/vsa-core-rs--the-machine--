# Stage 346 — independent externality audit

* release / questions: `external-math-exam-v1` / 4000
* split counts: `{'development': 3000, 'sealed': 1000}`
* cross-split exact overlaps: 0
* duplicate prompts: 0 groups / 0 records
* repeated normalized templates: 93 groups / 269 records
* answer markers: 0
* repository exact prompt overlaps: 0
* overlap locations: `{}`
* sealed answers parsed: false
* verdict: **clean_under_static_audit**

This evaluator is implemented independently of the Rust benchmark and curriculum schemas. It verifies release hashes, split isolation, wording/provenance metadata, exact and normalized overlap indicators, and answer-marker exposure. Oracle contents are hashed for release integrity but never parsed. Repeated templates are reported as a diagnostic signal, not automatically treated as contamination.
