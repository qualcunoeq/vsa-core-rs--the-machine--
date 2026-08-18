# Stage 219 — generic source-formula frontend

This benchmark validates one catalog-agnostic technical-language frontend
against two independently sourced formula catalogs: finite statistics and
sequences/series. The frontend receives only source records and a domain. It
does not contain formula- or subject-specific evaluator branches.

The 1,200-case corpus contains 840 complete requests, 120 ambiguous requests,
120 missing-input requests, and 120 unsupported requests. Complete requests
must pass through the generic source evaluator before they count as downstream
successes. Every result retains provenance and a deterministic replay hash.

Results:

* 1,200/1,200 exact status decisions;
* 840/840 complete frontends and downstream executions;
* 1,200/1,200 frontend replays;
* 840/840 downstream replays;
* 1,200/1,200 tamper rejections;
* 1,200/1,200 provenance-preserving results;
* zero false authorizations and zero false denials.

The corpus hash is `bf5421d0174c192fc58fe8582d25a1833993c151191d5afaaad87d15f1b35259`.
The benchmark is shadow-only: production routing and the live registry are
unchanged. The existing `FormulaFrontend*` API remains available as a
compatibility wrapper around the stricter generic result type.
