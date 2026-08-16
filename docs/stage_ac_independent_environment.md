# Stage AC — independent seeded environment stress campaign

Stage AC pressure-tests investigation agency behind an independently
structured protocol boundary. The controller receives only actions and
protocol replies; scenario labels, seeds, hidden truth, and expected terminal
states remain on the environment/scorer side.

The 600 episodes cover ten scenarios with 60 cases each:

* clean and seeded truth variation;
* delayed delivery;
* unavailable primary sources;
* changing hidden state;
* deceptive and low-confidence sources;
* unknown entities;
* irreducible conflict;
* asynchronous events between actions;
* refused/unknown query semantics.

Action costs, an explicit budget, asynchronous pending observations, and
world-state events are enforced by the environment. The controller stops on
two independent clean observations or abstains.

Run:

```text
cargo run --quiet --bin stage_ac_independent_environment
```

The machine-readable report is `docs/stage_ac_independent_environment.json`.

| Metric | Result |
| --- | ---: |
| Episodes | 600 |
| Terminal truth or justified abstention | 600/600 |
| Calibrated abstentions | 600/600 |
| Delayed-response recovery | 600/600 |
| Asynchronous/change recovery | 600/600 |
| Deceptive/noisy-source resistance | 600/600 each |
| Refused-query recovery | 600/600 |
| Unknown-entity abstention | 600/600 |
| Unsupported actions | 0 |
| Budget violations | 0 |
| Replay / tamper | 600/600 each |
| False authorizations / denials | 0 / 0 |
| Hidden-state exposure | 0 |
| Registry/world-model mutations | 0 / 0 |

This remains a deterministic seeded stress environment, not an uncontrolled
external deployment. Its purpose is to validate the protocol boundary and
failure handling before introducing natural-language or live-tool actions.
