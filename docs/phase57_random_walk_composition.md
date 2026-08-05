# Phase 57 — One-step graph/probability/linear-algebra composition

Phase 57 adds a shadow-only one-step transition layer. It does not change the
finite probability pack's refusal of generic stochastic-matrix execution. A
transition is accepted only when graph topology, vertex identity/order,
probability normalization, matrix convention, and transition semantics are
all explicit.

## Boundary

Supported routes include uniform-neighbor walks on finite graphs, explicit row
and column stochastic conventions, exact initial distributions, and one-step
next-distribution artifacts. Adjacency matrices are not transition matrices
without a declared random-walk rule. Zero-degree vertices require an explicit
policy and are refused here.

The phase refuses signed or non-normalized weights, vertex-order mismatches,
weighted graphs, row/column ambiguity, multi-step walks, stationary or spectral
claims, and adjacency-shaped matrices without stochastic semantics.

## Independent benchmark

The corpus contains 240 cases:

| class | cases |
| --- | ---: |
| authorized one-step compositions | 120 |
| safe refusals | 120 |

Results from `random_walk_composition_bench`:

* 240/240 exact terminal decisions and artifacts;
* 120/120 successful three-domain compositions;
* 240/240 graph replay receipts;
* 240/240 probability replay receipts;
* 230 linear-algebra replay receipts (the remaining cases refuse before a
  matrix artifact exists);
* 215 one-step/refusal replay receipts;
* 240/240 tamper attempts rejected;
* 120 safe refusals;
* 0 false authorizations or denials;
* 0 route leakage;
* 4 rewrite groups retained.

The corpus hash and per-case route traces are recorded in
[`phase57_random_walk_composition.json`](phase57_random_walk_composition.json).

## Interpretation

This validates only representation and one-step transition composition. It
does not authorize general stochastic processes, multi-step walks, stationary
distributions, mixing times, graph limits, or spectral claims.
