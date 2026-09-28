# Phase 6 — semantic-fidelity evaluation

Offline replay of the frozen stored proposals against the frozen gold corpus. Stored-output replay is a re-decode of recorded bytes, not a model regeneration.

* records: 8
* structurally accepted: 8
* interpretation accuracy: 1/8 (12.5%)
* solver accuracy (faithful cases only): 1/1
* infidelities / structurally unusable: 7 / 0
* verdicts proceed / clarify / reject: 1 / 2 / 5
* silent wrong answers: 0 (limit 0)
* downstream authorizations: 0
* replay mode: stored_output_replay
* coverage (baseline -> worker): 0/3 -> 3/3 (lift 3)
* coverage faithful answers: 3 (silent wrong 0)
* example clarification: Do you mean that b = a + 3?
* records SHA-256: `808e49b3dff47a086500df9f2146753a98ba4d63b80f1a4c94f44e68fbabf5e0`
* report SHA-256: `7a2fdfe4919e8c58a30c93f70a885d9d9e46b8b6f3fb119d88393f095795a01d`
