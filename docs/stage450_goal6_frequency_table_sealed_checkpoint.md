# Stage 450 — sealed frequency-table transfer checkpoint

This checkpoint evaluates the frozen `c5ecd68` portfolio after adding the
independently validated bounded frequency-table frontend.  The evaluator ran
in explicit privileged hash-only mode; development code received no sealed
plaintext and no production route or curriculum mutation occurred.

| Metric | Result |
|---|---:|
| Sealed questions / answer hashes | 1000 / 1000 |
| Plaintext answers read | 0 |
| Unique shadow candidates | 6 |
| Correct candidate hashes | 3 |
| Incorrect candidate hashes rejected | 3 |
| Candidate replay verification | 6/6 |
| Multiple-route ambiguities / no route | 0 / 994 |
| Production authorizations / false authorizations | 0 / 0 |
| Manifest unchanged | true |

The new correct candidate was `math-v1-sealed-0239`, routed through
`frequency_table_mean`.  Its weighted mean matched the sealed answer hash.
The prior portfolio's two correct candidates remained correct, so this is a
measured sealed transfer improvement from **2 to 3 correct candidates out of
1,000**, not a claim that all six executable candidates were correct.

The three mismatches were rejected by the hash-only scorer and did not become
authorizations.  Every candidate receipt replayed successfully and every
tampered receipt was rejected.

Evidence:

* implementation commit: `c5ecd68`
* dataset SHA-256: `3cf924116a0f8f6a0c84d0ce7949b0c1e16221e0d4b5fcb0c4322110e30714f2`
* probe report SHA-256: `4a162124a243b2670ebc06eae1c8356c74bb0c511f1491a5a1ee72439e675907`
* score report SHA-256: `674eb4729c1292593412979685e58910240536363debd7a86930a5227d66071f`

This remains a shadow checkpoint.  The live registry and production router
were not changed.
