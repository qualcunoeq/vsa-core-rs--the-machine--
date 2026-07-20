# HLE math-funnel candidate annotations

This is a diagnostic annotation of the 33 prompts marked `executor_candidate` by
the non-executing math funnel (`/tmp/hle_math_funnel_20260719_v4...`).  None is
inside the first algebra contract (anchored expression evaluation,
substitution, linear/quadratic real equations).  A correct answer must not be
attempted merely because the prompt contains `=` or LaTeX.

| # | source/domain | actual task | Tier-A algebra status | smallest missing capability |
|---:|---|---|---|---|
| 1 | Math | finite-set intersection extremal problem | reject | combinatorial design reasoning |
| 2 | Math | cubic polynomial plus integer sign constraints | reject | theorem/number-theory reasoning |
| 3 | Chemistry | molecular vibrational calculation | reject | chemistry model + units |
| 4 | Math | additive number theory threshold | reject | number-theory theorem |
| 5 | Math | uniform polynomial bound optimization | reject | real analysis/optimization |
| 6 | Math | complex-valued improper integral | reject | complex analysis + integration |
| 7 | Engineering | three-phase cable capacitance | reject | electrical model + units |
| 8 | Math | binomial convolution closed form | reject | generating functions/combinatorics |
| 9 | Math | iterative maximum-likelihood stochastic process | reject | probability/statistics semantics |
| 10 | CS/AI | block nested-loop join cost | reject | database cost model |
| 11 | Math | Kelly growth-rate comparison | reject | probability/optimization model |
| 12 | Math | nonlinear first-order differential equation | reject | ODE solver and solution conditions |
| 13 | Math | inverse-cosecant improper integral | reject | branch-sensitive calculus |
| 14 | Math | asymptotic hyperfactorial correction | reject | asymptotic analysis |
| 15 | Math | asymptotic product correction | reject | asymptotic analysis |
| 16 | Math | arithmetic-sequence functional constraint | reject | sequence theorem reasoning |
| 17 | Math | parametric arc length with possible parameters | reject | calculus/geometry |
| 18 | Math | elliptic/theta infinite product | reject | special-function identity |
| 19 | Math | cardinality of modular power set | reject | modular number theory |
| 20 | Math | large trigonometric product zero condition | reject | trigonometric/number-theory identity |
| 21 | Math | coalitional game value | reject | cooperative game theory |
| 22 | Math | finite additive-set existence problem | reject | additive combinatorics |
| 23 | Engineering | tandem aerofoil ground-effect lift ratio | reject | fluid/aerofoil model |
| 24 | Math | digit-reordering iteration | reject | discrete dynamical process |
| 25 | Physics | nonlinear optical cavity Hamiltonian | reject | quantum/statistical physics |
| 26 | Math | coefficient-square generating function | reject | generating functions |
| 27 | Math | nested definite integral expression | reject | nontrivial symbolic integration |
| 28 | Math | nonlocal gravitational functional equation | reject | advanced functional/physics model |
| 29 | Math | fourth Maclaurin coefficient of nested functions | reject | formal power-series expansion |
| 30 | Math | nonlinear rational recurrence threshold | reject | recurrence/continued-fraction reasoning |
| 31 | Math | logarithmic oscillatory improper integral | reject | special-function integration |
| 32 | Chemistry | Heck-reaction topology indices | reject | chemistry/entity retrieval |
| 33 | Chemistry | Geary autocorrelation molecular descriptor | reject | chemistry/statistical descriptor model |

## Contract boundary

The first executable island is intentionally limited to:

- complete arithmetic expressions with no free variables;
- explicit substitution (`Evaluate 2*x+1 at x=3`);
- one-variable linear equations;
- one-variable real quadratics (including no-root and repeated-root cases);
- exact replay of every returned root against the original equation.

The development corpus is in `algebra_island::development_cases()` and the
blind wording holdout is in `algebra_island::holdout_cases()`.  The 33 rows
above remain reconnaissance/coverage candidates until a separate typed method
contract is added for their required capability.

## Smallest-missing-capability counts

The annotation is a coverage diagnosis, not a failure of the CAS executor:

| capability needed to model the prompt | candidates |
|---|---:|
| calculus, asymptotics, ODEs, or special functions | 12 |
| domain-specific physical/chemical/CS model | 8 |
| combinatorics, sequences, or discrete construction | 6 |
| number-theory or trigonometric theorem reasoning | 4 |
| probability, optimization, or game-theory semantics | 3 |

None of the 33 is “equations already supplied, but only exact rationals or a
2×2 solver are missing.”  Exact arithmetic and systems are therefore engine
foundations, not evidence of immediate HLE coverage; the next score-oriented
step is a bounded reconnaissance pass over combinatorics/probability.

## Exact-backend safety boundary

The exact backend uses checked `i128` arithmetic with cross-cancellation for
addition, multiplication, and division. `IntegerOverflow` and
`ZeroDenominator` are explicit failure classes; an exact polynomial failure is
not downgraded to a floating-point solve. Irrational quadratic roots remain the
only intentional approximate terminal result inside the current contract.
