# Phase 6B — Governed linear-systems vertical

This phase extends the narrow algebra island from a fixed `Solve system:`
grammar to the independently authored prose forms exposed by the Phase 6A OOD
benchmark. It deliberately keeps the execution boundary narrow: exactly two
linear equations, two variables, exact coefficients, and a unique solution for
materialization.

## Scope

Accepted prose forms include:

- `Use ... and ... to determine x,y`;
- `Find x,y from ...; ...`;
- `Solve simultaneously: ... and ..., for x and y`;
- `The pair obeys ... and .... Solve for x,y`;
- rewrite variants with reordered equations and `together with` / ordered-pair
  wording.

The parser normalizes each form into the existing typed `EquationSystem` /
`VariableSet` contract. It does not fall back to the unbounded CAS parser.

## Outcome model

`classify_linear_system` now reports four explicit states:

- `Unique(solution)` — eligible for governed execution and replay;
- `NoSolution` — mathematically classified but not executable as a unique
  solution;
- `InfiniteSolutions(reason)` — dependent system, not executable as a unique
  solution;
- `Unsupported` — nonlinear, malformed, or outside the bounded contract.

`execute_linear_system` materializes only `Unique` results and requires replay
verification. This preserves the distinction between understanding a system
and being authorized to emit a unique solution receipt.

## OOD result

Command:

```text
cargo run --release --bin ood_bench -- data/algebra_ood_v1.json /tmp/ood_system_expansion_final2.json
```

Result:

```text
cases=48
decision=1.000
result=1.000
formalized=0.979
replay=1.000
false_auth=0
false_denials=0
rewrite decision stable=24/24
rewrite result stable=24/24
rewrite regressions=0
```

The 15 negative cases remain rejected, while all eight previously authorized
system executions now complete and replay successfully. The OOD oracle for
`x+3*y=11` and `2*x-y=3` was corrected to the exact solution
`{"x":"20/7","y":"19/7"}`.

## Focused tests

```text
linear_system tests: 5 passed
ood_benchmark test: 1 passed
```

The numeric-result comparison in the OOD harness also accepts equivalent exact
rational and decimal encodings for independently authored cases without
weakening execution or replay checks.

## Remaining boundary

This is intentionally not a general linear-algebra engine. Larger systems,
symbolic parameters, inequalities, nonlinear systems, and ambiguous prose
remain explicit unsupported outcomes. The next validation step is to grow the
independent corpus around these boundaries before adding further capability.
