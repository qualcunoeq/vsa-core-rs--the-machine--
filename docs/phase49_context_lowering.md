# Phase 49 — ContextBundleLoweringV1

Phase 49 adds a shadow-only typed lowering boundary:

```text
TargetContextBundle → EquationProblemSpec
```

Lowering compiles a justified cross-region context bundle into one of several
typed problem representations without solving, selecting a specialist method,
or authorizing an answer. Supported problem kinds are scalar equations,
property classifications, symbolic expressions, operator evaluations, and
coupled constraints. Ambiguous and unsupported bundles remain explicit
terminal states.

## Independent corpus

The independently authored corpus contains 100 cases:

* 70 complete cases (20 scalar, 20 symbolic, 20 property, 10 operator);
* 15 competing-scope ambiguities;
* 15 unsupported quoted-only contexts;
* 10 rewrite groups.

The frozen corpus hash is
`dca24f2081b38ed51383bf567d58327f3507149fdb5334056ec7872961531809`.

Results:

* 100/100 exact status decisions;
* 100/100 replay receipts verified, including rejection receipts;
* 0 dropped context symbols on complete cases;
* 0 downstream authorizations;
* 10/10 rewrite groups retained stable lowering decisions.

## Frozen HLE diagnostic rerun

The rerun consumes the Phase 48 context artifact and the frozen HLE dataset
(`31b26cc8e352af16bedb9a714feb788ae562be38898ab92dc54b4665882bf1c`). The
Phase 48 input artifact hash is
`0b5078fecb0cbf3b67d4132bc1a25ad8055b21136598d85c587859e2c381530b`.

All four previously blocked target-context cases now lower to complete typed
problem specifications and replay successfully:

* topological invariant → `PropertyClassification`;
* susceptibility `χ` → `SymbolicExpression`;
* Cheeger constant → `ScalarEquation`;
* exponent sum `α + β` → `SymbolicExpression`.

The terminal classification for all four is
`complete_lowered_problem_specialist_method_gap`: no specialist method was
selected, no candidate answer was emitted, and no downstream authorization
occurred. The HLE production score therefore remains 2/2,500.

## Boundary and non-goals

Lowering does not infer missing specialist semantics, solve equations, choose
among domain methods, or convert a completed lowering into authorization. A
complete lowered problem is only a typed handoff for a later, independently
validated method.
