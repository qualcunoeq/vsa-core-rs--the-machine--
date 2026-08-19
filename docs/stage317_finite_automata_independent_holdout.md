# Stage 317 — independent finite-automata holdout

- Cases: 100 (60 supported, 20 ambiguous, 20 refused)
- Exact decisions: 100/100
- Supported artifacts: 60/60
- Replay verified / tamper rejected: 60/60
- False authorizations / denials: 0 / 0
- Development generator reused: false
- Live registry mutations / HLE questions read: 0 / 0

The holdout uses separately authored parity, modular-counting, last-symbol, alternating, counter, and cyclic transition patterns. Its reference executor is independent of the Stage 314 development generator.
