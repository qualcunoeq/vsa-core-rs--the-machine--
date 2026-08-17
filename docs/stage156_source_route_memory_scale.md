# Stage 156 — Long-term memory for source/multimodal routes

This checkpoint exercises the append-only curriculum memory using the sealed
Stage 155 source and raw-OCR route report as provenance. It stores immutable
typed and replay receipts, then validates exact route/version retrieval,
duplicate and tamper rejection, and deterministic reconstruction.

Results:

- records: **100,000**;
- segments: **391** at capacity 256;
- replay verification: **100,000/100,000**;
- duplicate rejection: **1/1**;
- invalid-record rejection: **1/1**;
- exact visual-statistics typed retrieval has zero contamination;
- unknown-route retrieval returns zero records;
- tamper rejection: **1,000/1,000** sampled records;
- reconstruction hash: equal across independently rebuilt memory;
- live registry/curriculum mutations: **0 / 0**.

Each record retains the immutable Stage 155 report hash and Stage 154 cloned
admission provenance. The machine-readable report is
`docs/stage156_source_route_memory_scale.json`.
