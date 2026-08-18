# Stage 228 — structural source-module discovery before acquisition

This campaign removes the remaining hand-authored module-list step. Bounded
source documents are parsed into provenance-bearing formula records, and only
then are typed catalog candidates derived. A malformed source is rejected
before candidate creation. Discovery, planning, sandbox acquisition, and
replay run without live memory or manifest mutation.

Results:

* 4 source documents inspected: 3 discovered modules and 1 malformed document
  rejected before candidate creation;
* 3/3 discovery receipts replay-verified;
* 180/180 exact source-memory gaps replayed across 3 exact clusters;
* 4/4 learning plans replayed, with 3 promotable plans and 1 broad distractor
  refused;
* 21 source records derived from the documents;
* development: 180/180 exact, replayed, and tamper-rejected;
* holdout: 90/90 exact and replayed;
* 9/9 source mutations rejected;
* 3/3 clone catalogs appended, uniquely retrieved, and replay-verified;
* 180/180 gaps resolved downstream with 180/180 replay and tamper checks;
* parent curriculum memory and manifest unchanged;
* zero false authorizations and zero live mutations.

This is shadow acquisition only. The source documents are bounded repository
transcriptions; discovery is structural and does not infer a domain-specific
evaluator or publish a live capability.

Corpus hash:
`3e0aef0776a6a76967e0ca2f140d60e6ce7b6a3776b5c17806f10c1a9c58abd8`
