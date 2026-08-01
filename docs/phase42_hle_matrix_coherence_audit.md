# Phase 42 — Matrix candidate coherence audit

Phase 42 audits the **17 scalar-output** HLE cases behind the Phase 41
`matrix_rank_or_determinant` census candidate. It does not implement a matrix
capability or use HLE questions as training data.

## Reproducibility

```text
cargo run --quiet --bin hle_matrix_coherence_audit -- \
  docs/phase42_hle_matrix_coherence_audit.json
```

| Artifact | SHA-256 |
| --- | --- |
| HLE dataset | `31b26cc8e352af16bedb9a714feb788ae562be38898ab92dc54b4665882bf1c6` |
| Phase 42 report | `36507b03570313bb484c418243c95752ea9395f4aa56589c3633f7af0370cc4b` |

## Results

| Metric | Result |
| --- | ---: |
| Candidate cases audited | 17/17 |
| Coherent reusable families | 0 |
| Specialist or incompatible cases | 17/17 |
| Cases requiring a non-explicit/specialist matrix bridge | 14 |
| Cases not using a direct rank/determinant operation | 14 |
| Cases with parameters or random variables | 5 |

Repeated surface signatures were still split by exact input and operation:

| Repeated signature shape | Cases |
| --- | ---: |
| Explicit matrix + determinant identity | 3 |
| Control-system matrices + control design/factorization | 2 |
| Graph/adjacency matrix + graph invariant | 2 |
| Specialist matrix context + rank constraint | 2 |
| Other distinct signatures | 8 |

None of these repeated groups shares the complete contract required for a
safe capability: compatible input representation, operation, assumptions,
output, and existing typed bridge.

## Case-level fields audited

Each record preserves:

* input representation and dimensions;
* requested output;
* real/complex/integer/random/symbolic domain;
* numeric versus parameterized structure;
* exact operation;
* theorem or identity dependence;
* proof versus computation target;
* existing solver compatibility;
* required typed bridge;
* coherence signature and rejection reasons.

The largest apparent group therefore fails the semantic-coherence gate before
external education. No matrix contract is proposed, no HLE answer is
authorized, and production routing remains unchanged.
