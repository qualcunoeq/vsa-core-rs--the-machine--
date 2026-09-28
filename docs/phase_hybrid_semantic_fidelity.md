# Semantic fidelity and shadow worker connection (Phase 6)

Earlier phases established structural validation (is a proposal well formed?)
and typed capabilities (can we solve it?). Phase 6 adds semantic fidelity: does
a proposal faithfully represent the source sentence?

A proposal can be structurally perfect and still be wrong. The validator checks
schema, evidence spans, and symbol scope closure; it does not check that an
equation means what the sentence says. So `src/semantic_fidelity.rs` measures
fidelity independently, against a human-reviewed gold interpretation, and keeps
structural validity and semantic fidelity as separate reported numbers.

## Evaluation set

Eight labelled categories: correct, reversed relationship, wrong sign, wrong
quantity, missing condition, invented equation with a valid span, multiple
plausible, unsupported domain. The frozen corpus is
`data/semantic_fidelity_gold_v1.json`; a coverage corpus of prose word problems
(`data/semantic_fidelity_coverage_v1.json`) measures language coverage
separately from solver accuracy.

## Shadow connection

`src/semantic_shadow.rs` decodes a stored worker proposal, runs the existing
deterministic gate, lowers accepted proposals through
`semantic_handoff::lower_candidate_to_equation`, and feeds the resulting
`EquationProblemBinding` to `equation_classification` +
`route_classified_equation`. This is a shadow path: no user-visible answer is
produced, every record carries `downstream_authorized = false`, and the worker
configuration, raw output, validation diagnostics, fidelity verdict, and
replay mode are all preserved for audit.

Interpretation accuracy (fidelity) and solver accuracy (routing) are separate
fields of `ShadowReport`. Stored-output replay (re-decoding recorded bytes) is
labelled apart from model regeneration.

## Policy and clarification

A fail-closed `ShadowPolicy` maps each verdict to `Proceed`, `Clarify`, or
`Reject`. Only a faithful reading proceeds. Ambiguous readings present a short
clarification derived from the candidate's own relation, for example
`Do you mean that b = a + 3?`. If model-assisted wording is added,
`guard_answer_wording` rejects any displayed numeric or equation-shaped claim
that the structured result does not support.

## Result

Complete when the worker improves independently measured language coverage
while meeting an explicit wrong-answer limit. Measured offline
(`cargo run --bin semantic_fidelity_eval`):

* interpretation accuracy 1/8 on the labelled set (only the faithful case is
  labelled correct; the other seven are caught as named infidelities);
* coverage 0/3 -> 3/3 (lift 3) on prose word problems the deterministic binder
  abstains on;
* silent wrong answers 0, against an explicit limit of 0;
* downstream authorizations 0; replay mode `stored_output_replay`.

The evaluator fails its process if the wrong-answer limit is exceeded, if any
downstream authorization appears, or if coverage does not improve.
