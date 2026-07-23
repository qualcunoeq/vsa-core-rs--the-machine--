# Phase 10: Prose Recurrence Vertical

The repository already contained a typed exact affine-recurrence island. This
phase adds a conservative prose parser and connects it to the router without
opening the generic algebra fallback.

## Supported contract

The parser accepts only:

- an explicit indexed initial condition (`a_0 = c` or `a_1 = c`);
- a first-order affine rule (`a_(n+1) = p*a_n + q`);
- a numeric `n = k` target;
- exact checked integer arithmetic within the bounded unroll budget.

It abstains on missing or ambiguous indices, contradictory definitions,
nonlinear and higher-order recurrences, closed-form requests, and malformed
rules. Every accepted step is replayed, and tampered receipts are rejected.

## Independent corpus

`data/recurrence_ood_v1.json` contains 500 independently generated cases:

- 150 valid affine evaluations (including 50 shifted-index rewrite pairs);
- missing initial conditions;
- ambiguous indexing;
- nonlinear and higher-order recurrences;
- conflicting definitions;
- malformed recurrence statements.

The oracle and corpus generator live in
`scripts/generate_recurrence_ood.py`; the evaluator is
`prose_recurrence_benchmark` and can be run with:

```bash
cargo run --release --bin prose_recurrence_bench -- \
  data/recurrence_ood_v1.json /tmp/recurrence_ood_v1_report.json
```

## Result

```text
cases=500
authorized=150
correct_answers=150
replay_verified=150
tampered_receipts_rejected=150
false_authorizations=0
false_denials=0
rewrite_pairs=50
rewrite_decision_regressions=0
rewrite_answer_regressions=0
```

The mixed 1,000-case integration corpus also remains clean after replacing
the previous recurrence abstention slice with recurrence positives:

```text
route=1.000
decisions=1.000
false_auth=0
false_denials=0
authorized=720
replay=720
rewrite_regressions=0
```

These results establish bounded governed competence for the tested grammar,
not universal recurrence solving.
