# Phase 25 — Fractional quantity reasoning

This phase adds a bounded fractional-quantity vertical selected from the
remaining GSM8K failure clusters.  It accepts only explicit fractions applied
to known numeric quantities:

- fraction-of-quantity operations;
- remainder after removing an explicit fraction;
- one equal part of a known quantity.

It rejects percentages, probability, symbolic unknowns, compound growth, and
missing or ambiguous fraction specifications.

## Evidence

```text
RUSTFLAGS='-Awarnings' cargo run --release --quiet --bin fractional_quantity_bench -- data/fractional_quantity_v1.json
```

```text
cases=29
structural=29/29
accepted=17
replayed=17
ambiguous=7
unsupported=5
results=17/17
rewrite_pairs=2/2
false_auth=0
false_denials=0
failures={}
```

Accepted artifacts cross the existing QuantityRelation and algebra bridge and
are replay-verified.  The fractional vertical is not yet wired into global
routing; external reclassification should follow a separate candidate release
and leakage evaluation.
