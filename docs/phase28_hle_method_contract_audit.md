# Phase 28 — HLE method-family coherence gate

Phase 27 identified two-case families, but a count alone does not establish a
reusable method. Phase 28 separates the bridge contract from the method
contract and refuses to propose either when the grounded cases have distinct
semantic signatures.

```text
grounded target
→ semantic method signature
→ family coherence gate
→ bridge contract + method contract only if coherent
```

## Result on the 13 grounded targets

Three families had at least two cases. All three failed the coherence gate:

| Family | Signatures | Decision |
|---|---|---|
| Geometry inequality | quadratic-image shape; Euclidean circle ratio | defer |
| Number theory | random-series expectation; modular-power cardinality | defer |
| Fractal dimension | section dimension bound; self-similar component count | defer |

Therefore:

* families considered: **3**;
* coherent families: **0**;
* selected contract: **none**;
* false authorizations: **0** (shadow-only);
* production routing and HLE score: unchanged.

This is a useful negative result. A broad `NumberTheory` or
`GeometryInequality` capability would have hidden unrelated theorem and
representation requirements inside one method. The gate preserves the
distinction instead of manufacturing a two-case success.

## Contract schemas

When a future family passes the gate, the report emits two separate records.

### Bridge contract

It specifies:

* source artifact type;
* target solver artifact;
* required bindings;
* semantic invariants;
* ambiguity boundaries;
* rejection boundaries.

### Method contract

It specifies:

* accepted problem form;
* transformation or theorem;
* assumptions;
* output type;
* replay obligations.

No contract is authorized or promoted by this tool.

## Reproduction

```text
cargo test --bin hle_method_contract_audit
cargo run --bin hle_method_contract_audit -- \
  docs/phase27_hle_method_audit.json \
  /tmp/hle_method_contract_audit_2147e9e.json
```

The regenerated output is retained as
`docs/phase28_hle_method_contract_audit.json` with SHA-256
`ad91d9b6f5617158956c035d31048a7b3e970553512c5ded29009ca37d749638`.

## Next gate

Do not synthesize a method from these three families yet. Either split one
family into a genuinely homogeneous subfamily with repeated evidence, or
audit the singleton families for a shared method signature. Only a coherent
family should proceed to an independent positive/ambiguous/unsupported
corpus and shadow implementation.
