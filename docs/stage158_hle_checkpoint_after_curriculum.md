# Stage 158: HLE checkpoint after integrated curriculum

This is a sealed diagnostic checkpoint after the 7,400-case integrated
curriculum/source-memory release. The HLE dataset and router were unchanged;
the run cannot promote curriculum items or mutate a registry.

The executable records a SHA-256 for every question, the terminal classification,
answer digest, route receipt, and replay status. It also hashes the integrated
checkpoint, prior HLE checkpoint, dataset, and curriculum manifest so the
comparison is reproducible.

The result is reported in `stage158_hle_checkpoint_after_curriculum.json` and
the per-question trace is retained in the corresponding `.trace.jsonl` file.

## Result

The frozen run covered all **2,500** HLE questions. It authorized **2/2,500**
answers, matching the prior checkpoint (`delta = 0`), with **0** incorrect
authorizations and **0** false authorizations. There were **569** curriculum
signals, **260** visual-required cases, **1,718** cases without a curriculum
signal, and **520** unresolved signals. No pack route receipt was emitted.

Both authorized answers were replay-compatible; the remaining 2,498 cases were
not applicable for answer replay. The integrated 7,400-case curriculum
checkpoint and the prior HLE checkpoint are hash-bound in the JSON manifest.
The unchanged score is evidence that the current curriculum has not yet crossed
the HLE technical-language/specialist-method boundary, not evidence for
loosening routing.
