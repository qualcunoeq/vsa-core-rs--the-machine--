# Stage 180 — autonomous source-catalog acquisition

| Measure | Baseline | Shadow-admitted |
|---|---:|---:|
| Cases | 1000 | 1000 |
| Authorized answers | 0 | 600 |
| Sealed exact / authorized | 80 / 0 | 200 / 120 |
| Sealed learning delta | — | 120 |
| Replay / tamper | 1000 / 1000 | 1000 / 1000 |
| False authorizations / denials | 0 / 0 | 0 / 0 |

| Acquisition gate | Result |
|---|---:|
| Source records validated | 5/true |
| Inferred artifact IDs | 5 |
| Source mutations rejected | 6/6 |
| Development gap observations | 360 |
| Selected plan coverage | 360 |
| Sealed outcomes exposed | 0 |
| Manifest / registry mutations | 0 / 0 |

The domain module identity and provided artifact key were derived from the validated source catalog. Execution used the generic formula runtime; no domain-specific executor branch or live promotion was used.
