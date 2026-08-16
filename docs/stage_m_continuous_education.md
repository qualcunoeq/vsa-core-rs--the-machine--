# Stage M — bounded continuous self-education planning

This stage extends the one-shot gap planner into a deterministic, multi-round
education campaign. It consumes only replayable typed gap observations and
source-backed module metadata. A selected module removes only its exact
actionable artifacts from a sandbox residual set. Ambiguous and explicitly
unsupported observations are never resolved by lexical overlap.

The campaign is proposal-only: it does not mutate the curriculum manifest,
registries, or production routing, and it does not authorize an answer merely
because a learning module is selected.

## Independent campaign

Reproduce with:

```text
cargo run --quiet --bin stage_m_continuous_education
```

The machine-readable report is
`docs/stage_m_continuous_education.json`.

| Measure | Result |
|---|---:|
| Episodes | 300 |
| Exact campaign decisions | 300/300 |
| Campaign replay verification | 300/300 |
| Deterministic reruns | 300/300 |
| Tamper rejections | 300/300 |
| Manifest unchanged | 300/300 |
| Resolved actionable gap cases | 1,110 |
| Residual cases (ambiguous/unsupported/blocked) | 150 |
| Selected learning steps | 480 |
| Blocked steps | 30 |
| No-coverage steps | 60 |
| Complete steps | 210 |
| Source-gated selections | 300/300 |
| Forbidden selections | 0 |
| False authorizations | 0 |
| Live registry mutations | 0 |

The candidate set deliberately includes source-free shortcuts, an extension
with an unknown prerequisite, and a broad lexical candidate. The controller
selects only authoritative candidates with source provenance, independent
exercise evidence, and complete prerequisite closure. Unknown-prerequisite
coverage is reported as blocked; broad labels and ambiguity remain residuals.

Corpus SHA-256:
`3ba160d66ce2cc169ca8083c7c206027febf38db6aba509a66564deace6e61aa`

This validates campaign sequencing and governance, not automatic promotion or
subject-matter acquisition. The next program-level step is to connect selected
plans to sandbox source ingestion and independent exercise validation while
retaining the same immutable promotion boundary.
