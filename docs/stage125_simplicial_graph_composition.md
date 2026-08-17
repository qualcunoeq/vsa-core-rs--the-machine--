# Stage 125 — simplicial-complex to graph composition

The bounded homology substrate now composes with the finite graph pack through
an explicit `one_skeleton_graph` policy.  A simplex is never implicitly
treated as a graph: only its declared one-simplices become undirected graph
edges, while higher-dimensional faces remain in the homology artifact.

| Measure | Result |
|---|---:|
| Cases | 240 |
| Supported / ambiguous / refused | 120 / 40 / 80 |
| Exact route decisions | 240/240 |
| Authorized routes | 120/120 |
| Replay verified | 240/240 |
| Tamper rejected | 240/240 |
| False authorizations / denials | 0 / 0 |

Refusals cover invalid complexes and unsupported coefficient fields.  Missing
bridge policy remains ambiguous.  Vertex identity and order are preserved,
and no graph artifact authorizes higher-dimensional topology claims.

Reproduce with:

```text
cargo run --quiet --bin stage125_simplicial_graph_composition
```

Machine-readable report: `docs/stage125_simplicial_graph_composition.json`.
