# Phase 41 — Global HLE typed-target census

Phase 41 combines the available frozen HLE audit artifacts and clusters by
requested output artifact plus exact transformation signature, independent of
academic subject labels. It is a census only: no capability contract,
external knowledge pack, execution route, or promotion is created.

## Reproducibility

```text
cargo run --quiet --bin hle_global_target_census -- \
  docs/phase41_hle_global_target_census.json
```

Inputs are the Phase 29 method audit, Phase 30 law audit, and Phase 40
mechanics-target audit. The HLE dataset and all input report hashes are stored
in the machine-readable output.

| Artifact | SHA-256 |
| --- | --- |
| HLE dataset | `31b26cc8e352af16bedb9a714feb788ae562be38898ab92dc54b4665882bf1c6` |
| Phase 41 report | `b6c0109d1ca634d3e485af175ba7f1c288d258ca8ed759615255a14d997ea420` |

## Materialized census

| Pool | Requested | Materialized |
| --- | ---: | ---: |
| Missing-method cases | 222 | 222 |
| In-question equations | 34 | 34 |
| Representation bridges | 29 | subset of method cases |
| Mechanics target residuals | 152 | 152 |
| Derivation after retrieval | 189 | 0 |

The derivation-after-retrieval category is preserved only as an aggregate in
the Phase 21 document; no per-question immutable artifact was available. The
census records that limitation rather than fabricating 189 records.

The combined materialized set contains **393 unique case IDs** after 15
overlaps are collapsed.

## Candidate repeated typed transformations

These are candidates for external validation, not justified capabilities:

| Output artifact | Transformation signature | Cases |
| --- | --- | ---: |
| scalar/structured answer | matrix rank or determinant | 17 |
| scalar/structured answer | probability distribution | 14 |
| scalar/structured value | equation binding | 11 |
| scalar/structured answer | calculus transformation | 10 |
| scalar/structured answer | scientific-law application | 10 |
| scalar/structured answer | geometric construction | 9 |
| scalar/structured answer | recurrence evaluation | 8 |

The census deliberately excludes generic-specialist, target-grounding,
ambiguous, and unclassified groups from candidate status. A repeated
signature still requires semantic review, an independent external corpus,
explicit prerequisites and invariants, and a frozen holdout before any
contract can be proposed.

## Residuals and interpretation

There are **74 unresolved output residuals** in the materialized records. They
are retained as residual evidence rather than forced into an `OtherTarget`
ontology. The census therefore supports a measured next step: select one of
the exact repeated signatures—matrix rank/determinant is the largest—and test
whether it forms a real reusable family outside HLE.

This phase does not claim that any candidate family is coherent enough for
implementation. It establishes the global target-overlap shortlist while
preserving the zero-authorization boundary and leaving production routing
unchanged.
