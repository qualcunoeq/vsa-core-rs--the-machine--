# Stage G: sealed curriculum examination

The planner was given only 500 held-out question texts. It selected a bounded
route, requested prerequisite closure from the immutable curriculum DAG, and
executed only validated pack operations. Expected classifications remained in
the scorer and were not available to planning.

| metric | result |
| --- | ---: |
| sealed questions | 500 |
| correct supported authorizations | 400/400 |
| preserved ambiguities | 50/50 |
| safe refusals | 50/50 |
| prerequisite plans | 400 |
| replay verified | 500/500 |
| false authorizations | 0 |
| manifest changed | false |

The exact sealed corpus hash and run receipt are recorded in
`docs/stage_g_sealed_curriculum_exam.json`. This is a bounded curriculum
holdout, not an uncontrolled natural-language or HLE result.
