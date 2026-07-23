# Phase 23 — Unit-aware quantity reasoning

This phase adds a separate, narrow unit-aware vertical after the GSM8K
quantity candidate work.  It accepts only explicit conversion factors and
compatible linear addition/subtraction.  It does not import a conversion
table, infer missing units, or authorize incompatible dimensions.

## Scope

Supported:

- explicit conversions such as `100 centimeters per meter`;
- compatible addition and subtraction with an explicit target unit;
- length, volume, mass, and time units covered by the bounded factor table.

Ambiguous:

- missing target units;
- missing conversion factors;
- unspecified operation or multiple possible operations.

Unsupported:

- incompatible dimensions;
- implicit or “usual” conversions;
- percentages and finance;
- units outside the bounded table.

## Evidence

```text
RUSTFLAGS='-Awarnings' cargo run --release --quiet --bin unit_aware_quantity_bench -- data/unit_aware_quantity_v1.json
```

```text
cases=27
structural=27/27
accepted=15
replayed=15
ambiguous=6
unsupported=6
results=15/15
rewrite_pairs=3/3
false_auth=0
false_denials=0
failures={}
```

Accepted artifacts are handed to the existing algebra executor through an
explicit typed bridge and require replay verification.  This module is not
wired into global routing yet; integration should follow a larger independent
corpus and mixed-domain leakage test.
