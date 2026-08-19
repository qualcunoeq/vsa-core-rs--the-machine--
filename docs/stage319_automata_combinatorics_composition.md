# Stage 319 — automata/combinatorics composition

- Cases: 240 (120 supported, 40 ambiguous, 80 refused)
- Exact decisions: 240/240
- Supported count artifacts: 120/120
- Replay verified / tamper rejected: 240/240
- False authorizations / denials: 0 / 0
- Live registry mutations / HLE questions read: 0 / 0

The composition emits exact count-by-length traces for bounded binary DFAs. It refuses asymptotic growth, infinite-language claims, nondeterminism, nonbinary alphabets, missing horizons, and over-budget lengths.
