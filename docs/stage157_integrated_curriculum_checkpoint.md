# Stage 157 — Integrated curriculum checkpoint

This immutable checkpoint aggregates two independent sealed exams without
merging their corpora: the existing 5,000-case curriculum exam and the
2,400-case direct/raw-OCR source exam. It also records the 100,000-record
source-route memory reconstruction as a prerequisite for sustained operation.

Aggregate results:

- cases: **7,400**;
- supported / ambiguous / unsupported: **4,440 / 1,480 / 1,480**;
- exact decisions: **7,400/7,400**;
- supported authorizations: **4,440/4,440**;
- replay and tamper verification: **7,400/7,400**;
- false authorizations and denials: **0 / 0**;
- source-route memory records: **100,000**, reconstruction hash equal;
- production mutations: **0**.

Child report hashes are recorded in the machine-readable artifact
`docs/stage157_integrated_curriculum_checkpoint.json`. The sealed partitions
remain immutable and are not used as development data.
