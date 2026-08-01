# Phase 29 — full HLE reasoning-method coherence audit

Phase 28 showed that the 13 grounded notation cases were a specialist tail,
not a safe source of reusable methods. Phase 29 applies the same strict
semantic-signature test to all **222** HLE cases previously classified as
`missing_reasoning_method`.

The audit distinguishes:

* transformation candidates;
* representation-only bridge candidates;
* knowledge-dependent methods;
* composition gaps;
* specialist singletons;
* ambiguous or contaminated cases.

Subject labels are not sufficient. A family is contract-ready only when its
cases share an exact transformation signature and output artifact.

## Result

| Metric | Result |
|---|---:|
| Missing-method cases audited | 222 |
| Transformation candidates | 141 |
| Representation bridges | 29 |
| Knowledge-dependent candidates | 1 |
| Specialist singletons | 51 |
| Coherent reusable families | **0** |

The largest apparent groups still fail the output/signature gate. For example,
the 12 recurrence-looking cases mix cardinality, expression, and scalar
outputs; the 20 matrix-looking cases do the same. The audit also splits
seemingly similar matrix, hyperbolic, sampling, and special-function prompts
when their operators or state transformations differ.

No bridge or method contract is proposed, and no production route changes.

## Provenance

The report uses the frozen HLE dataset hash
`31b26cc8e352af16bedb9a714feb788ae562be38898ab92dc54b4665882bf1c` and the
regenerated bounded release trace hash recorded in the Phase 27 manifest.
Because the original full-process trace was not retained, the report is an
aggregate regeneration, not a claim of byte identity with the historical
trace.

The machine-readable output is `docs/phase29_hle_reasoning_method_audit.json`
with SHA-256
`426bf65d4daca4430bbb69c717ca4f636ee0d48d9822c8fd001dff6d33fdabd2`.

## Reproduction

```text
cargo test --bin hle_reasoning_method_audit
cargo run --bin hle_reasoning_method_audit -- \
  /tmp/hle_phase26_combined.traces.jsonl \
  /tmp/hle_reasoning_method_audit_2147e9e.json
```

This phase is diagnostic only. A future capability proposal must begin with a
homogeneous subfamily and an independent positive/ambiguous/unsupported
corpus; the 222-case aggregate is not itself authorization evidence.
