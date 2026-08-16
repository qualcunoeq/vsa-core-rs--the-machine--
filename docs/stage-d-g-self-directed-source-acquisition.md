# Stage D/G: Self-Directed Source Acquisition

This campaign connects the curriculum gap planner to shadow-only source
extraction. It selects a source-backed relation from observed gaps, extracts
the relation into the generic source-relation representation, and validates it
against independent exercises without mutating the live curriculum manifest.

## Result

The benchmark recorded 500 deterministic observations in three gap clusters
and four candidate plans. The planner selected
`source_relation_dna_complement`, covering 300 relation gaps. The selected
plan replayed successfully; a tampered coverage count was rejected.

The selected source document yielded one relation record. Six mutated source
documents were rejected. Generic relation evaluation matched the existing
bounded DNA biology capability on 120/120 independently generated exercises.

| Check | Result |
| --- | ---: |
| Observed gap cases | 500 |
| Gap clusters | 3 |
| Candidate plans | 4 |
| Selected coverage | 300 |
| Independent validations | 120/120 |
| Biology agreement | 120/120 |
| Source mutations rejected | 6/6 |
| Blocked unproven shortcuts | 1 |
| Plan replay / tamper rejection | pass / pass |
| Manifest unchanged | yes |
| Production authorizations | 0 |
| False authorizations | 0 |

The machine-readable record is
[`stage-d-g-self-directed-source-acquisition.json`](stage-d-g-self-directed-source-acquisition.json).
Its source-document SHA-256 is
`9a34afa1464ee6290fc5026d3210457d50ba56fb0e70ca97bcbfb634b527bbc6` and its
corpus SHA-256 is
`0dd28be20ff0c638a0ed300f4e2de9403dd19cb0b0b64636700014fba288c13b`.

## Boundary

The relation is only shadow-promotable after source provenance, independent
validation, replay, and mutation rejection. The unproven shortcut with no
source or exercise corpus remains blocked. This campaign does not mutate the
production registry or authorize a new live capability.
