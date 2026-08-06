# Phase 65 — HLE curriculum frontend obstruction audit

This diagnostic audit examines the 705 HLE questions with at least one broad
signal for a validated curriculum pack. It assigns one primary signal and
identifies the first missing field in that pack's typed frontend contract.
Overlapping signals remain recorded per row, but are not double-counted in the
primary obstruction totals.

The audit distinguishes incidental terminology, target/object construction,
symbol binding, dimensions/domains, missing assumptions, unsupported operators,
theorem depth, and cases that are theoretically formalizable. It never invokes
or promotes a curriculum pack.

It also reconstructs compatibility replay for the two baseline authorized
answers by running the current router twice and requiring stable answer output.
This is explicitly a compatibility procedure, not a claim that historical
replay receipts were recovered.

Run:

```text
cargo run --bin hle_curriculum_frontend_audit
```

The per-question trace and summary are written under `/tmp` by default.

## Frozen result

The machine-readable summary is [phase65_hle_curriculum_frontend_audit.json](phase65_hle_curriculum_frontend_audit.json).

| Metric | Result |
|---|---:|
| Audited questions | 705 |
| Signal occurrences | 848 |
| Complete formalization candidates | 36 |
| Signal incidental | 245 |
| Domain/dimensions missing | 229 |
| Assumptions absent | 106 |
| Theorem beyond pack boundary | 33 |
| Symbol binding unresolved | 30 |
| Target artifact not identifiable | 16 |
| Requested operation unsupported | 10 |
| Compatibility replays reconstructed | 2/2 |
| Compatibility replay failures | 0 |
| Pack invocations | 0 |
| False authorizations | 0 |

The two baseline authorized answers were rerun twice under the compatibility
procedure and produced stable answers. This closes the denominator gap without
claiming that the original historical replay receipts were recovered.
