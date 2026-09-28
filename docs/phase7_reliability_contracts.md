# Reliability contracts (Phase 7)

Earlier phases made the system observable. Phase 7 makes persistent use safe
for the system's own state and resources by turning written reliability claims
into checks the code enforces.

The guiding rule: **a guarantee the system reports must match what the active
configuration actually does.** A claim is either *assumed*, *checked*, or
*enforced*, and the report says which.

## Memory

`MemoryBudget` declares the capacities the theorems are stated against. The
insertion paths consult it and evict rather than grow past the cap:

* `add_to_dejavu_db` drains a cluster at the per-cluster entry cap and evicts
  the coldest cluster at the cluster cap, remapping cold storage and
  associations so indices stay valid.
* `add_transient_fact` enforces both the per-transient entry cap and the
  transient-cluster count cap.
* `novelty_gate_with_budget` is the budget-aware novelty gate.

`MemoryReport` accounts for the whole resident footprint — entry payloads,
label/metadata strings, dense accumulators, centroids and anchors, associations,
experiences, live indexer entries, and conversation state — replacing the
earlier snapshot that counted only a few terms and hard-coded several to zero.

Tests drive thousands of insertions against tight budgets and assert the caps
hold, rather than asserting a fixed fixture size.

## Replay taxonomy

One word, "replay", covered three different strengths. Phase 7 names them:

| Class | Proves | Example |
|---|---|---|
| `HashConsistency` | recorded bytes unchanged | receipt `replay_verified` hashes |
| `Recomputation` | same computation re-run agrees | `replay_linear_equation` |
| `IndependentVerification` | a different mechanism checks the answer | algebra residual substitution |

`replay_catalogue()` lists the mechanisms per class; the report emits counts so
a hash check is never presented as independent verification.

## Assumptions

`MATH.md §0.4` now carries an `Enforcement` column labelling each assumption
assumed / checked / enforced with the witness. The memory-boundedness theorems
(III.1, II.2) are runtime contracts, not asymptotic statements.

## Server

* loopback bind by default; non-loopback requires `--allow-remote` plus a token
* bearer-token auth on `/api/*` (constant-time comparison) when configured
* bounded request bodies and turn text length
* bounded concurrent in-flight turns (semaphore; 503 when busy)
* per-turn execution timeout with cooperative cancellation
* panic-safe active-turn cleanup

## Result

Measured by `cargo run --bin reliability_report`: 20,000 sustained
observations, 24 clusters, all caps respected, 2.65 MiB accounted against a
64 MiB budget; replay classes 4 / 9 / 5; 12 guarantees enforced, 2 checked, 0
assumed; projection path `cpu_soft_projection`. The binary exits non-zero if an
enforced guarantee is violated, so the report cannot claim more than the code
does.
