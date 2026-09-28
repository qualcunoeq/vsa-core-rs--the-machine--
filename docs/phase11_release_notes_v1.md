# The Machine v3.4 — release notes

> phase11-release-notes-v1 · crate 0.1.0 · persistence schema 2 · projection cpu_soft_projection

Packaged 2026-09-28.

Start with `cargo run --release --bin machine -- start`. The full operator reference is `docs/DEVELOPMENT.md`.

## Highlights

* One documented startup command (`machine start`) replaces research-specific commands.
* Configuration is validated with a remedy for every problem.
* `machine doctor` reports health and dependency status, including schema compatibility.
* Backup, restore, upgrade, and recover are first-class and safe to run unattended.
* The capability inventory is versioned and tied to the claim ledger and evaluation artifacts.

## Capabilities

* active: 2
* deferred: 2
* experimental: 1
* shadow: 2
* stable: 12

See `docs/phase11_capability_inventory_v1.json` for the versioned inventory.

## Evaluation results

| area | result | artifact |
|---|---|---|
| Reliability contracts | 20000 sustained observations, memory within budget: true; 15 guarantees enforced, 2 checked; projection cpu_soft_projection | `docs/phase7_reliability_v1.report.json` |
| Conversation evaluation | regression 18/28 answered (coverage 0.643), holdout coverage 0.769, drift 0, 6 ablations | `docs/phase9_conversation_eval_v1.report.json` |
| Semantic fidelity | 1/8 faithful, silent wrong answers 0 (limit 0), downstream authorizations 0 | `docs/semantic_fidelity_eval_v1.report.json` |
| Controlled autonomy | 6 scenarios, 5 completed, 1 controlled refusal, contracts satisfied: true | `docs/phase10_autonomy_v1.report.json` |

Regenerate each artifact with the command named in the phase document (`docs/DEVELOPMENT.md`).

## Upgrading

* Back up before upgrading; `machine upgrade` takes a timestamped backup automatically.
* A database newer than the build is refused, never misread.
* If an upgrade fails, `machine recover` restores the most recent backup and verifies it.
