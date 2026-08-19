# Stage 314 — source-derived bounded finite automata

- Source records: 1/5 validated
- Cases: 240 (120 supported, 40 ambiguous, 80 refused)
- Exact decisions: 240/240
- Supported trace artifacts: 120/120
- Replay verified: 240/240
- Tamper rejected: 240/240
- Provenance preserved: 120/120 emitted artifacts
- False authorizations / denials: 0 / 0
- Runtime case-specific branches: 0
- Live registry mutations / HLE questions read: 0 / 0

The shadow pack is derived from finite-automata definitions and executes only complete binary DFAs with explicit state and word budgets. Nondeterministic, epsilon, regular-expression, infinite-state, invalid, and over-budget requests remain refused.
