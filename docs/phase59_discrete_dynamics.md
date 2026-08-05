# Phase 59 — Bounded discrete dynamics

Phase 59 adds an exact finite-horizon dynamics layer. It covers scalar affine
recurrences, rational vector recurrences, and matrix-driven vector evolution.
Every update emits a trace entry; no closed form, spectral shortcut, or
infinite-horizon conclusion is inferred.

## Boundary

Supported requests use explicit initial values, exact rational coefficients,
typed matrices, and a horizon of at most eight steps. The layer refuses
asymptotic stability claims, nonlinear recurrences, continuous-time systems,
over-budget or infinite horizons, spectral closed forms, dimension mismatches,
symbolic parameters, and floating-point approximations presented as exact.

## Independent benchmark

The corpus contains 240 cases: 120 supported finite-horizon evolutions and 120
boundary or unsupported requests.

Results from `discrete_dynamics_bench`:

* 240/240 exact terminal decisions and artifacts;
* 120/120 supported artifacts exact;
* 240/240 replay receipts verified;
* 450/450 authorized intermediate trace entries replayed;
* 450/450 emitted trace entries replayed;
* 120 safe refusals;
* 240/240 tamper attempts rejected;
* 0 false authorizations or denials.

The corpus hash and per-case trace records are recorded in
[`phase59_discrete_dynamics_bench.json`](phase59_discrete_dynamics_bench.json).

## Interpretation

This is finite temporal evolution only. Nonlinear systems, continuous time,
asymptotic stability, closed forms requiring unsupported spectral theory, and
infinite-horizon convergence remain separate curriculum items.
