# Stage H — shifted DNA-language frontend

The bounded biology pack now has a narrow technical-language handoff. It
accepts explicit DNA sequence cues, identifies validate/complement,
reverse-complement, and base-composition requests, and requires an explicit
5-to-3 orientation for complement operations. Multiple sequence spans remain
ambiguous. RNA, codon, translation, protein, mutation, and phenotype requests
are refused rather than routed to DNA operations.

The independently authored shifted corpus contains 240 cases:

| Outcome | Cases |
| --- | ---: |
| Supported | 120 |
| Ambiguous | 40 |
| Unsupported | 80 |

Corpus SHA-256:
`1c8440ce638c7ea7c44d841f14beb09d3bb9b093d7d3a5909c65031367169846`

| Check | Result |
| --- | ---: |
| Exact frontend decisions | 240/240 |
| Complete frontend requests | 120/120 |
| Downstream biology authorizations | 120/120 |
| Frontend replay | 240/240 |
| Downstream replay | 120/120 |
| Frontend tamper rejection | 240/240 |
| Downstream tamper rejection | 120/120 |
| Provenance preserved | 240/240 |
| False authorizations | 0 |
| False denials | 0 |

The frontend is shadow-only and does not mutate live routing or the curriculum
registry.

Reproduction:

```text
cargo run --quiet --bin biology_frontend_bench
```
