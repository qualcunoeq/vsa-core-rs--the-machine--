# Finite-math funnel reconnaissance

The non-executing funnel scanned the 215 prompts previously classified as
`finite_combinatorics`. It extracted operation and modeling semantics without
calling a solver.

```text
prompts scanned                         215
explicit bounded exact operations         0
explicit uniformity                      10
requires combinatorial modeling         50
requires probability modeling             9
missing sampling policy                  23
missing replacement semantics             2
requires advanced theorem                20
diagram/table dependent                  16
outside finite-math signal               95
```

Task-kind counts:

```text
unclassified                            111
domain_modeling_required                 50
uniform_finite_probability               28
advanced_combinatorics                   16
expectation_finite_support                4
advanced_probability                      3
variance_finite_support                   2
recurrence                                1
```

The first pass produced eight false bounded candidates because generic words
such as “arrange”, “permutation”, `C(...)`, and “uniform” appeared inside
advanced graph, geometry, group-theory, recurrence, or puzzle questions. The
classifier was tightened to require explicit finite operands and to reject
those domain-modeling markers. The rerun produced **zero** bounded candidates.

Conclusion: there is currently no coherent explicit factorial/binomial/
uniform-finite-probability island in this HLE slice. Do not implement a
finite-math solver for score reasons yet; inspect number-theory or calculus
funnels next instead.
