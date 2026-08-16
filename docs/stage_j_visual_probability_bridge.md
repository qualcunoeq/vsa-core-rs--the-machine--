# Stage J: visual table to finite-probability composition

This campaign composes the coordinate-preserving visual table frontend with
the finite exact probability pack. Authorization requires an exact two-column
header (`outcome`, `probability`), rational probability cells, and a normalized
finite distribution. Numeric-looking columns with unknown semantics,
continuous-density headers, extra columns, decimal values, and non-normalized
probabilities are refused or kept ambiguous.

| Measure | Result |
|---|---:|
| Cases | 240 |
| Supported / ambiguous / refused | 120 / 40 / 80 |
| Exact decisions | 240/240 |
| Supported authorizations | 120/120 |
| Visual replay | 240/240 |
| Bridge artifacts emitted | 240/240 |
| Bridge replay (emitted) | 240/240 |
| Visual tamper rejection | 240/240 |
| Bridge tamper rejection (emitted) | 240/240 |
| False authorizations / denials | 0 / 0 |

This is a shadow-only route. The bridge preserves coordinate provenance and
delegates distribution validation to the existing finite-probability pack; it
does not infer probability semantics from visual layout alone.

Reproduction manifest:

* schema: `stage-j-visual-probability-bridge-v1`
* corpus SHA-256: `3f071d6659c65b4013471bd765685a278ddcb6822b4aa8921f3528bf89d7f4cb`
* machine-readable output: `docs/stage_j_visual_probability_bridge.json`
