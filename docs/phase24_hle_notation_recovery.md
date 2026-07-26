# Phase 24 — shadow equations/expressions notation recovery

Phase 23 identified a high-leverage subset of technical notation. This phase
adds a bounded, shadow-only normalizer for locally scoped equations and
expressions. It emits typed `SymExpr` candidates, symbol bindings, unresolved
binding markers, provenance spans, downstream compatibility, and a replay
receipt. It does not change production routing, authorization, registries, or
the HLE release candidate.

The accepted contract is deliberately narrow:

* inline/display equation or expression regions;
* local symbols and ordinary arithmetic notation;
* nested fractions, exponents, implicit multiplication, and chained prose;
* no visual-layout semantics or external specialist convention.

## Independent corpus

The deterministic corpus contains 80 cases:

| Class | Cases |
|---|---:|
| Supported | 60 |
| Ambiguous | 10 |
| Unsupported | 10 |
| Rewrite groups | 10 |

Result:

* decisions: **80/80**;
* false accepts: **0**;
* false rejections: **0**;
* accepted candidates replayed: **60/60**;
* rewrite regressions: **0/10**.

Corpus SHA-256: `8a7cae38e3bbdae2552464c502f5653763bb2db82d75706f6f41b7326ba765e8`

The report artifact from the actual run is retained outside the repository
and is hashed as:

`01a54095311e4af6f93d713b7999358e9ddf47ac0b7efe3f747b66a638274fb0`

## Matching HLE audit rows

The frozen Phase 23 notation report was filtered to equations/expressions
whose shadow outlook was `likely_normalization_only`. This produced 58
candidate rows:

* accepted normalization candidates: **16**;
* ambiguous: **10**;
* unsupported by the bounded parser: **32**;
* replay-verified accepted candidates: **16**.

These are downstream reclassification candidates, not newly authorized HLE
answers. No HLE answer, route, registry, or production parser was changed.

Phase 23 input audit SHA-256:
`5b9f7f2a49b9e69b8a0d4e1768615a14a34a03446aa68d09741605cf6edc6af6`

## Library-test baseline

The required `cargo test --lib` gate was attempted during this phase. It
reported failures already outside the notation changes, including:

* `capabilities::tests::production_capabilities_are_transformations_only`;
* `failure_taxonomy::tests::complete_typed_target_is_planning_failure_when_direct_audit_abstains`;
* `failure_taxonomy::tests::false_denial_gets_one_stable_dominant_bucket`;
* `formalization_benchmark::tests::seed_corpus_has_stable_holdout_and_taxonomy_metrics`;
* `function_application::tests::explicit_function_application_executes_and_replays`;
* `math::tests::test_explicit_sympy_cas_directive`;
* `physics::tests::test_verified_solve_problem`;
* the existing QA answer/chaining/negation tests;
* `router::tests::test_latex_math_requires_a_complete_standalone_ast`;
* `router::tests::test_typed_math_pipeline_solves_plain_prose_algebra_and_calculus`;
* `tests::test_math_engine_arithmetic_via_qa`.

The run also reached long-running dataset-backed tests and was stopped after
the known failures had been reported. Focused Phase 24 tests passed:

```text
cargo test --lib notation_normalization
cargo test --bin hle_notation_recovery
```

No Phase 24 test failure or production-state mutation was observed.

## Next gate

The 16 HLE normalization candidates require independent semantic adjudication
and downstream answer verification before any promotion. The next contract
should remain within this family and add only the highest-frequency parser
failure supported by a fresh holdout.
