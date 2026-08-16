# Stage Z — shadow validation of HLE gap learning proposals

Stage Y produced answer-key-blind learning proposals from exact residual
clusters. This stage validates those proposals against the already frozen,
source-backed independent-corpus manifests for each corresponding curriculum
capability.

Promotion remains disabled. A proposal must have source provenance, at least
120 independent supported exercises, exact decisions, replay and tamper
coverage, and zero false authorization/denial before it is considered
sandbox-validatable.

Expected result for the five exact-overlap plans:

| Gate | Result |
|---|---:|
| Plans with exact overlap | 5 |
| Sandbox-validated plans | 3/5 |
| Promotion-allowed plans | 0 |
| Manifest mutation | false |
| Production registry mutations | 0 |
| False authorizations | 0 |

Reproduce with:

```text
cargo run --quiet --bin stage_z_hle_gap_validation
```

Machine-readable report: `docs/stage_z_hle_gap_validation.json`.

Two plans remain blocked because their JSON manifests lack structured source
provenance, even though related Markdown documentation cites sources. The
validator does not infer provenance across formats.
