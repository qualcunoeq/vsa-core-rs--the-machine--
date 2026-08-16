# Stage A: ODE composition gate

The bounded scalar ODE pack composes with existing calculus and classical
mechanics only through explicit semantic bridges:

* a constant derivative may be differentiated as a continuous expression;
* a declared derivative of velocity may be bound as acceleration for Newton's
  second law with mass and units supplied explicitly.

The benchmark refuses to treat numerical approximation as exact calculus,
convert continuous ODEs into discrete dynamics, or infer mechanics semantics
from an arbitrary ODE. Ambiguous derivative/frame interpretations stop before
execution.

The independent corpus is recorded in
`docs/stage_a_ode_composition.json` with SHA-256
`1873a579cc53bc9c15b9a7dc43ef7faa22f4f9a11c4c62520243309a393a0f5d`.

| outcome | cases |
| --- | ---: |
| supported compositions | 120 |
| ambiguous bridges | 40 |
| refused bridges | 80 |
| exact decisions | 240/240 |
| replay verified | 240/240 |
| tamper rejected | 240/240 |
| false authorizations | 0 |
| false denials | 0 |

This remains a shadow curriculum gate; no production route or HLE holdout was
changed.
