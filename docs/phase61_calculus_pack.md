# Phase 61 — Bounded exact calculus foundations

This shadow-only pack enters calculus through a deliberately small one-variable
grammar. It uses the existing symbolic algebra engine for exact derivative and
antiderivative artifacts, and only authorizes finite limit/continuity cases
whose result is witnessed exactly in the bounded polynomial grammar.

Supported operations are:

* polynomial and elementary symbolic derivatives;
* bounded exact antiderivatives;
* finite definite integrals with an exact integer witness;
* finite polynomial limits by substitution;
* continuity of polynomial expressions at defined points.

It refuses multivariable and partial derivatives, improper or infinite bounds,
measure/distribution semantics, numerical approximation, unsupported
convergence claims, and non-exact definite/limit results.

## Frozen benchmark

The receipt is [phase61_calculus_pack.json](phase61_calculus_pack.json).

| Metric | Result |
|---|---:|
| Cases | 240 |
| Supported | 120 |
| Boundary | 40 |
| Unsupported | 80 |
| Exact decisions | 240/240 |
| Supported artifacts | 120/120 |
| Replay verification | 240/240 |
| Tamper rejection | 240/240 |
| False authorizations | 0 |
| False denials | 0 |

The pack does not bridge finite differences to derivatives or discrete
dynamics to continuous-time evolution. Such bridges require explicit theorem
contracts in later curriculum phases.

Run the benchmark with:

```text
cargo run --bin calculus_pack_bench
```

