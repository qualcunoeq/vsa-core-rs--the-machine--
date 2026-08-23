# Stage 394 — external progression-mean transfer checkpoint

The development-only residual audit identified a bounded repeated mechanism:
the mean of a finite arithmetic progression can be computed from justified
endpoints, including finite multiple ranges.  The source-derived frontend
lowers that statement to a generic formula record; the evaluator remains the
shared declarative expression runtime.

Frozen inputs:

- external questions SHA-256: `3cf924116a0f8f6a0c84d0ce7949b0c1e16221e0d4b5fcb0c4322110e30714f2`
- progression source SHA-256: `f92f74e0de178f21202e5f1f8c65e9cb6b8d363e20df21de94dc1f8e561df9db`
- independent benchmark report SHA-256: `a1bc1ae506f922534a33f2619f990579149d36edc43265709437b180aaf0d46f`
- curriculum manifest replay hash before/after: `3da81612ddac024a10c4e4c60547ed3b745662c33d5e1331769d8713822b6844`

Independent validation (`stage394_external_progression_mean_bench`):

- 240/240 exact decisions;
- 120/120 supported artifacts and values;
- 240/240 frontend replay and tamper rejection;
- 120/120 downstream replay;
- 240/240 downstream tamper rejection;
- 0 false authorizations or denials.

Answer-key-blind route-blind transfer, shadow-only:

| Partition | Questions | Route invocations | Unique candidates | Correct candidates | Candidate replay | False authorization |
|---|---:|---:|---:|---:|---:|---:|
| Development | 3,000 | 24,000 | 13 | 13 | 13/13 | 0 |
| Sealed | 1,000 | 8,000 | 1 | 1 | 1/1 | 0 |

The development candidate added by this route is
`math-v1-development-0513`; it is the arithmetic-progression mean case
identified by the residual audit.  The sealed candidate remains the prior
arithmetic-sequence route (`math-v1-sealed-0001`), so this stage does not claim
a new sealed capability.

Probe/score report hashes are retained outside the repository run directory:

- development probe file: `40495243b6a197098bb68603bda4ff5c4f8b0eca51c7dc992d51a65cd5a3ae7a`
- development score file: `e9445d62962be0706510892b299c4e44e0b72d4be94e802f5450e2920f1616bc`
- sealed probe file: `2c1a328d31952db7e5b39bd256f47a8b9b16f5378842e5481628cd8a537d5281`
- sealed score file: `12aee60a6e75c6fe3116b614b7dbc47d529cf58d116aa45ad1640a70fc6040ed`

Internal report hashes:

- development probe report: `3954ebcd1383996ed6c2586c8a3bd3a0e377f91868d1cdb74eaefcdd2e101ee8`
- development score report: `6b3a7ed029677987089322ba8eccc733dff6446d8e51f8b8b30d3bf27087b918`
- sealed probe report: `9b7e0eab749cd37f259b404d5445e2d8bdf0088a3267dc128f05ffb47d23799b`
- sealed score report: `b24d07e1e35fc28463098cfb013e78ba4eb2f7fca471f87fd1ea14b6e0872fa4`

No plaintext answers were read.  Sealed scoring used only privileged answer
hashes.  The portfolio and source route remain shadow-only; no production
authorization or live registry mutation occurred.
