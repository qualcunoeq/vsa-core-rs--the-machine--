# Stage H: source-derived chemistry technical-language frontend

The bounded chemistry frontend is a shadow-only semantic handoff from narrow
technical-language forms to the source-derived chemistry pack. It accepts
explicit local formula, balanced-reaction, and stoichiometric-ratio cues; it
preserves ambiguity for multiple candidates and refuses broader chemistry
semantics such as molar mass, oxidation state, equilibrium, and solution
chemistry.

The independently authored shifted-language corpus contains 240 cases:

| Outcome | Cases |
| --- | ---: |
| Supported | 120 |
| Ambiguous | 40 |
| Unsupported | 80 |

The corpus hash is
`ec6400a1b89e1e85b4b6655b2081692e0607a7e17544a0f05681cc0955e8e2ee`.

## Results

| Check | Result |
| --- | ---: |
| Exact status decisions | 240/240 |
| Complete frontend requests | 120/120 |
| Downstream chemistry authorizations | 120/120 |
| Frontend replay verification | 240/240 |
| Downstream replay verification | 120/120 |
| Frontend tamper rejection | 240/240 |
| Downstream tamper rejection | 120/120 |
| Provenance preserved | 240/240 |
| False authorizations | 0 |
| False denials | 0 |

No production registry, router, or live authorization path is changed by this
frontend. A complete frontend request is necessary but not sufficient for any
future capability beyond the validated chemistry pack route.
