# Stage N: sealed curriculum learning curve

This benchmark is independently generated and does not read HLE. The corpus is partitioned into 600 development, 200 validation, and 200 sealed cases; the sealed partition is evaluated only after the source-gated candidates are fixed.

- Corpus SHA-256: `e296e904839d771f2e260450831bbedfb67395b517e509e08ab2e2d8808ef1c7`
- Cases: 1000 (development 600, validation 200, sealed 200)
- Source-gated modules admitted: `source_derived_finite_statistics, source_formula_sequences`
- Final exact decisions: 200/200
- Final correct authorizations: 120
- Final unmet supported cases: 0
- Ambiguity preserved: 40
- Unsupported refusals: 40
- Replay verified: 200
- Tamper rejected: 200
- False authorizations: 0
- False denials: 0
- HLE questions read: 0
- Production registry mutations: 0

The learning curve is measured at baseline, after statistics admission, andafter statistics plus sequences admission. Every admission is gated by sourceprovenance, independent exercises, boundary refusals, replay, and tamper checks.
