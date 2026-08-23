# Stage 448 — bounded frequency-table frontend

This is an independently generated, answer-key-free benchmark for a narrow
source-backed frontend.  It accepts only an explicit two-column LaTeX table
whose headers identify a measured value and a positive integer frequency.  The
frontend lowers the table to the existing generic `weighted_mean` formula; it
does not infer chart geometry, missing cells, or population semantics.

| Metric | Result |
|---|---:|
| Cases | 240 |
| Supported / refused | 120 / 120 |
| Exact decisions | 240/240 |
| Supported values | 120/120 |
| Frontend replay verified | 240/240 |
| Tamper rejections | 240/240 |
| False authorizations / denials | 0 / 0 |

The refused half covers ambiguous header orientation, incomplete rows, missing
targets, non-positive frequencies, non-table prose, and three-column tables.
Both value/frequency and frequency/value column orientations are exercised.

The corpus, artifact, and implementation remain shadow-only.  No production
registry, curriculum manifest, or sealed partition was changed.

Evidence:

* corpus SHA-256: `075fc37cb78a3d9dddfe15fe08491cb4834f6cf83a14de55ba7f8d711af8e250`
* raw JSON report SHA-256: `410f983d375732c197376f63b4996eeb91dd255aaa64b3ef5ef05ab0d175e9fe`
* frontend source SHA-256: `2cebe5404e189c7e63b5fea24b88e9eef7e0a4e8a90e5db2adca928cb036a427`

Reproduce with:

```text
cargo run --quiet --bin stage448_frequency_table_frontend_bench
```
