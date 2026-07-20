# Prompt-grounded recurrence layer

`src/recurrence.rs` is a deliberately narrow execution substrate. It accepts
only a typed, prompt-grounded first-order explicit affine recurrence:

```text
a[n+1] = c * a[n] + b
```

The definition preserves the sequence identity, index variable and domain,
quantification, initial conditions, and source provenance. Execution is
bounded by `RecurrenceContract`, uses checked exact arithmetic from the algebra
island, and emits one replay-verifiable receipt for every unrolled step.

This module does not parse arbitrary recurrence prose, infer a recurrence from
listed examples, solve closed forms, analyze convergence, or authorize a
nonlinear recurrence. Those are separate representation capabilities.

## Review outcome

The four rows grouped by the heuristic miner were manually reviewed in
`recurrence_candidate_reviews_20260720.md`. They are not one executable
method family:

- one is a nonlinear rational-map threshold problem;
- one is nonlinear ODE stability analysis;
- one asks for closed-form sequence pattern inference;
- one is arithmetic-sequence modeling and algebra.

Therefore the recurrence registry remains empty and no HLE answer route is
enabled by this layer. The typed executor is tested only on synthetic affine
definitions until a real HLE cluster passes the same contract gates.
