# Stage 460 — Goal 6 binomial-notation sealed checkpoint

This checkpoint evaluates the frozen counting frontend and route portfolio in
explicit privileged hash-only mode. No sealed plaintext was read, and the
production registry and curriculum manifest remained unchanged.

| Metric | Result |
|---|---:|
| Sealed questions / answer hashes | 1000 / 1000 |
| Route invocations | 18000 |
| Unique candidates | 15 |
| Correct candidate hashes | 7 |
| Incorrect candidates rejected | 8 |
| Candidate replay | 15/15 |
| No executable route | 985 |
| False authorizations | 0 |
| Manifest unchanged | true |

The prior sealed shadow result was **5/1000**. The binomial notation bridge
adds two correct candidates, `math-v1-sealed-0076` and
`math-v1-sealed-0355`, raising the result to **7/1000**. The eight mismatches
were rejected and did not authorize production answers.

Evidence:

* probe file SHA-256: `eb7c6debd3e43b9dab8b0fea4d84690faad582db9a7970675ceba9f95a32b10d`
* score file SHA-256: `b5dafeb97889c0a41148ce2575f15b07df609afaf2b8940edecb2dc1e4841990`
* report hashes: probe `7c3550d4f9b26433addc4392f367f456048f99495f485e9d4870ba75b3d265f1`, score `872c9f87081f379578b353eae5be03d22e2c929a803e2864062634ae1844662e`

This is shadow transfer evidence, not live promotion.
