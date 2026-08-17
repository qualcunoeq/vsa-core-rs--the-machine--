# Stage 155 — Sealed source/multimodal curriculum examination

This checkpoint exercises the source-derived statistics, biology, and
chemistry frontends together with their coordinate-bearing raw-OCR table
bridges. It is independent of HLE and does not mutate the production registry
or curriculum manifest.

The corpus has six balanced families (`2,400` cases total): direct statistics,
biology, and chemistry language, plus raw-OCR visual statistics, biology, and
chemistry. Each family has `240` supported, `80` ambiguous, and `80`
unsupported cases. The corpus is partitioned into development (`1,440`),
validation (`480`), and sealed (`480`) subsets.

Results:

- exact decisions: **2,400/2,400**;
- supported authorized: **1,440/1,440**;
- ambiguity preservation: **480/480**;
- unsupported refusal: **480/480**;
- replay verification: **2,400/2,400**;
- tamper rejection: **2,400/2,400**;
- false authorizations and false denials: **0 / 0**;
- production registry mutations: **0**.

Machine-readable receipts are in
`docs/stage155_sealed_source_multimodal_exam.json`.
