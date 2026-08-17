# Stage 144 — Mixed frontend route-blind composition

Five bounded technical-language frontends were invoked on every report:
arithmetic functions, elementary number theory, combinatorics, finite
Dirichlet characters, and simplicial homology.  No frontend received the
expected family label, and downstream execution occurred only for a unique
complete route.

## Corpus and results

- 500 supported reports (100 per family)
- 100 ambiguous reports
- 100 unsupported reports
- Corpus SHA-256: `0baa55d91932958458a4acc7deedca1c6c62394abf69913d4cda1f4d55a6f0f4`

| Measure | Result |
|---|---:|
| Frontend invocations | 3,500 |
| Exact route decisions | 700/700 |
| Supported authorizations | 500/500 |
| Ambiguity/unsupported preservation | 200/200 |
| Replay verification | 700/700 |
| Tamper rejection | 700/700 |
| False authorizations | 0 |
| False denials | 0 |
| Route leakage | 0 |

Each supported family produced exactly 100 unique routes; all ambiguous and
unsupported reports produced none.  This validates combinatorics as a
route-blind participant without changing production routing or the sealed HLE
holdout.

Machine-readable report: `docs/stage144_mixed_frontend_route_blind.json`.
