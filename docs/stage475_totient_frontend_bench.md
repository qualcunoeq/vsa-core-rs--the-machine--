# Stage 475 — canonical totient/unit-count pressure benchmark

The bounded number-theory frontend now recognizes direct literal Euler
totients and unit-count questions only when the interval is exactly `0..n-1`
modulo `n`. General coprimality counts, symbolic inputs, asymptotic requests,
and competing moduli remain closed.

| Metric | Result |
|---|---:|
| Cases | 240 |
| Supported / ambiguous / unsupported | 120 / 60 / 60 |
| Exact decisions | 240/240 |
| Supported values | 120/120 |
| Frontend replay | 240/240 |
| Execution replay | 120/120 |
| Tamper rejections | 360/360 |
| False authorizations / denials | 0 / 0 |

The independent corpus varies named totient, function-call, and canonical
unit-count prose, then attacks noncanonical ranges, symbolic values,
asymptotic wording, and competing moduli. Execution uses the existing exact
bounded trial-factorization evaluator.

Evidence:

* corpus SHA-256: `583b05181aa89b4e4848de7ad7e3f567c3dbe3bbf9b74af5af3e6cf0b07eb483`
* report SHA-256: `b22854a9e9c9f77ff99a14e87840bae2fae42d2e4c5cc7c906e24185d3ddadf0`

No production registry or curriculum manifest was mutated.
