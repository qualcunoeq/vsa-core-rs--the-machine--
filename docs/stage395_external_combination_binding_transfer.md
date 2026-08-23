# Stage 395 — external natural-combination transfer checkpoint

The development residual audit exposed a repeated explicit selection grammar:
natural-language questions of the form `choose R out of N`.  The frontend
binds that grammar to the existing source-derived combination operation; it
does not infer semantics from roles, probability, ordering, or constraints.

Independent validation (`stage395_external_combination_binding_bench`):

- 240/240 exact decisions;
- 120/120 supported values;
- 240/240 frontend replay and tamper rejection;
- 120/120 downstream replay;
- 240/240 downstream tamper rejection;
- 0 false authorizations or denials.

Answer-key-blind route-blind transfer, shadow-only:

| Partition | Questions | Route invocations | Unique candidates | Correct candidates | Candidate replay | False authorization |
|---|---:|---:|---:|---:|---:|---:|
| Development | 3,000 | 27,000 | 15 | 15 | 15/15 | 0 |
| Sealed | 1,000 | 9,000 | 1 | 1 | 1/1 | 0 |

The two new development candidates are:

- `math-v1-development-0463` — choose 3 out of 8;
- `math-v1-development-1372` — choose 2 out of 7.

The sealed candidate remains the prior arithmetic-sequence route
(`math-v1-sealed-0001`); this stage produces no new sealed candidate.

Frozen inputs:

- external questions SHA-256: `3cf924116a0f8f6a0c84d0ce7949b0c1e16221e0d4b5fcb0c4322110e30714f2`;
- independent benchmark report SHA-256: `9ce067c4a40635f40a6850913ed687e2ee5337c928b5a46e1d360b4bd0aa4d36`;
- source citation: OpenStax *Contemporary Mathematics*, counting principles,
  permutations, and combinations.

Probe/score report hashes are retained outside the repository run directory:

- development probe file: `e50a14c92a4f2f08c216afea2bc2144ff7bb653fd1f51bfd63c8c4374c33dbb5`;
- development score file: `3867b9614659268f768ee419e886ec4660621799f350f46270d809cdd951501b`;
- sealed probe file: `ccf72ab1c9f644b0c6400706f304bbd542574d4e64583e614cb64258e73bd7fb`;
- sealed score file: `9972f653b8912b4c30ac80944aa1806083862f2d3940a3d532ea6c2d7ca87800`.

Internal report hashes:

- development probe report: `47eeb365c7d6d485c9b3ae709c2e12f1dddf618253f730fc10ba123ce6ac0792`;
- development score report: `99b6acc39ac2ec9770e813cf6565224c729c6731254a5ab6eab6e1657207686a`;
- sealed probe report: `3912faaa3c5b381ffa7e1322bc9caac3c234e7e3ef73199ed066748d7b2708d6`;
- sealed score report: `2a918aa2763e416b127b64ad0a87636dbbffd2287ca3e5e92534aaef449f0efb`.

No plaintext answers were read.  Sealed scoring used only privileged answer
hashes.  Production authorizations, live registry mutation, and curriculum
manifest changes remained zero.
