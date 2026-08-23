# Stage 467 — natural-language GCD route transfer

The validated bounded number-theory pack now accepts a narrow natural-language
bridge for explicit literal-integer GCD pairs (`greatest common divisor of 91
and 72` and `gcd(…)`). Algebraic expressions, target variables, mixed GCD/LCM
requests, and inferred operands remain refused.

| Metric | Development | Sealed |
|---|---:|---:|
| Questions / answer hashes | 3000 / 3000 | 1000 / 1000 |
| Route invocations | 57000 | 19000 |
| Unique candidates | 42 | 10 |
| Correct candidate hashes | 42 | 7 |
| Incorrect candidates rejected | 0 | 3 |
| Candidate replay | 42/42 | 10/10 |
| No executable route | 2958 | 990 |
| False authorizations | 0 | 0 |
| Manifest unchanged | true | true |

The new GCD route added three correct development candidates:
`math-v1-development-0168`, `-0379`, and `-0654`. It produced no sealed
candidate, so the frozen sealed result remains **7/1000**. The previous false
shadow candidate caused by binding a target variable as an operand was
rejected after a counterexample-driven parser repair.

Focused number-theory frontend tests: 5/5 passed. The route remains
shadow-only and does not authorize production answers.

Evidence:

* development probe report: `56ad0fbcfc4b04799001cf503fc4f4efd73c89e892b9caca1a1d4ed887d1c45e`
* development score report: `e8ab9a2a96c61dfed872b2c2e31f9ab52c61e22e9b71e3362430d1ead4389ae4`
* sealed probe report: `68d60541e66231444b48287e5bf7e154950756774b1fa48058ab17098e69a113`
* sealed score report: `e5bdb62c19e7f58bc53ba4175c57d5cd2205643c51ea37e17e513f351b3a77fe`
