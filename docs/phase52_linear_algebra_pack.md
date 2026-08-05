# Phase 52 — Finite-dimensional linear algebra curriculum pack

This is the first breadth-first curriculum pack after the Phase 51 manifest.
It is shadow-only and covers exact finite-dimensional integer inputs. The pack
does not register with production routing or answer HLE questions.

Source provenance is anchored to MIT OpenCourseWare 18.06SC Linear Algebra;
the pack stores the source citation on every result.

## Supported foundation

The bounded operations are:

* vector and matrix construction;
* exact rank and nullity;
* exact determinant and invertibility;
* bounded exact row reduction to rational RREF;
* diagonal-matrix eigenvalue extraction;
* exact inner products and orthogonality.

The implementation uses integer-exact determinant/minor rank, rational RREF,
and explicit dimension checks. It refuses symbolic domains, non-diagonal
eigenvalue problems, infinite-dimensional operators, malformed matrices, and
matrices outside the configured size budget.

## Independent pressure corpus

The frozen corpus contains 240 cases:

* 120 supported exact operations;
* 30 missing or dimension-boundary cases;
* 90 unsupported or out-of-domain cases;
* 10 rewrite groups.

The generated artifact is
`docs/phase52_linear_algebra_pack_bench.json`.

Results:

* 240/240 exact status decisions;
* 120/120 supported artifacts exact;
* 240/240 replay receipts verified;
* 240/240 tampered receipts rejected;
* 0 false authorizations;
* 0 false denials.

The corpus is a curriculum validation artifact, not an HLE development set.
The HLE holdout remains untouched and production routing remains unchanged.

## Shadow integration

Forty exact scalar artifacts (rank and determinant) were passed through the
existing independent solution-verification receipt path. Twenty non-diagonal
spectral cases were refused before that bridge. The integration report is
`docs/phase52_linear_algebra_shadow_integration.json`.

* 60/60 shadow cases classified;
* 40/40 scalar bridge receipts replayed;
* 20/20 unsupported spectral cases safely refused;
* 0 production authorizations.

The curriculum manifest now marks `linear_algebra_spectral` as
`shadow_validated`, while the HLE checkpoint remains deferred.

## Gate status

This phase establishes the representation and direct-computation level of the
linear-algebra domain. It does not claim theorem applicability, symbolic
parameter solving, spectral decomposition beyond diagonal matrices, or
cross-domain promotion. Those require later independent corpora and gates.

Run the benchmark with:

```text
cargo run --quiet --bin linear_algebra_pack_bench
```
