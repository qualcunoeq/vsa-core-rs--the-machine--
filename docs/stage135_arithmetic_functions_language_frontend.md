# Stage 135 — arithmetic-functions language frontend

Stage 135 adds a controlled technical-language frontend for the bounded
arithmetic-functions pack. It accepts only explicit finite requests for
divisor count, divisor sum, the Möbius function, or prime counting with a
positive integer argument. Missing operation/value information is preserved as
missing or ambiguous, while analytic, asymptotic, unbounded, and approximate
requests are refused.

| Measure | Result |
|---|---:|
| Cases | 240 |
| Supported / ambiguous / unsupported | 120 / 40 / 80 |
| Exact frontend decisions | 240/240 |
| Downstream artifacts emitted | 120 |
| Downstream authorizations | 120/120 emitted |
| Frontend replay verified | 240/240 |
| Downstream replay verified | 120/120 emitted |
| Frontend tamper rejected | 240/240 |
| Downstream tamper rejected | 120/120 emitted |
| False authorizations / denials | 0 / 0 |

The frontend is shadow-only and does not mutate the registry. Successful
requests retain source spans and provenance before entering the bounded pack;
completing a frontend binding alone never authorizes an answer.

Corpus SHA-256: `90e93939cf5ec543c1f8769ce5fcd000a1738f51c2c001e7f994fca85cfef9d0`

Reproduce with:

```text
cargo test --quiet --lib bounded_arithmetic_functions_frontend
cargo run --quiet --bin stage135_arithmetic_functions_language_frontend
```
