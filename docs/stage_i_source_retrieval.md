# Stage I: governed source retrieval

This shadow-only layer retrieves immutable source claims by exact subject,
predicate, domain, and scope. It preserves source lineage, distinguishes
independent corroboration from copied evidence, and refuses conflicts or
missing claims. Retrieved claims remain artifacts; no live registry or fact
store is mutated and no result is authorized merely because retrieval found a
candidate.

Source IDs remain visible for provenance, but corroboration-sensitive consumers
must use deduplicated upstream lineage IDs. Three reports copied from one
source therefore count as one lineage, not three independent confirmations.

The independent campaign contains 240 cases: 120 corroborated claims, 40
conflicting claims, and 80 missing/refused queries. The generated report is
`docs/stage_i_source_retrieval.json`.

The supported corpus verifies lineage deduplication in 120/120 cases while
preserving complete replay and tamper receipts.
