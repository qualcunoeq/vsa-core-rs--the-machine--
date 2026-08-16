# Phase 71 — Bounded finite Markov-chain pack

This milestone extends the validated finite-probability substrate with an
explicit row-stochastic transition representation. The pack supports exact
one-step evolution, finite horizons of at most eight steps, and a unique
stationary distribution for a declared two-state chain. It refuses ambiguous
row/column conventions, non-normalized transitions, larger stationary solves,
continuous-time semantics, spectral shortcuts, and over-budget traces.

The independent corpus contains 240 cases:

* 120 supported artifacts (one-step, finite-horizon, and two-state stationary);
* 20 convention-ambiguous cases;
* 100 refused or invalid cases (non-unique/larger stationary requests,
  over-budget horizons, and invalid transitions).

Results: 240/240 exact decisions, 240/240 replay verification, 240/240 tamper
rejections, and zero false authorizations or denials. The corpus hash is
`6d1054335b376d96725677485e35015fdfeefee702fd2236a62d6a93c9a3162e`.

Run with:

```text
cargo run --quiet --bin finite_markov_pack_bench
```

The implementation is shadow-only; no live registry or HLE routing changes.
