# Phase 44 — EquationProblemBindingV1

Phase 44 adds a generic, shadow-only bridge from equation-bearing language to a
typed problem representation. It deliberately stops before solver selection or
answer authorization.

The bridge preserves local symbol scope, source spans, declared versus inferred
status, type/domain annotations, unresolved alternatives, assumptions,
dependencies, indexed objects, function domains, coupled constraints, and a
deterministic replay hash. The primitive vocabulary is:

```text
BindLocalSymbol
BindRequestedUnknown
PropagateAssumption
BindIndexedObject
BindFunctionDomain
ConstructCoupledConstraints
```

## Independent cross-domain corpus

The benchmark contains 120 deterministic cases across elementary algebra,
parameterized regression, probability, recurrence relations, mechanics,
matrix expressions, and functions/indexed sequences:

| Decision | Cases |
|---|---:|
| Supported / complete | 70 |
| Ambiguous | 30 |
| Unsupported | 20 |

Results:

* 120/120 exact structural decisions;
* 120/120 replay receipts verified;
* 0 incorrect symbol or target bindings;
* 13 assumption-propagation cases;
* 20 coupled-constraint cases;
* 40 rewrite groups retained in the corpus;
* 0 downstream authorizations.

Corpus report: [`phase44_equation_problem_binding_bench.json`](phase44_equation_problem_binding_bench.json)
(SHA-256 `1b95f77914827be2dc29ef5ca88439784c91f6ca418e269f58a3c4ae615f6118`).

The negative boundary includes multiple scopes, non-unique requested targets,
unstated index domains, unstated function domains, conventional assumptions,
observation/equation confusion, competing constraint systems, and unsupported
representations. Successful binding never authorizes a solver call.

## Frozen HLE rerun

The 11 frozen scalar `equation_binding` cases from the Phase 30 law audit were
rerun against `data/hle.jsonl` without changing production routing:

* 11/11 binding replays verified;
* 10 ambiguous bindings preserved;
* 1 binding-complete case classified as requiring a specialist method;
* 0 downstream authorizations;
* 0 production registry or router changes.

Classification report: [`phase44_hle_equation_problem_binding_shadow.json`](phase44_hle_equation_problem_binding_shadow.json)
(SHA-256 `9a9fc261049e4f5c715893cd2c095aacaac4d96a46e18914dae799a6045c6569`).

The HLE inputs are frozen by dataset hash
`31b26cc8e352af16bedb9a714feb788ae562be38898ab92dc54b4665882bf1c`; the
Phase 30 audit hash is
`9fbe52a26b378c16e858bca75ca2835b5339aae5c31602e068b446205956c0ed4`.

This phase validates reusable semantic binding infrastructure, not a solver and
not HLE capability. The next downstream methods remain independently governed.
