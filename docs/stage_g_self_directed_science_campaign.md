# Stage G — self-directed science curriculum campaign

This shadow campaign receives exact typed gap observations and chooses among
source-backed biology, chemistry, and finite-statistics learning modules. It
does not use answer keys, mutate the curriculum manifest, or authorize a
module merely because it has lexical overlap. Selection is based on exact
artifact coverage, prerequisite closure, source provenance, and independent
exercise evidence.

The campaign contains 500 observations in four exact gap clusters:

* 350 DNA/base-composition gaps;
* 100 molecular-formula gaps;
* 50 finite-statistics gaps.

The planner selected `source_derived_biology` because it covered 350 exact
observations. Chemistry and statistics remained viable alternatives, while an
unproven shortcut candidate was blocked.

| Check | Result |
| --- | ---: |
| Observed cases | 500 |
| Gap clusters | 4 |
| Candidate plans | 4 |
| Selected coverage | 350 |
| Independent validation | 120/120 |
| Selected plan replay | true |
| Plan tamper rejection | true |
| Blocked shortcut candidates | 1 |
| Manifest unchanged | true |
| Production authorizations | 0 |
| False authorizations | 0 |

Corpus SHA-256:
`3617080d0cf329b8030251ef386ef8c65cfdf3d923acc67e1feb10525ce65eca`

This is a shadow learning-plan result; promotion remains a separate governed
operation.

Reproduction:

```text
cargo run --quiet --bin self_directed_science_campaign_bench
```
