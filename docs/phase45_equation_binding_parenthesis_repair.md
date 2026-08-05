# Phase 45 — structural parenthesis/function repair

Phase 45 adds a narrow structural repair to `EquationProblemBindingV1`. A
parenthesized expression now retains candidate interpretations rather than
being treated as function application solely because it matches `name(...)`.

The bridge records `ParenthesizedCandidate` artifacts with:

* grouping, tuple, operator-argument, or function-application form;
* head and body text;
* source spans and evidence;
* whether the head was explicitly declared.

Function-domain ambiguity is triggered only by explicit or bounded function
evidence: a declared function, a bounded named operator, function language, or
a lowercase function definition such as `f(x) = ...`. Uppercase forms such as
`A(X) = ...` are not treated as functions by naming convention alone.

## Regression results

The frozen Phase 44 corpus remains unchanged:

* 120/120 structural decisions;
* 120/120 replay verified;
* 0 incorrect symbol or target bindings;
* 0 downstream authorizations.

The six specialist-semantic HLE ambiguities remain protected. The four
repeated cases now expose non-function structural forms (grouping or tuple)
instead of receiving the generic function-domain explanation. No solver,
router, registry, or authorization path was changed.

## Scope

This is a parser-boundary repair, not a domain expansion. The six remaining
cases require genuine operator or domain semantics and remain diagnostic
residuals. The next test should be an independent parenthesis corpus covering
explicit functions, grouping, tuples, intervals, operator arguments, nested
forms, and undeclared symbols before any further binder broadening.

Audit artifact: [`phase45_equation_binding_ambiguity_audit.json`](phase45_equation_binding_ambiguity_audit.json)
(SHA-256 `59daafabd53661b925debb843d8b77f32d08665f5921234884dcf27b41874d6f`).
