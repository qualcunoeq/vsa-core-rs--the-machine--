# Stage 454 — sealed complex arithmetic transfer checkpoint

This checkpoint evaluates the complex-arithmetic route against the frozen
1,000-question sealed partition in explicit privileged hash-only mode.  The
implementation receives no sealed plaintext, and the live registry and
curriculum manifest remain unchanged.

| Metric | Result |
|---|---:|
| Sealed questions / answer hashes | 1000 / 1000 |
| Plaintext answers read | 0 |
| Unique shadow candidates | 8 |
| Correct candidate hashes | 5 |
| Incorrect candidate hashes rejected | 3 |
| Candidate replay verification | 8/8 |
| Multiple-route ambiguities / no route | 0 / 992 |
| Production authorizations / false authorizations | 0 / 0 |
| Manifest unchanged | true |

The two new correct candidates were `math-v1-sealed-0317` and
`math-v1-sealed-0527`, both routed through `complex_arithmetic`.  Together
with the prior three correct candidates, this raises the sealed shadow result
from **3/1,000 to 5/1,000**.  The three mismatches were rejected by hash
comparison and did not authorize production answers.

Evidence:

* source-derived complex pack pressure evidence: Stage 408
* dataset SHA-256: `3cf924116a0f8f6a0c84d0ce7949b0c1e16221e0d4b5fcb0c4322110e30714f2`
* probe report SHA-256: `a4bbe6870b3ab19e3612ec322c9012fd2f8345996a2d095577996e1437ac0530`
* score report SHA-256: `5f8e860ef21e9d6f4473e7f63e338dc33a1b3a56e1af3b36ddd27658ddc542a3`

This is shadow transfer evidence, not live promotion.
