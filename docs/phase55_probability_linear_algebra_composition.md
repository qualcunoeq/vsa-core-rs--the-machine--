# Phase 55 — Probability/linear-algebra composition

Phase 55 pressure-tests the boundary between the two shadow-validated
curriculum packs. It is diagnostic-only: no production route, registry entry,
or HLE behavior changes.

## Composition policy

Numeric vectors and matrices are not treated as probabilities by shape alone.
A route is accepted only when both packs independently verify the artifact and
the bridge carries explicit probability semantics. The benchmark covers:

* degenerate finite probability distributions to validated integer vectors;
* explicitly normalized integer vectors to finite distributions;
* exact expectation values agreeing with an independently replayed integer dot
  product;
* finite joint distributions reduced to explicitly grouped marginals;
* fractional distributions, signed weights, and unnormalized weights refused
  at the probability boundary;
* row/column stochastic ambiguity preserved;
* matrices with probability-looking entries refused without stochastic
  semantics;
* covariance-like matrices refused because covariance/statistical semantics
  are not yet part of the curriculum.

## Results

The independent corpus contains 120 cases: 60 authorized routes and 60 safe
refusals.

* 120/120 exact route decisions;
* 120/120 source/composition replay receipts;
* 80 target linear-algebra artifact replays (the remaining refusals correctly
  stop before a target artifact exists);
* 60/60 safe composition refusals;
* 120/120 tamper attempts rejected;
* 0 route leakage;
* 0 false authorizations and 0 false denials;
* 3 rewrite groups stable.

The per-case corpus, route trace, statuses, and hash are recorded in
[`phase55_probability_linear_algebra_composition.json`](phase55_probability_linear_algebra_composition.json).

## Interpretation

This validates cumulative curriculum infrastructure rather than a stochastic
matrix or covariance capability. Fractional probability vectors remain
unconsumed by the current integer-only linear-algebra pack, and valid matrix
entries do not imply stochastic semantics. Those are intentionally deferred
curriculum items.
