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

