# Phase 38 — Shifted-language MechanicsSituationV1 benchmark

Phase 38 tests the situation formalizer on independently authored surface
forms rather than the Phase 37 templates. The corpus contains reordered
clauses, indirect descriptions, irrelevant quantities, implicit targets,
ordinary-force versus net-force traps, scalar/vector ambiguity, multi-body
cases, generic energy, missing assumptions, and unsupported domains.

## Reproducibility

```text
cargo test --lib mechanics_situation --quiet
cargo run --quiet --bin mechanics_situation_shifted_bench -- \
  docs/phase38_mechanics_situation_shifted_bench.json
```

Report SHA-256:

```text
3e2eda2d430cbb1dc5716417380f4ed281769fe3d4bed45284a19952e0408af8
```

## Results

The shifted corpus has 260 cases: 120 supported, 120 ambiguous, and 20
unsupported.

| Metric | Result |
| --- | ---: |
| Exact status decisions | 260/260 |
| Exact law decisions | 120/120 |
| Exact supported values | 120/120 |
| Situation replay | 260/260 |
| Execution replay | 260/260 |
| Pack invocations | 120 |
| Complete pack results | 120/120 |
| Provenance-bearing cases | 260/260 |
| False domain entries | 0 |
| False unique-law selections | 0 |
| False authorizations | 0 |
| Registry mutated | no |
| HLE routing mutated | no |

The negative controls remain conservative: implicit targets, ordinary force
without proof it is net force, scalar magnitudes without direction, multi-body
systems, generic energy, missing assumptions, and unsupported relativistic or
rotational situations do not enter a unique single-law route.

This is still a bounded synthetic/independently authored language corpus, not
uncontrolled HLE prose. It establishes that the formalizer's structural
handoff survives the tested distribution shift. The next step is the
diagnostic HLE rerun with this frozen situation layer, while preserving the
production router and baseline score.

