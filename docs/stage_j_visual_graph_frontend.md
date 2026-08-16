# Stage J: governed visual graph frontend

This milestone adds a second multimodal route beyond visual tables. A
coordinate-bearing visual extractor may provide explicit vertex identities,
edge endpoints, direction policy, confidence, and provenance. The frontend
formalizes those observations only when they establish a bounded finite simple
graph, then delegates to the existing graph pack.

It does not infer graph semantics from geometry, proximity, unlabeled line
segments, or a square-looking visual layout. Ambiguous direction, unknown edge
endpoints, duplicate/self-loop edges, unsupported graph labels, and weak
observations remain closed.

| Measure | Result |
|---|---:|
| Cases | 240 |
| Supported / ambiguous / refused | 120 / 40 / 80 |
| Exact decisions | 240/240 |
| Supported authorizations | 120/120 |
| Visual replay | 240/240 |
| Visual tamper rejection | 240/240 |
| Graph bridge artifacts emitted | 120/120 |
| Graph replay (emitted) | 120/120 |
| Graph tamper rejection (emitted) | 120/120 |
| False authorizations / denials | 0 / 0 |

The graph handoff preserves vertex ordering and coordinate provenance. It is a
shadow-only route and does not alter production routing or curriculum status.

Reproduce with:

```text
cargo run --quiet --bin visual_graph_frontend_bench
```

Machine-readable report: `docs/stage_j_visual_graph_frontend.json`.
