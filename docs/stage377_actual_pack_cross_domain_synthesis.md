# Stage 377 — actual-pack cross-domain synthesis

- cases: 1000 (800 supported, 100 ambiguous, 100 refused)
- exact decisions: 1000/1000
- supported intermediate artifact entries: 1920
- replay verified / tamper rejected: 1000/1000
- false authorizations / denials: 0 / 0
- route leakage: 0
- route counts: `{"CombinatoricsProbability": 160, "GraphLinearAlgebra": 160, "GraphProbabilityMarkov": 160, "NumberTheoryAlgebra": 160, "ProbabilityLinearAlgebra": 160}`
- failure localization: `{"ambiguous route or target": 100, "unsupported domain or specialist spectral operation": 100}`
- corpus SHA-256: `67109018e955d6492a04a5c0bc6dcd89e620d3859a562969df86c9ecfee8c75e`

This controlled deterministic corpus calls the actual graph, probability, linear-algebra, finite-Markov, combinatorics, number-theory, and abstract-algebra pack APIs. Supported cases require typed handoffs; ambiguous and refused cases are executed through real pack boundaries and remain closed. It is stronger than the earlier fake-artifact route test, but it is not the sealed natural-language external exam and should not be reported as such.

Reproduce with `cargo run --quiet --bin stage377_actual_pack_cross_domain_synthesis`.
Machine-readable report: `docs/stage377_actual_pack_cross_domain_synthesis.json`
