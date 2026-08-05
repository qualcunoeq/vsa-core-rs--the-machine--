# Phase 54 — Finite exact probability curriculum pack

Phase 54 adds a shadow-only, bounded probability domain to the curriculum. It
is deliberately finite and exact: probabilities are represented as normalized
integer rationals over an explicit finite sample space. The pack is not
registered with production routing.

## Boundary

Supported operations are distribution construction, complements, unions,
intersections, conditional probability, independence, total probability,
Bayes' rule, expectation, and variance. The implementation refuses continuous
or measure-theoretic probability, asymptotic results, stochastic-process
execution, unspecified sample spaces, inferred independence, invalid
normalization, zero conditioning events, and unsupported stochastic-matrix
execution.

Every result carries the OpenStax *Introductory Statistics 2e* source record,
provenance, assumptions, reasons, and a replay hash. Accepted probability
vectors may bridge only when they are degenerate integer vectors; fractional
vectors are intentionally left unconsumed by the current linear-algebra pack.

## Independent benchmark

The generated corpus contains 240 cases:

| class | cases |
| --- | ---: |
| supported | 120 |
| boundary (missing, ambiguous, or zero-conditioning) | 40 |
| unsupported | 80 |

Results from `probability_pack_bench`:

* 240/240 exact status decisions;
* 120/120 exact supported artifacts;
* 240/240 replay receipts verified;
* 240/240 tamper attempts rejected;
* 10/10 degenerate probability-vector bridges accepted;
* 10/10 fractional/invalid bridge attempts refused;
* 0 false authorizations and 0 false denials;
* 5 rewrite groups stable;
* no supported-artifact mismatch families.

The corpus hash and per-case receipts are recorded in
[`phase54_probability_pack_bench.json`](phase54_probability_pack_bench.json).

## Governance

The pack is shadow curriculum infrastructure only. It does not mutate the
registry, router, or production authorization path. Continuous distributions,
measure-theoretic claims, asymptotic limits, and full Markov-chain semantics
remain separate future curriculum items.
