# Source provenance integrity gate

This audit checks the shared citation contract across source-derived packs and
evaluator outputs. It is separate from subject semantics: a typed artifact is
not eligible for downstream use unless its source identity, license, retrieval
metadata, and evidence span are complete and replayable.

| metric | result |
| --- | ---: |
| source families | 8 |
| unique source identifiers | 11 |
| citation entries audited | 240/240 |
| valid citations | 240/240 |
| citation replay | 240/240 |
| tampered citation receipts rejected | 240/240 |
| catalog mutation cases rejected | 6/6 |
| evaluator receipts replayed | 3/3 |
| false authorizations | 0 |
| production registry mutations | 0 |

The audit covers formula catalogs, finite metric and topology definitions,
source relations, science-law records, and citations emitted by chemistry,
biology, and complex-arithmetic evaluators. The 240 entries are a deterministic
audit expansion over the extracted citations; the report also records the
number of unique source identifiers so repeated records are not mistaken for
new source material.

The shared `validate_source_citation` gate now requires a nonempty source ID,
title, section, HTTPS URL, license, retrieval timestamp, and evidence span.
Malformed catalog citations are rejected before execution. This remains a
shadow governance check and does not mutate any registry or curriculum route.

Machine-readable report: `docs/source_provenance_integrity.json`.

Run:

```text
cargo run --quiet --bin source_provenance_integrity_bench
```
