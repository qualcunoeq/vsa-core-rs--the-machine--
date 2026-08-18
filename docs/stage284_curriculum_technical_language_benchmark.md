# Stage 284 — curriculum technical-language benchmark

An independently varied 2,000-case technical-language corpus was routed through all bounded frontends. Prompts include naturalized definitions, notation, competing domains, missing conventions, unsupported operators, and prose distractors.

* exact decisions: 2000/2000
* authorized: 840
* ambiguity preserved: 580
* unsupported refusals: 580
* replay / tamper: 2000 / 2000
* false authorizations / denials: 0 / 0
* route leakage: 0
* HLE questions read / production mutations: 0 / 0

Permanent split: development 600, validation 400, sealed 400, boundary 600.

Reproduce with `cargo run --quiet --bin stage284_curriculum_technical_language_benchmark`.
