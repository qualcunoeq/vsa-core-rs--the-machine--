# Stage AB — retrieval-guided epistemic investigation

Stage AB connects immutable source retrieval to the epistemic investigator.
Retrieved claims remain claims: they can update a hypothesis only when the
query is exact, the result is replay-valid, and at least two independent
upstream lineages corroborate the same object.

The 500-case corpus contains:

* 200 corroborated claims that may authorize a supported update;
* 100 copied-report cases whose source IDs differ but lineage does not;
* 100 conflicting claims;
* 100 missing claims.

Run:

```text
cargo run --quiet --bin stage_ab_retrieval_investigation
```

The machine-readable report is `docs/stage_ab_retrieval_investigation.json`.

| Metric | Result |
| --- | ---: |
| Cases | 500 |
| Correct lifecycle decisions | 500/500 |
| Corroborated claims authorized | 200 |
| Copied claims refused | 100/100 |
| Conflicts refused | 100/100 |
| Missing claims refused | 100/100 |
| Ambiguity preserved | 300/300 |
| Retrieval and belief replay | 500/500 each |
| Tamper rejections | 500/500 |
| False authorizations / denials | 0 / 0 |
| Registry/world-model mutations | 0 / 0 |

The campaign specifically verifies that three reports inherited from one
upstream lineage do not count as three independent confirmations. No live
registry, fact store, or world model is mutated.
