# Phase 58 — Bounded finite-step random walks

Phase 58 extends the Phase 57 one-step composition to a fixed, exact budget
of one through eight steps. Each step is executed through the validated
one-step layer, normalized by the finite-probability pack, and retained in an
immutable trace. No spectral shortcut or stationary inference is available.

## Boundary

Supported inputs require a fixed graph, explicit vertex order, explicit row or
column convention, a declared transition matrix, and a normalized finite
initial distribution. Every intermediate distribution must replay and remain
normalized.

The layer refuses zero-step requests, steps beyond eight, time-varying
transitions, adjacency matrices without transition semantics, vertex-order
drift, signed or non-normalized transitions, zero-degree graphs without a
policy, and stationary or spectral claims.

## Independent benchmark

The corpus contains 240 cases: 120 supported traces (1, 2, 4, and 8 steps)
and 120 refusal/defect cases.

Results from `random_walk_composition_bench`:

* 240/240 exact terminal decisions and artifacts;
* 120/120 successful bounded traces;
* 120/120 successful three-domain replays;
* 240/240 graph and probability source replays;
* 230 linear-algebra replays (all emitted artifacts);
* 215 bounded-step/refusal receipts (all emitted receipts);
* 450/450 intermediate trace entries replayed on authorized traces;
* 525/525 emitted intermediate trace entries replayed across all routes;
* 240/240 tamper attempts rejected;
* 120 safe refusals;
* 0 false authorizations or denials;
* 0 route leakage;
* 4 rewrite groups retained.

The corpus hash and per-case traces are recorded in
[`phase58_bounded_random_walk.json`](phase58_bounded_random_walk.json).

## Interpretation

This validates bounded temporal depth only. Multi-step limits beyond eight,
time-inhomogeneous chains, stationary distributions, hitting times, mixing,
and spectral analysis remain separate curriculum items.
