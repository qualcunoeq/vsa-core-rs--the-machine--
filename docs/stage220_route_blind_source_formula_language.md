# Stage 220 — route-blind source-formula technical language

This benchmark puts raw technical reports in front of five independently
sourced declarative formula catalogs: bounded economics, finite statistics,
finite regression, rectangular complex arithmetic, and finite sequences/series.
Every report is offered to every catalog. A route is authorized only when one
catalog emits a complete typed request and the generic source evaluator emits
a replayable result.

Results:

* 2,000 cases: 1,200 supported, 400 ambiguous, 400 unsupported;
* 2,000/2,000 exact route decisions;
* 1,200/1,200 authorized downstream artifacts;
* 10,000/10,000 frontend invocations replayed and tamper-rejected at the
  case level;
* 1,200/1,200 downstream replays and tamper rejections;
* 2,000/2,000 provenance-preserving cases;
* zero false authorizations, false denials, or live registry mutations.

The corpus hash is
`b50f7013081728a4b3902025828de33cd907643235a887e950fa9a23b69de1e8`.
The benchmark does not read HLE and does not alter production routing.

This remains a bounded technical-language result: it accepts source-declared
formula aliases and explicitly labeled rational inputs. It is evidence for
route isolation and provenance-preserving handoff, not unrestricted prose
understanding.
