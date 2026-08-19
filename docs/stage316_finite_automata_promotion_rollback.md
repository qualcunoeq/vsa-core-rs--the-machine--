# Stage 316 — finite-automata promotion and rollback

- Cases: 240
- Source preflight: 240/240
- Exact lifecycle decisions: 240/240
- Promotions / blocked or denied: 100 / 140
- Registry replays / tamper rejections: 240 / 240
- Rollbacks applied: 60
- World state preserved / historical replay: 60 / 60
- False authorizations / denials: 0 / 0
- Production registry mutations / curriculum mutations: 0 / 0

The source-derived automata capability is evaluated only in cloned registries. Clean promotion, policy denial, regression blocking, dependency conflict, migration failure, competing boundaries, accumulated-state rollback, and historical replay are all explicit lifecycle cases.
