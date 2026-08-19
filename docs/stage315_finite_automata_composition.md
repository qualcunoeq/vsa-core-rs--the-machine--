# Stage 315 — finite-automata composition

- Cases: 240 (120 supported, 40 ambiguous, 80 refused)
- Exact decisions: 240/240
- Invariants preserved: 120/120
- Equivalent routes: 120/120
- Replay verified: 240/240
- Tamper rejected: 240/240
- False authorizations / denials: 0 / 0
- Route leakage: 0
- Live registry mutations / HLE questions read: 0 / 0

Complete binary DFAs lower to labelled graphs or execute as bounded traces. State order, alphabet labels, initial state, accepting states, and trace semantics remain attached. Numeric matrices, nondeterminism, epsilon transitions, minimization, language equivalence, and over-budget execution are refused.
