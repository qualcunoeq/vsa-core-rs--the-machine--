# Stage 437 — regression route integration diagnostic

The source-derived regression frontend was connected to the route-blind
portfolio after its independent Stage 436 validation. The 3,000-question
development scan remained unchanged: no naturally authored prompt reached a
unique executable regression route.

- portfolio routes: 13
- development questions / route invocations: 3000 / 39000
- unique candidates / ambiguities / no route: 23 / 0 / 2977
- correct / rejected hash-aligned candidates: 23 / 0
- candidate replay: 23 / 23
- frontend replay / tamper: 39000 / 39000
- execution replay / tamper: 24 / 24
- false authorizations: 0
- plaintext answers / sealed questions read: 0 / 0
- manifest unchanged: true
- probe report SHA-256: `be1f57812200fbb373d81914b7d76fd120b5973c94fea279a11c35d724dc4e7f`
- score report SHA-256: `1c5e92abd8e28715a4b9e2c1612c98a32c59b03d2a078256dd5fe2a374edb3de`

This is a transfer diagnosis, not a claim of regression coverage. The route
remains shadow-only.
