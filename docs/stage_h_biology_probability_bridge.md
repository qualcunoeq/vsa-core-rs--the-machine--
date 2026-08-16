# Stage H — biology/probability composition

This bridge turns DNA base counts into a finite probability distribution only
when the sampling policy explicitly says `uniform_position`. It preserves the
base outcome order A/C/G/T and the originating biology replay hash. Raw DNA
sequences, complements, and unsupported sampling assumptions do not acquire
probabilistic semantics.

The independent composition corpus contains 240 cases:

| Outcome | Cases |
| --- | ---: |
| Supported uniform-position distributions | 120 |
| Missing sampling policy | 40 |
| Refused composition | 80 |

Corpus SHA-256:
`31206e09cb295dcb3c02c8b14feaf60cb6824e52693cb932408954ef80063876`

| Check | Result |
| --- | ---: |
| Exact decisions | 240/240 |
| Valid typed handoffs | 120/120 |
| Biology replay | 240/240 |
| Bridge replay | 240/240 |
| Probability replay | 240/240 |
| Tamper rejection | 240/240 |
| Sampling policy preserved | 120/120 |
| Distribution semantics preserved | 120/120 |
| False authorizations | 0 |
| False denials | 0 |

The bridge is shadow-only and does not infer independence, population
frequencies, genotype probabilities, or stochastic-process semantics.

Reproduction:

```text
cargo run --quiet --bin biology_probability_bridge_bench
```
