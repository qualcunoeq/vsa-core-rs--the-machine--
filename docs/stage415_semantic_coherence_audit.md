# Stage 415 — semantic-coherence audit

- source records / clean / rejected: 5566 / 2105 / 3461
- signature memberships / overlap / unclassified: 1109 / 287 / 1283
- answer keys read / production authorizations / false authorizations: 0 / 0 / 0
- sealed manifest records: 1091 (prompt text not consumed)
- manifest unchanged: true

| Signature | Candidates | Dev | Val | Sources | Recoverable | Structurally repeated |
|---|---:|---:|---:|---:|---:|:---:|
| `linear_system_solve` | 92 | 73 | 19 | 6 | 48 | true |
| `linear_programming` | 7 | 6 | 1 | 1 | 4 | false |
| `inequality_constraints` | 39 | 26 | 13 | 3 | 11 | false |
| `line_slope_equation` | 23 | 18 | 5 | 8 | 13 | true |
| `linear_word_problem` | 562 | 445 | 117 | 10 | 146 | false |
| `triangle_area` | 18 | 16 | 2 | 5 | 10 | true |
| `triangle_side_or_perimeter` | 19 | 15 | 4 | 6 | 7 | false |
| `circle_measurement` | 34 | 26 | 8 | 7 | 18 | true |
| `rectangle_measurement` | 23 | 12 | 11 | 4 | 13 | true |
| `trapezoid_measurement` | 10 | 7 | 3 | 1 | 0 | false |
| `volume_measurement` | 20 | 17 | 3 | 5 | 4 | false |
| `surface_area` | 12 | 7 | 5 | 4 | 5 | false |
| `coordinate_geometry` | 89 | 74 | 15 | 9 | 14 | false |
| `angle_trigonometry` | 161 | 119 | 42 | 8 | 70 | false |

`structurally_repeated` is a screening signal only; it is not a capability contract or semantic proof. Any selected family requires independent exercises, boundary cases, and downstream validation.
