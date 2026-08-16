# Stage AA — governed promotion and rollback

Stage Z validated five answer-key-blind learning proposals in a sandbox but
kept promotion disabled. This stage consumes only those immutable validation
receipts and exercises their complete lifecycle in cloned registries:

```text
sandbox-validated proposal
→ policy/dependency/migration checks
→ staged promotion in clone
→ receipt replay and tamper check
→ induced rollback
→ historical active-version replay
```

The 300 independently authored lifecycle cases cover clean promotion, policy
denial, regression blocking, dependency conflict, migration failure,
competing semantic boundaries, rollback with accumulated world-state hashes,
and historical replay. No production registry or curriculum manifest is
opened for mutation.

Run:

```text
cargo run --quiet --bin stage_aa_governed_promotion
```

The machine-readable report is
`docs/stage_aa_governed_promotion.json`.

| Metric | Result |
| --- | ---: |
| Cases | 300 |
| Source-validated modules | 5 |
| Exact lifecycle decisions | 300/300 |
| Staged promotions | 100 |
| Blocked or denied proposals | 200 |
| Rollback proposals / applied | 60 / 60 |
| World-state hashes preserved | 60/60 |
| Historical replays | 60/60 |
| Staged registry replays | 300/300 |
| Promotion receipt replays | 300/300 |
| Tamper rejections | 300/300 |
| False authorizations / denials | 0 / 0 |
| Production registry mutations | 0 |
| Curriculum manifest mutations | 0 |

Promotion remains a cloned-registry operation. A sandbox-valid proposal is not
silently promoted into live routing; policy, dependency, migration, and
boundary checks remain explicit.
