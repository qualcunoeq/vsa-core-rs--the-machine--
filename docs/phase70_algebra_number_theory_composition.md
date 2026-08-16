# Phase 70 — Algebra/number-theory composition audit (hardened)

Phase 70 verifies that elementary number-theory operations consume algebraic
prerequisites without erasing arithmetic conditions. Supported routes include
Bézout-to-inverse, gcd-to-congruence, compatible CRT, algebraic unit-to-inverse,
and kernel/image-to-congruence handoffs.

The independent corpus contains 240 routes:

* 120 supported routes with preserved invariants;
* 40 ambiguity or missing-evidence routes;
* 80 explicit refusals for nonunits, incompatible CRT systems, nonlinear
  Diophantine requests, cryptographic claims, and missing coprimality evidence.

The receipt records 240/240 exact route decisions, 120/120 supported invariant
checks, and **120/120 typed handoffs**.  Each supported second stage is checked
against the prior artifact and shared operands: a Bézout certificate must prove
the inverse prerequisite, CRT compatibility is tied to the preceding gcd, and
the kernel-to-congruence route checks the source/target lift rather than
equating unrelated solution counts.  All 240 routes replay and reject tampered
receipts, with zero false authorizations or denials. HLE remains untouched and
the routes remain shadow-only.

Run:

```text
cargo run --quiet --bin number_theory_composition_bench
```
