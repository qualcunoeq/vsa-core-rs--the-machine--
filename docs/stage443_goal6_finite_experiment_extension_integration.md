# Stage 443 — finite-experiment extension integration diagnostic

The validated finite-experiment extension was evaluated through the route-blind
Goal 6 portfolio on the development partition only. No answer keys or sealed
questions were read, and the curriculum manifest was unchanged.

## Development result

- questions / routes / invocations: `3000 / 15 / 45000`
- unique candidates / ambiguities / no executable route: `24 / 0 / 2976`
- finite-die candidates / execution replay: `1 / 1`
- frontend replay / tamper rejection: `45000 / 45000`
- execution replay / tamper rejection: `25 / 25`
- production authorizations / false authorizations: `0 / 0`
- manifest unchanged: `true`

On the lexical `die`/`dice` subset of 41 development prompts, the finite-die
route classified `1` complete, `13` ambiguous, `8` missing, and `19`
unsupported. The newly supported independent forms (separate first/second
conditions and explicitly labeled octahedral pairs) remain ambiguous on the
natural prompts where fairness or uniformity is unstated. This is a transfer
diagnosis, not a reason to infer uniformity or authorize additional answers.

## Provenance

- portfolio report SHA-256:
  `9ee2d5b5804d6126e4e0ffb68e1080aa54c05299982fe42be3857307ae3fa5a5`
- source frontend SHA-256:
  `dd6ec9441201db541a19c2900f2acff8032e6a881cef3f8233f0cb5ced706b44`
- Stage 441 extension corpus SHA-256:
  `9758e3f730da7917d5a4ac279a29345b5c4e24090ffd14ba3ca05f0ee7ff8a83`
- sealed data touched: `false`
- registry mutated: `false`

