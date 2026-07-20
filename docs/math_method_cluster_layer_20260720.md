# Mathematical method-shape mining

The method registry now has a diagnostic companion in
`src/math_method_mining.rs`. It aggregates reviewed benchmark annotations by
the transformation that a question needs, rather than by a theorem name or a
route label:

```text
definition instantiation
direct theorem instantiation
identity application
finite case reduction
recurrence unrolling
invariant application
bound application
transform and evaluate
classification lookup
constructive search
proof by contradiction
```

Each annotation records whether premises, definitions, and side conditions
are explicit, what verification is available, the estimated number of method
steps, and the representation cost. `MethodClusterReport` ranks clusters by
eligible evidence first, then structural compatibility and verification. The
ordering is deterministic and does not depend on registry insertion order.

The eligibility gate is intentionally narrow: a question must be structurally
compatible, have explicit premises and definitions, have extractable side
conditions, support at least replay verification, require one step, and have
low or medium representation cost. This is a pack-selection signal only; it
never grants execution authority.

`to_markdown()` produces a reviewable report. A future pack must meet a minimum
question count and verification level, then pass shadow retrieval,
instantiation, mutation, and static-schema checks before it can be registered
for execution. Until that evidence exists, the runtime remains pack-empty.

The module has no prompt matching, embeddings, CAS calls, theorem execution, or
answer formatting. This keeps reconnaissance safe and prevents a high
retrieval score from becoming an unsupported answer.
