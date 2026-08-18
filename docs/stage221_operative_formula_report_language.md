# Stage 221 — operative formula report grounding

This gate extends the route-blind source-formula frontend to multi-region
reports. Each supported report contains a source definition or incidental
formula followed by an explicit operative request. The frontend records all
regions, selects only the formula attached to an operative target verb, and
passes the selected source record to the same generic evaluator. Reports with
multiple operative formulas remain ambiguous.

Results:

* 2,000 reports: 1,200 supported, 400 ambiguous, 400 unsupported;
* 2,000/2,000 exact route decisions and 1,200/1,200 authorized routes;
* 10,000/10,000 report invocations with 2,000/2,000 case replays and tamper
  rejections;
* target regions preserved in 1,200 supported cases;
* excluded definition/context regions preserved in 1,600 supported or
  ambiguous cases;
* 1,200/1,200 downstream replays and tamper rejections;
* zero false authorizations, false denials, or live registry mutations.

Corpus hash:
`5936d7b49a1cdd0e2159f792cd9b70c3879b36184e1135e9a9ca7f7eb964cca6`.

This remains a bounded source-derived language gate. It does not infer a
formula from subject vocabulary, and it does not read HLE or mutate production
routing.
