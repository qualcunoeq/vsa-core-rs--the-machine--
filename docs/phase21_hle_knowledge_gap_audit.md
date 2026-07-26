# Phase 21 — HLE missing-knowledge audit

This phase audits only the 1,842 questions classified as
`missing_factual_knowledge` by the frozen Phase 20 trace. It does not retrieve
documents, change the router, or promote any claim. The classifier is a
deterministic lexical shadow taxonomy intended to create a human-review queue.
Its output is not treated as ground-truth annotation.

Run:

```text
cargo run --bin hle_knowledge_audit -- \
  /tmp/hle_release_candidate_2147e9e.traces.jsonl \
  /tmp/hle_knowledge_audit_2147e9e.json
```

Input trace SHA-256: `3b0cc1aac3819b8f41343f21cd02ff8c54b0e5b3be1f99ce9164b5c6a2cb2348`

## Shadow taxonomy

| Candidate gap family | Cases |
|---|---:|
| Apparent gap from normalization | 338 |
| Missing empirical fact | 315 |
| Missing taxonomic fact | 235 |
| Missing historical or textual knowledge | 229 |
| Missing specialist convention | 233 |
| Derivation after factual retrieval | 189 |
| Missing equation or scientific law | 138 |
| Missing definition or terminology | 32 |
| Missing named theorem | 18 |
| Needs manual review | 115 |
| **Total** | **1,842** |

The largest apparent cluster is normalization contamination (338), followed
by empirical and taxonomic facts. This supports repairing normalization before
bulk knowledge acquisition: some apparent “knowledge” failures may already
contain enough information once notation, references, or answer formats are
formalized correctly.

## Interpretation and safeguards

The audit deliberately uses confidence labels and preserves five representative
samples per family. Several low-confidence lexical matches are visibly false
positives—for example, a theorem question containing a mathematical
“defined” clause can land in the definition bucket. Those rows remain review
items and cannot create a source query, claim, fact, or capability proposal by
themselves.

The next step is manual adjudication of the largest and most contaminated
families, followed by independently sourced claims with provenance, validity
conditions, corroboration, disagreement tracking, and replay. No knowledge
layer was mutated by this phase.
