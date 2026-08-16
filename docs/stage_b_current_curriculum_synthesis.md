# Stage B — current-curriculum cross-domain synthesis

This audit extends the original cross-domain synthesis gate to the current
curriculum state. It exercises explicit typed handoffs across:

* abstract algebra → elementary number theory (unit to modular inverse);
* bounded combinatorics → finite exact probability (count to declared
  two-outcome distribution);
* finite graph → linear algebra (adjacency matrix with preserved ordering);
* finite probability → finite Markov chains (validated initial distribution to
  one-step state evolution).

The corpus contains 1,000 route plans:

* 900 supported routes;
* 50 ambiguous routes with unresolved algebraic convention;
* 50 refused routes outside the declared number-theory domain.

| metric | result |
| --- | ---: |
| exact route decisions | 1,000/1,000 |
| supported semantic handoffs | 900/900 |
| replay verification | 1,000/1,000 |
| tamper rejection | 1,000/1,000 |
| false authorizations | 0 |
| false denials | 0 |

Corpus hash: `20779a277e6489fbdf9c4660736003e4566c8a938787a42f0a0462e9394096b4`.

This remains a shadow synthesis audit. It verifies composition of declared
artifacts; it does not mutate the live router, registry, or curriculum
manifest, and it does not claim broad natural-language transfer.

Run:

```text
cargo run --quiet --bin current_curriculum_synthesis_bench
```
