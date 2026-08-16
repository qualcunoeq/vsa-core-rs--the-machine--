# Stage J: route-blind multimodal visual composition

Every mixed input is offered to both visual frontends. The dispatcher does not
use modality or lexical route hints. Authorization requires exactly one
complete route and replayable downstream artifacts.

| Measure | Result |
|---|---:|
| Cases | 240 |
| Supported / ambiguous / unsupported | 120 / 40 / 80 |
| Exact decisions | 240/240 |
| Frontend invocations | 480 |
| Authorized supported | 120/120 |
| Ambiguities preserved | 40/40 |
| Unsupported refusals | 80/80 |
| Table replay | 240/240 |
| Graph replay | 240/240 |
| Downstream random-walk artifacts emitted | 60 |
| Downstream random-walk replay (emitted) | 60/60 |
| Visual frontend tamper receipts | 240/240 |
| Downstream tamper rejection (emitted) | 60/60 |
| False authorizations / denials | 0 / 0 |
| HLE questions read | 0 |
| Production registry mutations | 0 |

Graph random-walk execution requires explicit row-stochastic transitions and
an independently replayable finite probability distribution. Adjacency shape
alone cannot authorize stochastic semantics.

Reproduce with:

```text
cargo run --quiet --bin stage_j_multimodal_route_blind
```

Machine-readable report: `docs/stage_j_multimodal_route_blind.json`.
