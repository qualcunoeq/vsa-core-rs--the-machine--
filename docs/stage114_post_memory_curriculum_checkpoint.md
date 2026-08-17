# Stage 114 — Post-memory sealed curriculum checkpoint

This checkpoint preserves separate denominators for curriculum cases, source
catalogs, memory records, retrieval queries, and the frozen HLE holdout.

| Metric | Result |
|---|---:|
| Curriculum cases | 10,400 |
| Curriculum authorized | 6,204 |
| Curriculum safe refusals/ambiguities | 4,196 |
| Curriculum replay | 10,400/10,400 |
| Curriculum tamper rejection | 10,400/10,400 |
| Curriculum false authorization/denial | 0/0 |
| Source catalogs | 3 |
| Memory records | 100,000 |
| Memory replay | 100,000/100,000 |
| Memory tamper rejection | 100,000/100,000 |
| Retrieval queries | 2,000 |
| Retrieval receipt replay | 2,000/2,000 |
| Retrieval receipt tamper rejection | 2,000/2,000 |
| Retrieval contamination | 0 |
| Manifest mutation | 0 |
| Frozen HLE cases | 2,500 |
| Frozen HLE correct authorized | 2 |
| Frozen HLE false authorizations | 0 |

The HLE values are a preserved baseline, not a new evaluation. Parent report
hashes and the immutable holdout remain recorded for later checkpoints.
