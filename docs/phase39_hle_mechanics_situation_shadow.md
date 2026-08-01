# Phase 39 — Frozen HLE mechanics-situation diagnostic

Phase 39 reruns the frozen 2,500-question HLE release through the shifted,
structural `MechanicsSituationV1` formalizer from Phase 38 and the unchanged
shadow classical-mechanics pack. Production routing and answer authorization
are not called.

## Reproducibility

```text
cargo test --lib mechanics_situation --quiet
cargo run --quiet --bin hle_mechanics_situation_shadow -- \
  docs/phase39_hle_mechanics_situation_shadow.json
```

The input is `data/hle.jsonl`. The report contains one hashed record per
question, the formalizer status and candidate laws, the first failing gate,
pack status, candidate/reference comparison, provenance count, and replay
results.

| Artifact | SHA-256 |
| --- | --- |
| HLE input (`data/hle.jsonl`) | `see report dataset_sha256` |
| Mechanics pack | `see report pack_sha256` |
| Phase 39 report | `f53668c01485846c41af15f8087bbcf256abb2ee57de30889631bbb44a8c8adf` |
| implementation commit | `7676e08` plus this diagnostic binary |

## Funnel result

| Gate | Count |
| --- | ---: |
| HLE questions | 2,500 |
| Mechanics signals detected | 216 |
| Signals inside the current supported subdomain | 191 |
| Complete typed situations | 0 |
| Unique applicable laws | 1 |
| Complete bindings | 0 |
| Pack invocations | 0 |
| Complete pack results | 0 |
| Candidate answers | 0 |
| Reference matches | 0 |
| False authorizations | 0 |
| Situation replay verified | 2,500/2,500 |
| Execution replay verified | 2,500/2,500 |

The one candidate law was not executable because the situation remained
outside the pack boundary or lacked a required assumption. No HLE answer was
authorized and the production baseline remains 2 correct authorized answers.

## First failing gate for mechanics signals

| First failing gate | Count |
| --- | ---: |
| Unsupported mechanics subdomain | 46 |
| Missing required quantities | 14 |
| Missing assumption | 4 |
| Target not groundable | 152 |

These counts are restricted to the 216 questions with a mechanics vocabulary
signal. The complete report also records a classification for all 2,500
questions; non-mechanics questions remain diagnostic `no_mechanics_candidate`
or target-not-groundable records rather than being treated as failures of the
mechanics pack.

## Interpretation

The frozen HLE run produced no pack invocation and no score increase. This is
not evidence of a generic language-grounding regression: Phase 38's shifted
corpus reached 120/120 supported values with zero false unique-law selections.
On HLE, the bottleneck is insufficient overlap with the elementary,
single-body five-law pack and the absence of complete, uniquely grounded
situations. The mechanics signals that do occur are mostly target/quantity
gaps or unsupported subdomains.

The run was shadow-only. The registry, production router, and HLE score were
unchanged. Every formalized situation and shadow execution replayed from its
stored digest.
