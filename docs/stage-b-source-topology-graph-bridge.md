# Stage B — finite topology to graph composition

The source-derived topology domain now contributes a typed cross-domain
artifact. With an explicit `strict_specialization_graph` policy, a validated
finite topology is lowered to the loop-free directed graph of its strict
specialization relation. The carrier ordering and source provenance are
preserved; reflexive preorder edges are intentionally omitted because the
graph pack is loop-free.

No graph semantics are inferred from a topology by default. Missing policy,
invalid topologies, and unresolved topology orientation remain refused.

| metric | result |
| --- | ---: |
| cases | 240 |
| supported compositions | 120 |
| ambiguous policy cases | 40 |
| refused cases | 80 |
| exact decisions | 240/240 |
| authorized compositions | 120/120 |
| topology/bridge replay | 240/240 |
| graph replay | 240/240 |
| tamper rejection | 240/240 |
| false authorizations | 0 |
| route leakage | 0 |

The machine-readable report is
[`stage-b-source-topology-graph-bridge.json`](stage-b-source-topology-graph-bridge.json).
