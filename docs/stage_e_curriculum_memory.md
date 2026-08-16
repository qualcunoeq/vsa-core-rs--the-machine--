# Stage E: curriculum-scale memory

The curriculum now has an append-only memory substrate for learned artifacts
and receipts. Records are segmented at 256 entries, indexed by domain, and
retain immutable provenance and content hashes. Memory stores artifacts; it
does not make them executable or promote them into live routing.

The scale campaign is recorded in `docs/stage_e_curriculum_memory.json`.

| metric | result |
| --- | ---: |
| records appended | 100,000 |
| segments | 391 (capacity 256) |
| balanced domain retrieval | 25,000 each across 4 domains |
| exact domain/artifact retrieval | 25,000 |
| exact retrieval contamination | 0 |
| empty exact query | 0 results |
| exact version `v1` retrieval | 8,333 |
| version contamination | 0 |
| explicit stale `v0` retrieval | 8,334 |
| replay verified | 100,000/100,000 |
| tamper rejected | 100,000/100,000 |
| duplicate IDs rejected | 1 |
| invalid pre-hashed records rejected | 1 |
| live mutation | false |

The memory layer is deterministic and bounded at the segment level. Exact
domain/artifact/version retrieval now prevents broad-domain and stale-version
contamination before a typed planner consumes a record. Historical versions
remain explicitly retrievable; no version is silently promoted. Future
curriculum work can add retention, prerequisite indexing, and ranking without
weakening append-only history or replay.
