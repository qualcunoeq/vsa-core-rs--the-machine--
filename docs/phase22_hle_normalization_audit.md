# Phase 22 — normalization-contamination mechanism audit

This is a shadow-only second pass over the 338 rows that Phase 21 marked
`apparent_gap_from_normalization`. It clusters representation mechanisms; it
does not change parsing, retrieve knowledge, or authorize answers.

Run:

```text
cargo run --bin hle_knowledge_audit -- \
  /tmp/hle_release_candidate_2147e9e.traces.jsonl \
  /tmp/hle_knowledge_audit_2147e9e.json
cargo run --bin hle_normalization_audit -- \
  /tmp/hle_knowledge_audit_2147e9e.json \
  /tmp/hle_normalization_audit_2147e9e.json
```

Source trace SHA-256: `3b0cc1aac3819b8f41343f21cd02ff8c54b0e5b3be1f99ce9164b5c6a2cb2348`

## Shadow mechanism counts

| Mechanism marker | Cases |
|---|---:|
| Specialist notation | 227 |
| Embedded formula | 105 |
| Cross-sentence binding | 4 |
| Nested question structure | 1 |
| Quotation or citation structure | 1 |
| Answer-format confusion | 0 |
| Implicit variables | 0 |
| Unresolved abbreviation | 0 |
| Domain terminology | 0 |
| **Total** | **338** |

The immediate signal is that most rows contain specialist mathematical or
scientific notation, while another 105 contain embedded formula material.
This is a representation surface worth addressing before broad retrieval.
The four cross-sentence and one nested-structure rows are smaller but likely
high-value parser tests because they exercise binding rather than vocabulary.

These are deliberately called *mechanism markers*, not confirmed root causes.
For example, a LaTeX marker can coexist with a genuine theorem or factual gap;
the presence of notation alone does not prove that normalization would make a
question solvable. The full records and five samples per cluster are retained
in the JSON report for blinded adjudication.

## Safety boundary

No parser rule, capability, knowledge source, registry, or ontology changed.
The next implementation step should select a small independent paraphrase and
notation corpus from these clusters, include ambiguous and unsupported
controls, and evaluate a shadow normalization contract before any promotion.
