# Stage F: automatic prerequisite discovery

The curriculum DAG now has a shadow planner that maps requested typed
artifacts to owning packs, computes transitive prerequisite closure, and tests
candidate dependency edges for cycles. Unknown artifacts remain residuals;
cycle proposals are rejected. The source manifest is never mutated.

The 300-case campaign is recorded in
`docs/stage_f_prerequisite_discovery.json`.

| result | cases |
| --- | ---: |
| complete prerequisite plans | 240 |
| unknown-artifact residuals | 30 |
| cycle proposals rejected | 30 |
| exact decisions | 300/300 |
| manifest immutable | true |

This is a planning and diagnosis layer. It proposes curriculum order but does
not promote packs, alter routing, or authorize execution.
