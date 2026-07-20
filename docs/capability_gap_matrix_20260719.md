# HLE executable-overlap synthesis

| Domain slice | Prompts | Explicit executable after extraction | Modeling/theorem or unsupported representation |
|---|---:|---:|---:|
| Mechanics-adjacent | 40 | 0 | 40 |
| Tier-A algebra candidates | 33 | 0 | 33 |
| Finite mathematics | 215 | 0 | 215 |
| Number theory | 182 | 0 | 182 |
| Calculus | 181 | 0 | 181 |

The repeated result is decisive: arithmetic executors are not the current
benchmark bottleneck. HLE prompts overwhelmingly require selecting or
constructing a formal method before execution. The next architecture should
therefore generalize the typed, provenance-preserving method registry rather
than add another standalone calculator.

Existing reusable substrate:

```text
typed problem extraction
→ method premises/side conditions
→ authorized execution
→ exact algebra/numeric backend
→ independent replay and completeness receipt
```

Next implementation target: a narrow mathematical-method retrieval family
selected from recurring modeling/theorem clusters, with provenance and an
entailment gate. Do not rerun full HLE until that family has a verified,
non-empty benchmark overlap.
