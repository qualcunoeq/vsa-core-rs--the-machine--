# Calculus funnel reconnaissance

The non-executing scan covered all 181 prompts classified as elementary
calculus. Explicit-expression guards reject nested, fractional, special
function, applied-modeling, and theorem prompts.

```text
prompts scanned                         181
explicit bounded operations               0
requires symbolic capability             25
requires modeling                         52
requires specialized definition           26
requires advanced theorem                  3
requires convergence argument              1
diagram/table dependent                   18
outside calculus signal                   56
```

Task counts:

```text
unclassified                             74
ode_pde                                  49
special_functions                        26
explicit_integration                     11
explicit_limit_evaluation                11
finite_or_infinite_series                 4
optimization                              3
asymptotic_analysis                       3
```

The four initial bounded candidates were nested-function integration,
high-power absolute trigonometric products, fractional derivatives in curved
spacetime, and coupled oscillator/wave equations. All require advanced
symbolic or applied modeling and were tightened out. The final run has
**zero** explicit bounded candidates.

Conclusion: no standalone calculus executor should be added for HLE score
purposes yet.
