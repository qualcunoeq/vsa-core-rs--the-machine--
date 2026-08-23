# Stage 393 — external mean-binding transfer checkpoint

The development-only residual audit identified a repeated, source-backed
frontend mechanism: a finite mean stated as an explicit count and total
(`sum of N items is S`). The frontend now lowers that statement to the
existing source-derived `arithmetic_mean` record. The generic hash-only
scorer also recognizes LaTeX fraction answers as an equivalent representation;
it does not change candidate values or routing.

Frozen inputs:

- external questions SHA-256: `3cf924116a0f8f6a0c84d0ce7949b0c1e16221e0d4b5fcb0c4322110e30714f2`
- finite-statistics source SHA-256: `cb935aaf7d099bbea76f918992096d78bc79968afec037692f8544fa0cc55fcc`
- source-plan SHA-256: `c2e589c326cce225872a44f3e2d358a97950a24d49bed8bbb5632e8bd44c41b7`
- independent benchmark report SHA-256: `cff83b2aae33344c978db05cacf417b2ce6cb399d8d4991b1a2c03cdf8d4182d`

Independent validation (`stage393_external_mean_binding_bench`):

- 240/240 exact decisions;
- 120/120 supported artifacts and values;
- 240/240 frontend replay and tamper rejection;
- 240/240 downstream tamper rejection;
- 0 false authorizations or denials.

Answer-key-blind transfer, shadow-only:

| Partition | Questions | Unique candidates | Correct candidates | Candidate replay | False authorization |
|---|---:|---:|---:|---:|---:|
| Development | 3,000 | 12 | 12 | 12/12 | 0 |
| Sealed | 1,000 | 1 | 1 | 1/1 | 0 |

Development route candidates increased from 11 to 12; the new candidate is a
previously unsupported natural-language mean problem. The sealed count did
not change. No plaintext answers were read, the sealed scorer used only answer
hashes in privileged evaluation, and the curriculum manifest/live registry
remained unchanged.

Raw evaluator hashes are retained outside the repository run directory:

- development probe: `691c921631911e0f06213887c84d53aa8a28018fba1de4c49463073a3eba3331`
- development score: `9fb87afadaec3e2efda23b8b3e04a4ffd87d6fdfc8662f768b0642fac58cd71d`
- sealed probe: `daa5becd02d2d66daa9f152cbfddfa86181ea9695a86fb023bc43fa877313142`
- sealed score: `3dbc698749cd9d73ee1da14d15cee71b6d34497ea2d9b7f4e939b92fe2526902`

This is a small transfer gain, not broad external competence. The route
remains shadow-only and is not promoted to production.
