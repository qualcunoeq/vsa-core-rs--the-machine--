# Stage 247 — multimodal science route-blind benchmark

This stage evaluates OCR-table ingestion without a subject-specific dispatcher.
Each table is formalized once from coordinate-bearing TSV and offered to the
probability, chemistry, and biology bridges. A route may authorize only when
its own typed semantics are satisfied; malformed and ambiguous tables fail
closed.

Results:

* 240/240 exact decisions: 80 probability tables, 80 element-count tables,
  40 base-count tables, and 40 refused ambiguous/malformed tables;
* 200 authorized cases, with exactly one authorized bridge per supported case;
* visual replay and tamper rejection: 240/240 each;
* bridge emissions/replays/tamper rejections: 720/720 each (three offered
  bridges per table artifact);
* zero route leakage, false authorizations, false denials, or manifest
  mutations.

The benchmark demonstrates that visual table structure can feed independent
science bridges without routing by labels or collapsing probability, chemistry,
and biology semantics. Value/weight tables and ragged grids remain refused.

Corpus hash:
`7d6a270338b5d075f2a3a3bf1c710abd6ce69f5cf977950406c7828c9f3e1e0e`
