# Stage H — source-derived bounded DNA biology

This is the first source-derived biology capability. It represents DNA
sequences over the explicit alphabet A/T/C/G, computes base composition, and
constructs aligned or reverse complements only when strand orientation is
stated. It refuses RNA, codon translation, mutation/phenotype claims,
unbounded sequences, and orientation-dependent requests without an explicit
convention.

The source contract is based on OpenStax *Biology 2e*, sections 3.5 and 14.2:
DNA uses A, T, C, and G, with A pairing with T and G pairing with C. The
source record and URL are embedded in every accepted artifact.

The independent corpus contains 240 cases:

| Outcome | Cases |
| --- | ---: |
| Supported | 120 |
| Ambiguous | 40 |
| Refused | 80 |

Corpus SHA-256:
`7a7149423aea8ac1097ac341ecedc04cdcb74d3d19f82803d12b89aaf4a5198a`

| Check | Result |
| --- | ---: |
| Exact decisions | 240/240 |
| Supported artifacts | 120/120 |
| Replay verification | 240/240 |
| Tamper rejection | 240/240 |
| Provenance preserved | 240/240 |
| False authorizations | 0 |
| False denials | 0 |

The pack is shadow-only and does not mutate production routing, the live
registry, or the frozen HLE holdout.

Reproduction:

```text
cargo run --quiet --bin biology_pack_bench
```
