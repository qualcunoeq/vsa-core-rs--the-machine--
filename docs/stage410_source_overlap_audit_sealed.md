# Stage 410 — source overlap audit

- Partition / questions: sealed / 1000
- Legacy routes / unique candidates: 12 / 4
- Cumulative unique / ambiguities: 4 / 0
- Source-selection replays / failures: 3 / 0
- Answer keys / production authorizations / false authorizations: 0 / 0 / 0
- Manifest unchanged: true

Candidate lineages:

- FiniteSet: selected=true frontend_complete=1 execution_complete=0 replay=1000/0
- TruthTable: selected=true frontend_complete=0 execution_complete=0 replay=1000/0
- LinearInterpolation: selected=true frontend_complete=0 execution_complete=0 replay=1000/0

This answer-key-blind audit measures external reachability only; it does not score or authorize candidates.
