# Phase 43 — Equation-binding primitive audit

Phase 43 audits the 11 scalar-output HLE cases whose Phase 30 records used the
`equation_binding` bridge. It separates reusable low-level binding operations
from the domain-specific method needed after binding.

## Reproducibility

```text
cargo run --quiet --bin hle_equation_binding_audit -- \
  docs/phase43_hle_equation_binding_audit.json
```

| Artifact | SHA-256 |
| --- | --- |
| HLE dataset | `31b26cc8e352af16bedb9a714feb788ae562be38898ab92dc54b4665882bf1c` |
| Phase 30 law audit | `9fbe52a26b378c16e858bca75ca2835b5339aae5c31602e068b446205956c0ed` |
| Phase 43 report | `cdf639235f81f12417d51b1f77726a65cf0c6d65c0892561e0518ffe214d59b2` |

## Results

| Metric | Result |
| --- | ---: |
| Scalar equation-binding cases | 11/11 |
| Cases sharing local-symbol binding | 11 |
| Cases sharing requested-unknown binding | 11 |
| Cases requiring indexed/function/domain binding | 4 |
| Cases requiring coupled-constraint binding | 2 |
| Cases requiring assumption propagation | 5 |
| Reusable low-level bridge primitives | 2 |
| Reusable domain method families | 1 two-case family |

The two reusable bridge primitives are:

```text
bind_local_symbols_to_typed_values
bind_requested_unknown
```

The only repeated domain method family is `parametric_regression_fit` with two
cases. The other nine cases are specialist singletons: topological
classification, statistical response, graph invariants, functional
inequalities, electromagnetic fields, nonlocal PDEs, integer identities,
analytic asymptotics, and matrix-model questions.

## Interpretation

Equation binding is a useful infrastructure layer, but the HLE sample does
not justify a broad domain solver. The two binding primitives are generic
enough to validate independently against external examples; the regression
pair may be a separate small candidate, but is not promoted from this audit.

Every case preserves required symbol bindings, assumptions, rejection
boundaries, primitive operations, and bridge signature. No HLE answer was
authorized and production routing is unchanged.
