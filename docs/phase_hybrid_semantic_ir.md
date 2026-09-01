# Hybrid semantic proposal boundary

This milestone adds the model-independent boundary required before connecting
local neural parser workers. A worker emits a versioned `CandidateSemanticParse`
containing target, operation, symbols, scopes, relations, assumptions, domain
hints, candidate pack, unresolved alternatives, evidence spans, and
reproducibility metadata.

The deterministic validator returns exactly one of:

* `AcceptCandidate` — the proposal is structurally complete;
* `PreserveAmbiguity` — alternatives or scope conflicts remain;
* `RejectCandidate` — required metadata, spans, or bindings are invalid.

Model confidence is recorded for diagnosis but is never consulted for
authorization. Every candidate and validation receipt has a SHA-256 replay
hash. The receipt always has `downstream_authorized = false`; solver selection,
answer authorization, registry mutation, and world-model updates remain
outside this boundary.

Candidate ensembles use the same fail-closed rule: exactly one complete member
with all other members rejected may be selected. Multiple complete members or
any unresolved member produce `PreserveAmbiguity`; an empty or wholly invalid
ensemble is rejected.

Validation evidence:

* 5 focused tests pass;
* low-confidence but structurally valid proposals are accepted;
* duplicate symbol scopes preserve ambiguity;
* invalid evidence spans are rejected;
* unresolved model alternatives are preserved;
* candidate tampering invalidates replay.
* ensembles select one surviving proposal or preserve ambiguity.

This is a semantic IR and validator only. No model runtime, GPU worker, live
registry, or production route is changed by this milestone.
