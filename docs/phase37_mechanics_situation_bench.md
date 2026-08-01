# Phase 37 — MechanicsSituationV1 shadow formalizer

Phase 37 adds a bounded structural handoff between mechanics language and the
externally grounded classical-mechanics pack:

```text
mechanics situation
→ typed quantities and requested target
→ candidate-law applicability
→ assumption and domain checks
→ shadow pack execution
```

The implementation is not wired to HLE or production routing. It preserves
candidate alternatives, unresolved assumptions, provenance spans, and replay
hashes instead of guessing a law from broad vocabulary.

## Reproducibility

```text
cargo test --lib mechanics_situation --quiet
cargo run --quiet --bin mechanics_situation_bench -- \
  docs/phase37_mechanics_situation_bench.json
```

The frozen independent corpus and per-case results are in
[`phase37_mechanics_situation_bench.json`](phase37_mechanics_situation_bench.json).
Report SHA-256:

```text
9fb8492a4c56ac59b16a9ef741e9dfde15829d24b14fa8881008fe8b3bde2e77
```

## Corpus and results

The corpus has 240 cases:

* 160 direct supported situations across the five Phase 34 laws;
* 20 generic-energy ambiguities;
* 20 missing-assumption cases;
* 20 unsupported-domain cases (relativistic/rotational);
* 20 multi-law composition cases.

| Metric | Result |
| --- | ---: |
| Exact status decisions | 240/240 |
| Exact law decisions on uniquely supported cases | 160/160 |
| Exact supported values | 160/160 |
| Situation replay | 240/240 |
| Execution replay | 240/240 |
| Pack invocations | 160 |
| Complete pack results | 160/160 |
| Provenance-bearing cases | 240/240 |
| False unique applications | 0 |
| False authorizations | 0 |
| Registry mutated | no |
| HLE routing mutated | no |

The formalizer extracts masses, forces, acceleration, velocity, spring
constant, displacement, requested output, candidate laws, unresolved
assumptions, and source markers. Generic “energy” remains ambiguous; missing
inertial or spring assumptions remain non-unique; unsupported domains are
rejected; multi-law requests do not silently collapse into one route.

This validates the structural handoff and its safety boundary on an independent
deterministic corpus. It does not claim robust natural-language mechanics or
HLE coverage. The next step is a shifted, independently authored situation
corpus, followed by a diagnostic HLE rerun only after the situation contract is
frozen.

