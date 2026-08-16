# Stage C: controlled technical-language benchmark

This benchmark places a noisy semantic boundary in front of the validated
curriculum packs. It uses independently generated paraphrases rather than
route labels, and emits only typed pack requests, ambiguity, or unsupported
classification. It does not add live parser routes or use HLE as development
data.

The corpus is recorded in `docs/stage_c_technical_language.json` with SHA-256
`5c233964304915372bdc7fef00e691c62e076c73bfcdea05ad8bb1588e18303a`.

| metric | result |
| --- | ---: |
| cases | 2,000 |
| supported reports | 1,200 |
| ambiguous reports | 400 |
| unsupported reports | 400 |
| typed targets grounded | 2,000/2,000 |
| ambiguities preserved | 400/400 |
| unsupported reports refused | 400/400 |
| supported routes authorized | 1,200/1,200 |
| downstream replay | 2,000/2,000 |
| provenance preserved | 2,000/2,000 |
| false authorizations | 0 |
| false fact insertions | 0 |

The language surface is controlled and bounded: combinatorial selection,
modular inverses, scalar ODEs, affine derivatives, and simple complete graphs,
plus explicit ambiguous and unsupported near-misses. This is an initial gate
for technical-language ingestion, not evidence of unrestricted natural-
language competence.
