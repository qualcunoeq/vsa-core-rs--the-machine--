# Number-theory funnel reconnaissance

The non-executing scan covered all 182 prompts classified as number theory.
The classifier required explicit integer operands, a finite target operation,
and no theorem/modeling marker before accepting a bounded computation.

```text
prompts scanned                         182
explicit bounded computations             0
explicit but magnitude-unsupported       4
requires advanced theorem               43
requires domain modeling                 21
requires search strategy                 11
requires proof                            5
diagram/table dependent                  13
outside number-theory signal             85
```

Task counts:

```text
unclassified                            113
advanced_theorem                         43
integer_sequence                          8
modular_evaluation                        4
prime_factorization                       3
diophantine_equation                      2
digit_constraint                          2
explicit_divisibility                     1
counting_integers                         1
```

The first pass exposed three false bounded candidates: infinite sums/products,
special-function products, and recurrence/binomial expressions with modular
notation. Those were tightened into advanced-theorem cases. The final rerun
has **zero** explicit bounded candidates.

Conclusion: do not implement Euclid, CRT, modular exponentiation, or prime
factorization for HLE score purposes yet. Number theory has the same boundary
as mechanics, algebra, and finite math: the benchmark prompts require model
construction or theorem selection before arithmetic.
