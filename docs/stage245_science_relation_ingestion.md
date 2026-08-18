# Stage 245 — science and relation ingestion

This stage adds a controlled scientific boundary using three independently
typed paths: an attributed DNA base-pair relation, bounded chemistry formulas
and reactions, and bounded DNA operations.

Results:

* 1 source relation record parsed and provenance-preserved;
* relation path: 120/120 supported and exact, 3/3 boundaries refused;
* chemistry path: 100/100 supported and exact, 50/50 boundaries refused;
* biology path: 100/100 supported and exact, 50/50 boundaries refused;
* 500 total cases, 320 authorized exact results, and no false authorization;
* 623 result/frontend replays and 423 tamper rejections;
* unsupported semantics included unknown bases, ambiguous relations, wrong
  domains, molar-mass requests, unbalanced reactions, RNA/codon requests,
  missing strand orientation, and multiple sequence targets;
* zero false denials or live mutations.

The scientific routes remain bounded: chemistry does not infer unlisted
properties, and biology does not infer RNA, codon, mutation, or phenotype
semantics. Every accepted result retains source citation and replay evidence.

Source hash:
`9a34afa1464ee6290fc5026d3210457d50ba56fb0e70ca97bcbfb634b527bbc6`
