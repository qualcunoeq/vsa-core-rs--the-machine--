# Phase 46 — HLE requested-target ambiguity audit

The four cases that remained ambiguous after the Phase 45 parenthesis repair
were audited at the requested-target layer. No parser, solver, router, or
authorization behavior changed.

Results:

* 4/4 residuals reproduced;
* 2 cases share `natural_language_property_target`;
* 2 cases share `non_ascii_target_symbol_normalization`;
* requested artifact types are distinct but structurally explicit;
* 0 parser changes;
* 0 authorization changes.

The two natural-language cases ask for a classification group or a minimal
value using prose operations that are not yet represented as target artifacts.
The two notation cases ask for derived expressions involving Greek/non-ASCII
symbols (`chi` and `alpha + beta`) that are not bound to a single requested
answer expression.

Both mechanisms have repeated evidence, but this audit does not implement a
target-grounding extension. The next safe experiment should use an independent
corpus to validate target-operation and notation-target contracts before these
HLE cases are reconsidered.

Artifact: [`phase46_hle_target_ambiguity_audit.json`](phase46_hle_target_ambiguity_audit.json)
(SHA-256 `604ec11fcbac7a927a420600c83b24b5ee3dfbd99ae5d592887156f7a3ebaed8`).
