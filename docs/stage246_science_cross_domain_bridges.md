# Stage 246 — science cross-domain bridges

This stage composes the bounded science capabilities with validated
mathematical artifacts while preserving semantic labels and explicit policy.

Results:

* chemistry: 100/100 molecular artifacts became labeled element-count vectors;
  20 stoichiometric-ratio artifacts were refused by the linear bridge;
* chemistry bridge replay/tamper checks: 120/120 each;
* biology: 100 base-composition artifacts crossed under the explicit
  `uniform_position` policy, while 34 of 50 policy controls refused and 16
  valid uniform controls crossed;
* biology bridge replay/tamper checks: 150/150 each;
* 100/100 probability handoffs from base counts were exact, replayed, and
  tamper-rejected;
* 270 bridge inputs, 316 exact authorized artifacts, zero semantic leakage,
  false authorizations, false denials, or live mutations.

The bridges do not infer probability from arbitrary DNA counts, treat a
stoichiometric ratio as an element vector, or erase chemistry/biology
provenance during mathematical execution.

Corpus hash:
`87c6d5f4c2d267e341669a91c0f26d0a968140e0b953dbcd0caf0d5ec43d5d9c`
