# Stage 268 — economics promotion and rollback

Clone-only lifecycle test for the utility-selected economics candidate.

* decisions: 240 / 240 exact
* promotions: 100
* blocked: 100
* regressions / rollbacks: 40 / 40
* promotion replay / tamper: 240 / 240
* historical replay / parent preservation: 240 / 240
* clone-only cases: 240
* false authorizations / denials: 0 / 0
* live manifest / registry mutations: 0 / 0

No production curriculum or route was mutated.

Reproduce with `cargo run --quiet --bin stage268_economics_promotion_rollback`.
