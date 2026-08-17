# Stage 141 — Frontend scope repair

The Stage 140 corpus was rerun after repairing scope overreach in the bounded
arithmetic-function and elementary-number-theory frontends.  Operation aliases
are now grouped by semantic family, repeated local bindings remain ambiguous,
and visible formulas from a competing arithmetic-function family prevent a
number-theory request from becoming a complete route.

## Frozen evidence

- Corpus: independent Stage 140 embedded-notation and multi-scope corpus
- Corpus SHA-256: `97c22dd23c8bc6e33990c465791d746887e1fc7869bcf7d0f26d02eeda6ea23b`
- Cases: 240
- Repair run: `STAGE140_REPAIRED=1`

## Results

| Measure | Result |
|---|---:|
| Exact decisions | 240/240 |
| Frontend replay | 240/240 |
| Tamper rejection | 240/240 |
| Overbroad completions | 0 |
| False authorizations | 0 |
| False denials | 0 |
| Production registry mutation | 0 |

The original Stage 140 pressure run recorded 80 overbroad completions and 40
shadow false authorizations.  The repaired run eliminates both without
changing the live router or authorizing from frontend completion alone.

The machine-readable receipt is
`docs/stage141_frontend_scope_repair.json`.
