# Phase 34 — Externally grounded classical-mechanics pack

Phase 34 is the first specialist knowledge pack developed independently of
the HLE target questions. It is shadow-only: the pack is not registered with
the production router and cannot authorize HLE answers.

## Source-backed scope

The pack contains five relations from OpenStax *University Physics Volume 1*:

* Newton's second law, `F_net = m * a` — section 5.3;
* linear momentum, `p = m * v` — section 9.1;
* non-relativistic kinetic energy, `K = 1/2 * m * v^2` — section 7.2;
* Hooke restoring force, `F_spring = -k * x` — section 5.6;
* elastic potential energy, `U = 1/2 * k * x^2` — section 8.1.

Each record stores its source URL, section, license/attribution note,
retrieval date, assumptions, validity domain, variables, and unit constraints.
The source records are not treated as unrestricted facts: an exercise must
match a named record, provide all required typed inputs, and satisfy units
before evaluation.

## Independent boundary corpus

The 16-case corpus contains ten supported numeric exercises and six controls:

* missing mass or speed;
* incompatible units;
* relativistic kinetic energy outside the non-relativistic pack;
* overloaded `energy` alias;
* a Hooke-law request explicitly outside the linear regime.

Results:

| Metric | Result |
| --- | ---: |
| Total cases | 16 |
| Supported cases | 10 |
| Rejected/boundary cases | 6 |
| Complete results | 10 |
| False authorizations | 0 |
| False denials | 0 |
| Replay-verified results | 16/16 |
| Unique lookups | 5 |
| Ambiguous lookups | 1 |
| Unsupported/missing lookups | 2 |
| Registry mutated | No |

The immutable report is
[`phase34_classical_mechanics_pack_bench.json`](phase34_classical_mechanics_pack_bench.json),
SHA-256:

```text
13dcda1a6c2f31304f56720453c47718c2034d9b68d2553fc6ac58bc33376768
```

Run:

```text
cargo run --quiet --bin classical_mechanics_pack_bench -- \
  docs/phase34_classical_mechanics_pack_bench.json
```

## Architectural boundary

The pack deliberately does not support relativistic mechanics, variable-mass
forms of Newton's law, nonlinear springs, vector decomposition, collisions,
or natural-language extraction. Those require separate contracts and remain
unsupported. The evaluator emits a source-bearing result with a replay hash;
it does not publish facts or alter the law registry.

## Verification

```text
cargo test --lib classical_mechanics_pack --quiet
```

The focused deterministic test verifies a Newton-law calculation and its
replay receipt. The broader repository continues to emit unrelated
pre-existing warnings.

## Source records

* https://openstax.org/books/university-physics-volume-1/pages/5-3-newtons-second-law
* https://openstax.org/books/university-physics-volume-1/pages/9-1-linear-momentum
* https://openstax.org/books/university-physics-volume-1/pages/7-2-kinetic-energy
* https://openstax.org/books/university-physics-volume-1/pages/5-6-common-forces
* https://openstax.org/books/university-physics-volume-1/pages/8-1-potential-energy-of-a-system
