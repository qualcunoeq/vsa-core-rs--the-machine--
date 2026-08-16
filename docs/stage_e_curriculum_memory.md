# Stage E: curriculum-scale memory

The curriculum now has an append-only memory substrate for learned artifacts
and receipts. Records are segmented at 256 entries, indexed by domain, and
retain immutable provenance and content hashes. Memory stores artifacts; it
does not make them executable or promote them into live routing.

The scale campaign is recorded in `docs/stage_e_curriculum_memory.json`.

| metric | result |
| --- | ---: |
| records appended | 10,000 |
| segments | 40 (capacity 256) |
| balanced domain retrieval | 2,500 each across 4 domains |
| exact domain/artifact retrieval | 2,500 |
| exact retrieval contamination | 0 |
| empty exact query | 0 results |
| replay verified | 10,000/10,000 |
| tamper rejected | 10,000/10,000 |
| duplicate IDs rejected | 1 |
| invalid pre-hashed records rejected | 1 |
| live mutation | false |

The memory layer is deterministic and bounded at the segment level. Exact
domain/artifact retrieval now prevents broad-domain contamination before a
typed planner consumes a record. Future curriculum work can add retention,
version selection, prerequisite indexing, and ranking without weakening
append-only history or replay.
