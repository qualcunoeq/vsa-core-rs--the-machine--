# Phase 48 — target-context assembly

Phase 48 adds a shadow `TargetContextBundle` handoff between target grounding
and equation-problem binding. It computes a relevance closure over separated
definitions, constraints, assumptions, and declarations, while explicitly
excluding incidental and quoted regions.

The bundle preserves target, operation, included and excluded regions, symbol
dependencies, assumptions, constraints, scope alternatives, provenance, and a
replay hash. It never authorizes downstream execution.

## Independent corpus

The corpus contains 90 cases:

* 30 complete context bundles;
* 30 ambiguous duplicate-scope bundles;
* 30 unsupported no-asserted-context bundles.

Results:

* 90/90 exact context decisions;
* 90/90 replay verified;
* 90/90 inclusion/exclusion decisions correct;
* 5 rewrite groups;
* 30 binding-handoff-ready bundles;
* 0 downstream authorizations.

Benchmark: [`phase48_target_context_bench.json`](phase48_target_context_bench.json)
(SHA-256 `c2c1c60a3ac076e22448102007c6ff49068b4af2f9e34a156dc5f5ea85abf4b7`).

## Frozen HLE rerun

All four Phase 46/47 residuals now receive complete, relevant context bundles:

* 4/4 context decisions complete;
* 4/4 context replays verified;
* quoted/incidental regions excluded in all four cases;
* 4/4 handoffs marked ready for a typed downstream binder;
* existing raw equation binding remains incomplete in all four cases;
* 0 candidate answers and 0 authorizations.

Terminal outcome: `context_complete_equation_binding_handoff_only` (4 cases).
This isolates the remaining work to the interface that consumes a structured
context bundle, rather than further region selection or target parsing.

HLE rerun: [`phase48_hle_target_context_rerun.json`](phase48_hle_target_context_rerun.json)
(SHA-256 `0b5078fecb0cbf3b67d4132bc1a25ad8055b21136598d85c587859e2c381530b`).
