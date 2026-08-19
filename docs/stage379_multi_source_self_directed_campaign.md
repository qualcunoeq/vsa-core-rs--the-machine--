# Stage 379 — multi-source self-directed campaign

- preflight verified: true
- source library / validated / rejected: 4 / 3 / 1
- validation receipts / replays: 4 / 4
- initial / resolved / remaining gaps: 4 / 3 / 1
- campaign rounds / selected rounds / selected modules: 4 / 3 / 3
- campaign replay / manifest unchanged: true / true
- false authorizations / live registry mutations: 0 / 0
- corpus SHA-256: `cde140bedbcb764f7cee46b6f80822a5a5541ab176185505e862923edefb5855`

The planner receives three validated source modules plus one invalid candidate and one unavailable residual. It admits only replay-valid candidates, selects all three by exact artifact coverage, leaves the unavailable specialist gap unresolved, and preserves the curriculum manifest. This is a controlled self-directed campaign, not evidence of broad external-exam transfer.

Reproduce with `cargo run --quiet --bin stage379_multi_source_self_directed_campaign`.
Machine-readable report: `docs/stage379_multi_source_self_directed_campaign.json`
