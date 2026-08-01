# Phase 30 — HLE equation and scientific-law audit

Phase 29 found no reusable method family in the 222 missing-method cases.
Phase 30 therefore audits the larger pool previously classified as
`missing_equation_or_scientific_law`, looking for repeated structured
knowledge and reusable bridge primitives rather than whole-problem solvers.

```text
HLE question
→ law/equation cues
→ variables, units, assumptions
→ retrieval sufficiency
→ typed output and bridge primitives
→ repeated law-family evidence
```

## Result

| Metric | Result |
|---|---:|
| Law/equation cases | 138 |
| Retrieval-ready equation candidates | 12 |
| Equations stated or embedded in-question | 34 |
| Missing prerequisites | 41 |
| Specialist singletons | 51 |
| Repeated non-generic law families | 8 |

The largest broad bucket is intentionally `other_specialist` (65 cases), but
it is not treated as a reusable family. The repeated named families are:

* algebraic identity (29);
* probability formula (10);
* thermodynamics (9);
* quantum physics (6);
* electromagnetism (5);
* reaction stoichiometry (4);
* mechanics (3);
* population genetics (3).

These counts justify independent corpus construction, not immediate retrieval
or authorization. A family still needs repeated law cues, compatible output
artifacts, explicit validity conditions, and an untouched HLE holdout.

## Bridge primitives

The audit records reusable lower-level bridges rather than claiming a domain
solver:

* `named_law_lookup` — 104 cases;
* `equation_binding` — 34 cases.

Each case also records variables, units, assumptions, requested output,
whether the law is stated in-question, retrieval sufficiency, and the nearest
existing capability. No source retrieval or answer authorization occurs.

## Provenance and reproduction

The input is the Phase 22/23 knowledge-audit artifact generated from the
frozen HLE dataset. The regenerated report is checked in as
`docs/phase30_hle_law_audit.json` with SHA-256
`9fbe52a26b378c16e858bca75ca2835b5339aae5c31602e068b446205956c0ed`.

```text
cargo test --bin hle_law_audit
cargo run --bin hle_law_audit -- \
  /tmp/hle_knowledge_audit_2147e9e.json \
  /tmp/hle_law_audit_2147e9e.json
```

The release-trace regeneration limitation from the Phase 27 manifest still
applies: aggregate counts are reproduced, while the unavailable historical
trace is not claimed byte-identical.

## Next gate

Select one repeated family only after manually checking its case IDs. Build an
external positive/ambiguous/unsupported law corpus with source provenance,
validity domains, units, and disagreement handling. Then test a shadow typed
law artifact against the untouched HLE cases. Do not promote a broad
“scientific-law solver.”
