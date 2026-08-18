# Stage 230 — provenance-only source-corpus discovery

This campaign removes the predeclared subject and version table from source
module discovery. The input is only three bounded source documents. Formula
records are extracted and grouped by their cited `SOURCE_ID`; catalog identity,
source lineage, and module boundaries are derived from that provenance.

Results:

* 3 raw documents accepted; a corpus containing one malformed document was
  rejected before any candidate was emitted;
* 6 provenance-derived modules from 21 source records, including four distinct
  cited sections inside the economics document;
* 6/6 discovery receipts replay-verified;
* 180/180 independent validation cases exact and replayed;
* 120/120 typed gaps replayed across 6 exact clusters;
* 7/7 plans replayed, with 6 exact source-backed plans and 1 broad distractor
  refused;
* 6/6 clone catalogs appended, uniquely retrieved, and replay-verified;
* parent memory and curriculum manifest unchanged;
* zero false authorizations and zero live mutations.

No subject-specific evaluator or curriculum-domain branch participates in
discovery. This remains a bounded, shadow-only source-education experiment;
the cited documents are immutable repository transcriptions.

Corpus hash:
`71b10d76a59b83b5ff439375a64d06d57e72f2d492a078ccdeacf18ad0607c7d`
