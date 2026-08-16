# Stage C — shifted finite-statistics language campaign

This campaign tests the finite-statistics frontend against an independently
authored shifted surface rather than repeating the original five sentence
templates. Supported cases reorder clauses, use explicit `=` and `:`
separators, vary aliases, and preserve exact integer or rational values.
Ambiguous cases omit a required target or quantity; refused cases request
continuous or inferential statistics, or provide unlabeled observations.

The corpus contains 2,000 cases:

* 1,200 supported reports;
* 400 ambiguous reports;
* 400 refused reports.

Results:

| metric | result |
| --- | ---: |
| exact frontend decisions | 2,000/2,000 |
| authorized answers | 1,200/1,200 |
| supported downstream replays | 1,200/1,200 |
| frontend replay | 2,000/2,000 |
| downstream replay (emitted results) | 1,200/1,200 |
| frontend tamper rejection | 2,000/2,000 |
| downstream tamper rejection | 2,000/2,000 |
| false authorizations | 0 |
| false denials | 0 |

Corpus hash: `f5a63fef07970c62091bf275e3808c4e7dc381fc6726704357e21d37d2db55e0`.

This remains a bounded frontend result, not evidence of unrestricted technical
language understanding. The parser accepts only explicit labels and exact
finite-statistics forms; it does not infer values or statistical assumptions
from ordinary prose.

Run:

```text
cargo run --quiet --bin source_statistics_shifted_bench
```
