# Stage X — route-blind multimodal curriculum composition (1,000 cases)

This benchmark scales the visual-table and visual-graph frontends into a
route-blind curriculum gate. Every case is offered to both frontends, and a
route authorizes only when its typed artifact and downstream replay are valid.
The graph route composes with one-step finite random-walk execution; the table
route composes with the explicitly labelled finite probability bridge.

| Measure | Result |
|---|---:|
| Cases | 1,000 |
| Supported / ambiguous / unsupported | 600 / 200 / 200 |
| Exact decisions | 1,000/1,000 |
| Frontend invocations | 2,000 |
| Authorized supported | 600/600 |
| Ambiguities preserved | 200/200 |
| Unsupported refusals | 200/200 |
| Table replay | 1,000/1,000 |
| Graph replay | 1,000/1,000 |
| Downstream artifacts emitted | 300 |
| Downstream replay (emitted) | 300/300 |
| Visual frontend tamper rejection | 1,000/1,000 |
| Downstream tamper rejection (emitted) | 300/300 |
| False authorizations / denials | 0 / 0 |
| HLE questions read | 0 |
| Production registry mutations | 0 |

The generated report is
`docs/stage_x_multimodal_curriculum_1000.json`. Reproduce with:

```text
cargo run --quiet --bin stage_x_multimodal_curriculum_1000
```
