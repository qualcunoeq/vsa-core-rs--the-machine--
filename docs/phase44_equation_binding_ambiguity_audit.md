# Phase 44 follow-up — HLE binding-ambiguity evidence audit

This audit examines the ten frozen HLE cases that the Phase 44 shadow binder
classified as ambiguous. It does not alter `EquationProblemBindingV1`, solver
routing, or authorization.

Results:

* 10/10 ambiguous cases reproduced;
* 4 cases share a repeated, potentially recoverable mechanism:
  `overbroad_parenthesis_function_detection`;
* 6 cases require domain-dependent function or operator semantics;
* 4/4 repeated-mechanism cases are recoverable in principle from stronger local
  evidence, without adding a specialist solver;
* binder changed: false.

The repeated mechanism is not permission to broaden the parser. It is an
evidence-gathering target: function-domain gating should only be reconsidered
after an independent corpus tests explicit function declarations against
incidental parentheses, operator notation, and quoted expressions.

The six remaining cases include materially domain-dependent notation (such as
correlations, PDE functions, parametric models, or Schur-function operators).
They remain ambiguous until domain/codomain or operator semantics are supplied
by local evidence. No specialist method is inferred from these cases.

Audit report: [`phase44_equation_binding_ambiguity_audit.json`](phase44_equation_binding_ambiguity_audit.json)
(SHA-256 `a4672949d23e4cfbee124a8eb39a1ee28a5b14d554fd24bfc3acfbf69f224ae7`).

This closes the requested evidence audit. The next experiment, if pursued,
should be an independent ambiguity corpus for the repeated parenthesis/function
mechanism—not a binder expansion against the HLE questions themselves.
