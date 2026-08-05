# Phase 62 — Calculus composition and semantic boundaries

This shadow benchmark composes the bounded calculus pack with existing
discrete dynamics, finite probability, and mechanics-shaped symbolic routes.
It authorizes only explicit semantic bridges:

* a declared scalar update expression may be differentiated;
* a continuous-time mechanics expression may use the calculus derivative;
* an exact antiderivative may be checked against a bounded definite integral;
* a supported polynomial limit may support a continuity result.

The benchmark refuses the tempting but invalid conversions:

* discrete recurrence → differential equation;
* sampled data → continuous function;
* finite probability mass → density/integral;
* finite difference → derivative;
* finite sum → integral;
* inferred domains;
* cancellation across excluded points;
* antiderivatives without domain constraints.

## Frozen result

Receipt: [phase62_calculus_composition.json](phase62_calculus_composition.json).

| Metric | Result |
|---|---:|
| Cases | 240 |
| Supported compositions | 120 |
| Safe refusals | 120 |
| Exact route decisions | 240/240 |
| Intermediate artifacts valid | 120/120 |
| Stronger invariants preserved | 120/120 |
| Replay verification | 240/240 |
| Tamper rejection | 240/240 |
| Approximation bridges refused | 120/120 |
| False authorizations | 0 |
| False denials | 0 |
| Semantic leakage | 0 |

Run with:

```text
cargo run --bin calculus_composition_bench
```

