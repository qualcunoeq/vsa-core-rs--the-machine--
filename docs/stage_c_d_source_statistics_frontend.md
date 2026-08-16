# Stage C/D — Finite-statistics language frontend

The source-derived statistics catalog now has a controlled technical-language
boundary. The frontend extracts only explicitly labeled quantities (`sum`,
`count`, `weighted_sum`, `total_weight`, `n`, and `p`), emits provenance spans,
and preserves ambiguous output requests. It refuses unsupported continuous
statistics, regression, confidence intervals, and unlabeled observations.

The independent corpus contains 240 natural-language cases:

* 120 uniquely formalized and authorized through the source catalog;
* 40 ambiguous cases preserved without a typed request;
* 80 missing or unsupported cases refused.

Results: 240/240 exact frontend decisions, 120/120 authorized values replayed,
240/240 frontend replay receipts, 240/240 frontend tamper rejections,
240/240 downstream tamper rejections, and zero false authorizations or
denials. Corpus hash:
`8b5656ceaceea7d3628835143810c1f683dba2071b3ce94e61269a1ad5d7b292`.

Run:

```text
cargo run --quiet --bin source_statistics_frontend_bench
```
