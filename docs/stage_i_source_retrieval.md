# Stage I: governed source retrieval

This shadow-only layer retrieves immutable source claims by exact subject,
predicate, domain, and scope. It preserves source lineage, distinguishes
independent corroboration from copied evidence, and refuses conflicts or
missing claims. Retrieved claims remain artifacts; no live registry or fact
store is mutated and no result is authorized merely because retrieval found a
candidate.

The independent campaign contains 240 cases: 120 corroborated claims, 40
conflicting claims, and 80 missing/refused queries. The generated report is
`docs/stage_i_source_retrieval.json`.
