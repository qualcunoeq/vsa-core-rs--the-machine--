# Stage 369 — two-lineage generic source acquisition

- selected / executable lineages: 2 / 2
- selection replay / module replays: true / 2
- source records: 2
- supported exercises / exact decisions: 2 / 8
- boundary cases / decisions / replays: 6 / 6 / 6
- execution replays / tamper rejections: 2 / 8
- source mutation rejected: true
- acquisition replay / clone-only promotion: true / true
- false authorizations / denials: 0 / 0
- live registry mutations / parent catalog unchanged: 0 / true
- corpus SHA-256: `99ef8899746b810be549dda6e2b9b0ebe78306b18c61816f1258de0063bb1214`

Two independently cited source lineages provide declarative rational-expression records to the same generic runtime. Supported exercises and explicit missing, ambiguous, and invalid-domain boundaries replay successfully. The acquisition receipt is promotable only in a clone; no live registry or parent catalog mutation occurs.

Reproduce with `cargo run --quiet --bin stage369_two_lineage_source_acquisition`.
Machine-readable report: `docs/stage369_two_lineage_source_acquisition.json`
